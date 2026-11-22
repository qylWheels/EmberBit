use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

use crate::ast;

#[derive(Debug, Clone)]
pub struct Array {
    pub elements: Vec<InnerMutableValueRef>,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct Slice {
    pub array: InnerMutableValueRef,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct Tuple {
    pub elements: Vec<InnerMutableValueRef>,
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub fields: HashMap<String, InnerMutableValueRef>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub ast: *const ast::FunctionDefinition,
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Array(Array),
    Slice(Slice),
    Tuple(Tuple),
    Struct(Struct),
    Function(Function),
}

#[derive(Debug, Clone)]
pub struct InnerMutableValueRef(Rc<RefCell<Value>>);

impl Hash for InnerMutableValueRef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.0.as_ptr() as usize).hash(state);
    }
}

impl PartialEq for InnerMutableValueRef {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}

impl Eq for InnerMutableValueRef {}

#[derive(Debug)]
pub enum InterpretError {
    MissingMain,
    TypeMismatch(&'static str),
    UnknownBinaryOp,
    ReturnOutsideFunction,
    UnknownExpression(&'static str),
}

pub type InterpretResult<T> = Result<T, InterpretError>;

pub struct Interpreter {
    pub struct_defs: HashMap<String, ast::StructDefinition>,
    pub function_defs: HashMap<String, ast::FunctionDefinition>,
    pub values: HashSet<InnerMutableValueRef>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            struct_defs: HashMap::new(),
            function_defs: HashMap::new(),
            values: HashSet::new(),
        }
    }

    pub fn interpret_module(
        &mut self,
        module: &ast::Module,
    ) -> InterpretResult<InnerMutableValueRef> {
        let mut env = HashMap::new();

        for topdef in &module.top_definitions {
            match topdef {
                ast::TopDefinition::Import(_) => {}
                ast::TopDefinition::Struct(s) => {
                    self.struct_defs.insert(s.name.clone(), s.clone());
                }
                ast::TopDefinition::Function(f) => {
                    let value =
                        InnerMutableValueRef(Rc::new(RefCell::new(Value::Function(Function {
                            ast: f as *const ast::FunctionDefinition,
                        }))));
                    self.values.insert(value.clone());
                    env.insert(f.name.clone(), value);
                }
            }
        }

        let main_def = env
            .get("main")
            .cloned()
            .ok_or(InterpretError::MissingMain)?;

        match *main_def.0.borrow() {
            Value::Function(Function { ast }) => {
                return self.interpret_function(unsafe { ast.as_ref().unwrap() }, &env);
            }
            _ => return Err(InterpretError::MissingMain),
        }
    }

    pub fn interpret_function(
        &mut self,
        func: &ast::FunctionDefinition,
        env: &HashMap<String, InnerMutableValueRef>,
    ) -> InterpretResult<InnerMutableValueRef> {
        let mut newenv = env.clone();
        for stmt in &func.body {
            if let ast::Statement::Return(ast::ReturnStatement { expr }) = stmt {
                return self.interpret_expression(expr, &newenv);
            }
            newenv = self.interpret_statement(stmt, &newenv)?;
        }
        let value = InnerMutableValueRef(Rc::new(RefCell::new(Value::Tuple(Tuple {
            elements: vec![],
        }))));
        self.values.insert(value.clone());
        Ok(value)
    }

    pub fn interpret_statement(
        &mut self,
        stmt: &ast::Statement,
        env: &HashMap<String, InnerMutableValueRef>,
    ) -> InterpretResult<HashMap<String, InnerMutableValueRef>> {
        let mut newenv = env.clone();
        match stmt {
            ast::Statement::ConstDef(s) => {
                let v = self.interpret_expression(&s.expr, &newenv)?;
                newenv.insert(s.name.clone(), v);
            }
            ast::Statement::VarDef(s) => {
                let v = self.interpret_expression(&s.expr, &newenv)?;
                newenv.insert(s.name.clone(), v);
            }
            ast::Statement::Assignment(s) => {
                let value = self.interpret_expression(&s.rhs, &newenv)?;
                let reference = self.interpret_lvalue_expression(&s.lhs, &mut newenv)?;
                unsafe { *reference = value };
            }
            ast::Statement::Return(_) => {
                return Err(InterpretError::ReturnOutsideFunction);
            }
            ast::Statement::Expression(s) => {
                let _ = self.interpret_expression(&s.expr, &newenv)?;
            }
        }
        Ok(newenv)
    }

    pub fn interpret_expression(
        &mut self,
        expr: &ast::Expression,
        env: &HashMap<String, InnerMutableValueRef>,
    ) -> InterpretResult<InnerMutableValueRef> {
        match expr {
            ast::Expression::Array(e) => {
                let elements = e
                    .elements
                    .iter()
                    .map(|x| self.interpret_expression(x, env))
                    .collect::<InterpretResult<Vec<_>>>()?;
                let length = elements.len();
                let value = InnerMutableValueRef(Rc::new(RefCell::new(Value::Array(Array {
                    elements,
                    length,
                }))));
                self.values.insert(value.clone());
                Ok(value)
            }
            ast::Expression::Tuple(e) => {
                let elements = e
                    .elements
                    .iter()
                    .map(|x| self.interpret_expression(x, env))
                    .collect::<InterpretResult<Vec<_>>>()?;
                let value =
                    InnerMutableValueRef(Rc::new(RefCell::new(Value::Tuple(Tuple { elements }))));
                self.values.insert(value.clone());
                Ok(value)
            }
            ast::Expression::Binary(e) => {
                let lhs = self.interpret_expression(e.lhs.as_ref(), env)?;
                let rhs = self.interpret_expression(e.rhs.as_ref(), env)?;
                let lhs_int = match *lhs.0.borrow() {
                    Value::Int(n) => n,
                    _ => return Err(InterpretError::TypeMismatch("binary lhs must be int")),
                };
                let rhs_int = match *rhs.0.borrow() {
                    Value::Int(n) => n,
                    _ => return Err(InterpretError::TypeMismatch("binary rhs must be int")),
                };
                let result = match e.op {
                    ast::BinaryOp::Add => lhs_int + rhs_int,
                    ast::BinaryOp::Sub => lhs_int - rhs_int,
                    ast::BinaryOp::Mul => lhs_int * rhs_int,
                    ast::BinaryOp::Div => lhs_int / rhs_int,
                };
                let value = InnerMutableValueRef(Rc::new(RefCell::new(Value::Int(result))));
                self.values.insert(value.clone());
                Ok(value)
            }
            ast::Expression::Slice(ast::SliceExpression { base, start, end }) => {
                let array = self.interpret_expression(base.as_ref(), env)?;
                let start = self.interpret_expression(start.as_ref(), env)?;
                let end = self.interpret_expression(end.as_ref(), env)?;
                let start = match *start.0.borrow() {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => unreachable!(),
                };
                let end = match *end.0.borrow() {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => unreachable!(),
                };
                let value = InnerMutableValueRef(Rc::new(RefCell::new(Value::Slice(Slice {
                    array,
                    start,
                    end,
                }))));
                self.values.insert(value.clone());
                Ok(value)
            }
            ast::Expression::Index(e) => {
                let index = self.interpret_expression(e.index.as_ref(), env)?;
                let index = match *index.0.borrow() {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => unreachable!(),
                };
                let base = self.interpret_expression(e.base.as_ref(), env)?;
                match &*base.0.borrow() {
                    Value::Array(a) => Ok(a.elements[index].clone()),
                    Value::Slice(slice) => {
                        let array = &slice.array;
                        match &*array.0.borrow() {
                            Value::Array(a) => Ok(a.elements[slice.start + index].clone()),
                            _ => unreachable!(),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            ast::Expression::Identifier(e) => Ok(env.get(&e.name).cloned().unwrap()),
            ast::Expression::Integer(e) => {
                let value = InnerMutableValueRef(Rc::new(RefCell::new(Value::Int(e.value))));
                self.values.insert(value.clone());
                Ok(value)
            }
            ast::Expression::Call(e) => {
                let callee = self.interpret_expression(e.callee.as_ref(), env)?;
                let args = e
                    .args
                    .iter()
                    .map(|x| self.interpret_expression(x, env))
                    .collect::<InterpretResult<Vec<_>>>()?;
                match &*callee.0.borrow() {
                    Value::Function(func) => {
                        let func_ast = unsafe { func.ast.as_ref().unwrap() };
                        let arg_names = func_ast
                            .params
                            .iter()
                            .map(|x| x.1.clone())
                            .collect::<Vec<_>>();
                        let mut newenv = env.clone();
                        for (name, arg) in arg_names.iter().zip(args) {
                            newenv.insert(name.clone(), arg.clone());
                        }
                        self.interpret_function(func_ast, &newenv)
                    }
                    _ => unreachable!(),
                }
            }
            ast::Expression::Struct(e) => {
                let field_names = e
                    .fields
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>();
                let field_exprs = e
                    .fields
                    .iter()
                    .map(|(_, expr)| self.interpret_expression(expr, env))
                    .collect::<InterpretResult<Vec<_>>>()?;
                let fields = field_names
                    .iter()
                    .zip(field_exprs.iter())
                    .map(|(name, expr)| (name.clone(), expr.clone()))
                    .collect::<HashMap<_, _>>();
                let value =
                    InnerMutableValueRef(Rc::new(RefCell::new(Value::Struct(Struct { fields }))));
                self.values.insert(value.clone());
                Ok(value)
            }
            ast::Expression::FieldAccess(e) => {
                let base = self.interpret_expression(e.base.as_ref(), env)?;
                match &*base.0.borrow() {
                    Value::Struct(struct_val) => {
                        // First try to find field in struct.
                        match struct_val.fields.get(&e.field.name) {
                            Some(val) => {
                                self.values.insert(val.clone());
                                return Ok(val.clone());
                            }
                            None => (),
                        };

                        // If field not found, we try to find it in methods.
                        match struct_val.methods.get(&e.field.name) {
                            Some(method) => {
                                self.values.insert(method.clone());
                                return Ok(method.clone());
                            }
                            None => unreachable!(),
                        };
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn interpret_lvalue_expression<'a>(
        &mut self,
        lval_expr: &ast::Expression,
        env: &'a mut HashMap<String, InnerMutableValueRef>,
    ) -> InterpretResult<*mut InnerMutableValueRef> {
        let env_copy = env.clone();
        match lval_expr {
            ast::Expression::Identifier(e) => {
                Ok(env.get_mut(&e.name).unwrap() as *mut InnerMutableValueRef)
            }
            ast::Expression::Index(e) => {
                let index = self.interpret_expression(e.index.as_ref(), &env_copy)?;
                let index = match *index.0.borrow() {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => unreachable!(),
                };
                let base = self.interpret_lvalue_expression(e.base.as_ref(), env)?;
                let base = unsafe { base.as_mut().unwrap() };
                match &mut *base.0.borrow_mut() {
                    Value::Array(arr) => {
                        return Ok(&mut arr.elements[index] as *mut InnerMutableValueRef);
                    }
                    Value::Slice(slice) => match &mut *slice.array.0.borrow_mut() {
                        Value::Array(arr) => {
                            return Ok(
                                &mut arr.elements[slice.start + index] as *mut InnerMutableValueRef
                            );
                        }
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                };
            }
            ast::Expression::FieldAccess(e) => {
                let base = self.interpret_lvalue_expression(e.base.as_ref(), env)?;
                let base = unsafe { base.as_mut().unwrap() };
                match &mut *base.0.borrow_mut() {
                    Value::Struct(struct_val) => {
                        let field_val = struct_val.fields.get_mut(&e.field.name).unwrap();
                        return Ok(field_val as *mut InnerMutableValueRef);
                    }
                    _ => unreachable!(),
                };
            }
            _ => unreachable!(),
        }
    }
}
