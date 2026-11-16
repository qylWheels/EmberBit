use std::collections::HashMap;

use crate::ast;

#[derive(Debug, Clone)]
pub struct Pointer {
    pub pointee: Box<Value>,
}

#[derive(Debug, Clone)]
pub struct VariablePointer {
    pub pointee: Box<Value>,
}

#[derive(Debug, Clone)]
pub struct Array {
    pub elements: Vec<Value>,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct ConstSlice {
    pub array: Box<Array>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct VarSlice {
    pub array: Box<Array>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct Tuple {
    pub elements: Vec<Value>,
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Pointer(Pointer),
    VariablePointer(VariablePointer),
    Array(Array),
    ConstSlice(ConstSlice),
    VarSlice(VarSlice),
    Tuple(Tuple),
}

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
        }
    }

    pub fn interpret_module(&mut self, module: &ast::Module) -> InterpretResult<Value> {
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
        &self,
        func: &ast::FunctionDefinition,
        env: &HashMap<String, Value>,
    ) -> InterpretResult<Value> {
        let mut newenv = env.clone();
        for stmt in &func.body {
            if let ast::Statement::Return(ast::ReturnStatement { expr }) = stmt {
                return self.interpret_expression(expr, &newenv);
            }
            newenv = self.interpret_statement(stmt, &newenv)?;
        }
        Ok(Value::Tuple(Tuple { elements: vec![] }))
    }

    pub fn interpret_statement(
        &self,
        stmt: &ast::Statement,
        env: &HashMap<String, Value>,
    ) -> InterpretResult<HashMap<String, Value>> {
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
        &self,
        expr: &ast::Expression,
        env: &HashMap<String, Value>,
    ) -> InterpretResult<Value> {
        match expr {
            ast::Expression::Address(e) => Ok(Value::Pointer(Pointer {
                pointee: Box::new(self.interpret_expression(e.expr.as_ref(), env)?),
            })),
            ast::Expression::AddressVariable(e) => Ok(Value::VariablePointer(VariablePointer {
                pointee: Box::new(self.interpret_expression(e.expr.as_ref(), env)?),
            })),
            ast::Expression::Array(e) => {
                let elements = e
                    .elements
                    .iter()
                    .map(|x| self.interpret_expression(x, env))
                    .collect::<InterpretResult<Vec<_>>>()?;
                let length = elements.len();
                Ok(Value::Array(Array { elements, length }))
            }
            ast::Expression::Tuple(e) => {
                let elements = e
                    .elements
                    .iter()
                    .map(|x| self.interpret_expression(x, env))
                    .collect::<InterpretResult<Vec<_>>>()?;
                Ok(Value::Tuple(Tuple { elements }))
            }
            ast::Expression::Binary(e) => {
                let lhs = self.interpret_expression(e.lhs.as_ref(), env)?;
                let rhs = self.interpret_expression(e.rhs.as_ref(), env)?;
                let lhs_int = match lhs {
                    Value::Int(n) => n,
                    _ => return Err(InterpretError::TypeMismatch("binary lhs must be int")),
                };
                let rhs_int = match rhs {
                    Value::Int(n) => n,
                    _ => return Err(InterpretError::TypeMismatch("binary rhs must be int")),
                };
                let v = match e.op {
                    ast::BinaryOp::Add => lhs_int + rhs_int,
                    ast::BinaryOp::Sub => lhs_int - rhs_int,
                    ast::BinaryOp::Mul => lhs_int * rhs_int,
                    ast::BinaryOp::Div => lhs_int / rhs_int,
                };
                Ok(Value::Int(v))
            }
            ast::Expression::ConstSlice(ast::ConstSliceExpression { base, start, end })
            | ast::Expression::VarSlice(ast::VarSliceExpression { base, start, end }) => {
                let array = self.interpret_expression(base.as_ref(), env)?;
                let array = match array {
                    Value::Array(a) => a,
                    _ => unreachable!(),
                };
                let start = self.interpret_expression(start.as_ref(), env)?;
                let end = self.interpret_expression(end.as_ref(), env)?;
                let start = match start {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => unreachable!(),
                };
                let end = match end {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => unreachable!(),
                };
                if matches!(expr, ast::Expression::ConstSlice(_)) {
                    Ok(Value::ConstSlice(ConstSlice {
                        array: Box::new(array),
                        start,
                        end,
                    }))
                } else {
                    Ok(Value::ConstSlice(ConstSlice {
                        array: Box::new(array),
                        start,
                        end,
                    }))
                }
            }
            ast::Expression::Index(e) => {
                let index = self.interpret_expression(e.index.as_ref(), env)?;
                let index = match index {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => unreachable!(),
                };
                let base = self.interpret_expression(e.base.as_ref(), env)?;
                match base {
                    Value::Array(a) => Ok(a.elements[index].clone()),
                    Value::ConstSlice(slice) => {
                        Ok(slice.array.elements[slice.start + index].clone())
                    }
                    Value::VarSlice(slice) => Ok(slice.array.elements[slice.start + index].clone()),
                    _ => unreachable!(),
                }
            }
            ast::Expression::Identifier(e) => env
                .get(&e.name)
                .cloned()
                .ok_or(InterpretError::TypeMismatch("unbound identifier")),
            ast::Expression::Integer(e) => Ok(Value::Int(e.value)),
        }
    }

    pub fn interpret_lvalue_expression<'a>(
        &self,
        lval_expr: &ast::Expression,
        env: &'a mut HashMap<String, Value>,
    ) -> InterpretResult<&'a mut Value> {
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
                    Value::VarSlice(slice) => {
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
