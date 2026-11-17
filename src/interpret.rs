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
pub enum Value {
    Int(i64),
    Array(Array),
    Slice(Slice),
    Tuple(Tuple),
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
        for topdef in &module.top_definitions {
            match topdef {
                ast::TopDefinition::Import(_) => {}
                ast::TopDefinition::Struct(s) => {
                    self.struct_defs.insert(s.name.clone(), s.clone());
                }
                ast::TopDefinition::Function(f) => {
                    self.function_defs.insert(f.name.clone(), f.clone());
                }
            }
        }

        let main_def = self
            .function_defs
            .get("main")
            .cloned()
            .ok_or(InterpretError::MissingMain)?;

        let env = HashMap::new();
        self.interpret_function(&main_def, &env)
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
        &self,
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
                *reference = value;
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
        }
    }

    pub fn interpret_lvalue_expression<'a>(
        &self,
        lval_expr: &ast::Expression,
        env: &'a mut HashMap<String, InnerMutableValueRef>,
    ) -> InterpretResult<InnerMutableValueRef> {
        let env_copy = env.clone();
        match lval_expr {
            ast::Expression::Identifier(e) => env
                .get_mut(&e.name)
                .ok_or(InterpretError::TypeMismatch("unbound identifier")),
            ast::Expression::Index(e) => {
                let index = self.interpret_expression(e.index.as_ref(), &env_copy)?;
                let index = match index {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => return Err(InterpretError::TypeMismatch("lvalue index must be int")),
                };
                let base = self.interpret_lvalue_expression(e.base.as_ref(), env)?;
                match base {
                    Value::Array(arr) => return Ok(&mut arr.elements[index]),
                    Value::Slice(slice) => {
                        return Ok(&mut slice.array.elements[slice.start + index]);
                    }
                    _ => {
                        return Err(InterpretError::TypeMismatch(
                            "lvalue index base must be Array or VarSlice",
                        ));
                    }
                };
            }
            _ => Err(InterpretError::UnknownExpression(
                "unknown lvalue expression",
            )),
        }
    }
}
