use crate::ast;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum SemanticError {
    NoType(String),
    UnknownType(String),
    UnboundedIdentifier(String),
    IncompatibleType(String),
    ArrayElemType(String),
    Variable(String),
    IndexType(String),
    SliceType(String),
}

pub type Result<T> = std::result::Result<T, SemanticError>;

type Type = ast::Type;
type TrivialType = ast::TrivialType;
type PointerType = ast::PointerType;
type VariablePointerType = ast::VariablePointerType;
type ArrayType = ast::ArrayType;
type ConstSliceType = ast::ConstSliceType;
type VarSliceType = ast::VarSliceType;
type TupleType = ast::TupleType;
type FunctionType = ast::FunctionType;

#[derive(Debug, Clone)]
pub struct IdentifierProperties {
    pub ty: Type,
    pub variable: bool,
}

pub struct SemanticChecker {
    /// usize: address of AST node
    type_of_node: HashMap<usize, Type>,
}

impl Default for SemanticChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticChecker {
    pub fn new() -> Self {
        Self {
            type_of_node: HashMap::new(),
        }
    }

    pub fn get_type_of_node<T>(&self, node: &T) -> Option<&Type> {
        self.type_of_node.get(&(node as *const T as usize))
    }

    pub fn type_annotation_to_type(&self, type_annotation: &ast::Type) -> Result<Type> {
        Ok(type_annotation.clone())
    }

    pub fn check_module(&mut self, ast: &ast::Module) -> Result<()> {
        for topdef in &ast.top_definitions {
            match topdef {
                ast::TopDefinition::Import(i) => self.check_import(i)?,
                ast::TopDefinition::Struct(s) => self.check_struct(s)?,
                ast::TopDefinition::Function(f) => {
                    self.check_function(f, &HashMap::new())?;
                }
            }
        }
        Ok(())
    }

    pub fn check_import(&mut self, _ast: &ast::ImportDefinition) -> Result<()> {
        Ok(())
    }

    pub fn check_struct(&mut self, _ast: &ast::StructDefinition) -> Result<()> {
        Ok(())
    }

    pub fn check_function(
        &mut self,
        ast: &ast::FunctionDefinition,
        ctx: &HashMap<String, IdentifierProperties>,
    ) -> Result<ast::FunctionType> {
        let param_types: Vec<Type> = ast
            .params
            .iter()
            .map(|(_name, ty)| self.type_annotation_to_type(ty))
            .collect::<Result<Vec<_>>>()?;

        let mut curr_ctx = ctx.clone();
        for stmt in &ast.body {
            curr_ctx = self.check_statement(stmt, &curr_ctx)?;
        }

        let last_stmt = ast
            .body
            .last()
            .ok_or_else(|| SemanticError::IncompatibleType("Empty function body".into()))?;

        let return_type = if let ast::Statement::Return(ast::ReturnStatement { expr }) = last_stmt {
            self.check_expression(expr, &curr_ctx)?
        } else {
            Type::Tuple(ast::TupleType { elem_types: vec![] })
        };

        let annotation = self.type_annotation_to_type(&ast.return_type)?;
        if annotation != return_type {
            return Err(SemanticError::IncompatibleType(format!(
                "Function {} has incompatible return types {:?} (annotation) and {:?} (actual)",
                ast.name, annotation, return_type
            )));
        }

        let ty = FunctionType {
            param_types,
            return_type: Box::new(return_type),
        };
        self.type_of_node.insert(
            ast as *const ast::FunctionDefinition as usize,
            Type::Function(ty.clone()),
        );
        Ok(ty)
    }

    pub fn check_statement(
        &mut self,
        ast: &ast::Statement,
        ctx: &HashMap<String, IdentifierProperties>,
    ) -> Result<HashMap<String, IdentifierProperties>> {
        match ast {
            ast::Statement::ConstDef(ast::ConstDefStatement { name, ty, expr }) => {
                let expr_type = self.check_expression(expr, ctx)?;
                let annotation = self.type_annotation_to_type(ty)?;
                if annotation != expr_type {
                    return Err(SemanticError::IncompatibleType(format!(
                        "Const statement has incompatible types {:?} (annotation) and {:?} (actual)",
                        annotation, expr_type
                    )));
                }
                let mut new_ctx = ctx.clone();
                new_ctx.insert(
                    name.clone(),
                    IdentifierProperties {
                        ty: expr_type,
                        variable: false,
                    },
                );
                Ok(new_ctx)
            }
            ast::Statement::VarDef(ast::VarDefStatement { name, ty, expr }) => {
                let expr_type = self.check_expression(expr, ctx)?;
                let annotation = self.type_annotation_to_type(ty)?;
                if annotation != expr_type {
                    return Err(SemanticError::IncompatibleType(format!(
                        "Var statement has incompatible types {:?} (annotation) and {:?} (actual)",
                        annotation, expr_type
                    )));
                }
                let mut new_ctx = ctx.clone();
                new_ctx.insert(
                    name.clone(),
                    IdentifierProperties {
                        ty: expr_type,
                        variable: true,
                    },
                );
                Ok(new_ctx)
            }
            ast::Statement::Assignment(ast::AssignmentStatement { lhs, rhs }) => {
                if !self.check_expression_assignable(lhs, ctx)? {
                    return Err(SemanticError::Variable(
                        "Assignment statement has non-variable lhs".into(),
                    ));
                }
                let lhs_type = self.check_expression(lhs, ctx)?;
                let rhs_type = self.check_expression(rhs, ctx)?;
                if lhs_type != rhs_type {
                    return Err(SemanticError::IncompatibleType(format!(
                        "Assignment statement has incompatible types {:?} (lhs) and {:?} (rhs)",
                        lhs_type, rhs_type
                    )));
                }
                Ok(ctx.clone())
            }
            ast::Statement::Return(ast::ReturnStatement { expr }) => {
                let _ = self.check_expression(expr, ctx)?;
                Ok(ctx.clone())
            }
            ast::Statement::Expression(ast::ExpressionStatement { expr }) => {
                let _ = self.check_expression(expr, ctx)?;
                Ok(ctx.clone())
            }
        }
    }

    pub fn check_expression_assignable(
        &mut self,
        ast: &ast::Expression,
        ctx: &HashMap<String, IdentifierProperties>,
    ) -> Result<bool> {
        match ast {
            ast::Expression::Identifier(ast::Identifier { name }) => match ctx.get(name) {
                Some(props) => Ok(props.variable),
                None => Err(SemanticError::UnboundedIdentifier(format!(
                    "Identifier {} is not declared",
                    name
                ))),
            },
            ast::Expression::Index(ast::IndexExpression { base, .. }) => {
                let base_type = self.check_expression(base, ctx)?;
                let variable = match base_type {
                    Type::Array(_) => self.check_expression_assignable(base, ctx)?,
                    Type::VarSlice(_) => true,
                    _ => false,
                };
                return Ok(variable);
            }
            _ => Ok(false),
        }
    }

    pub fn check_expression(
        &mut self,
        ast: &ast::Expression,
        ctx: &HashMap<String, IdentifierProperties>,
    ) -> Result<Type> {
        let key = ast as *const ast::Expression as usize;
        match ast {
            ast::Expression::Address(ast::AddressExpression { expr }) => {
                let pointee_type = self.check_expression(expr, ctx)?;
                let ty = Type::Pointer(ast::PointerType {
                    pointee_type: Box::new(pointee_type),
                });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::AddressVariable(ast::AddressVariableExpression { expr }) => {
                let pointee_type = self.check_expression(expr, ctx)?;
                let ty = Type::VariablePointer(ast::VariablePointerType {
                    pointee_type: Box::new(pointee_type),
                });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Array(ast::ArrayExpression { elements }) => {
                let types = elements
                    .iter()
                    .map(|e| self.check_expression(e, ctx))
                    .collect::<Result<Vec<_>>>()?;
                if types.is_empty() {
                    return Err(SemanticError::ArrayElemType(
                        "Array isn't allowed to be empty".into(),
                    ));
                }
                let type_set: HashSet<&Type> = types.iter().collect();
                if type_set.len() > 1 {
                    return Err(SemanticError::ArrayElemType(
                        "Array expression has elements of different types".into(),
                    ));
                }
                let ty = Type::Array(ast::ArrayType {
                    elem_type: Box::new(types[0].clone()),
                    size: elements.len() as i64,
                });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Tuple(ast::TupleExpression { elements }) => {
                let types = elements
                    .iter()
                    .map(|e| self.check_expression(e, ctx))
                    .collect::<Result<Vec<_>>>()?;
                let ty = Type::Tuple(ast::TupleType { elem_types: types });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Index(ast::IndexExpression { base, index }) => {
                let base_type = self.check_expression(base, ctx)?;
                let elem_type = match base_type {
                    Type::Array(ast::ArrayType { elem_type, .. }) => *elem_type,
                    Type::ConstSlice(ast::ConstSliceType { elem_type, .. }) => *elem_type,
                    Type::VarSlice(ast::VarSliceType { elem_type, .. }) => *elem_type,
                    other => {
                        return Err(SemanticError::IncompatibleType(format!(
                            "Base expression must be of type Array or Slice, but {:?} is given",
                            other
                        )));
                    }
                };
                let index_type = self.check_expression(index, ctx)?;
                if !matches!(&index_type, Type::Trivial(ast::TrivialType { name }) if name == "i32")
                {
                    return Err(SemanticError::IndexType(format!(
                        "Index must be of type i32, but {:?} is given",
                        index_type
                    )));
                }
                self.type_of_node.insert(key, elem_type.clone());
                Ok(elem_type)
            }
            ast::Expression::ConstSlice(ast::ConstSliceExpression { base, start, end })
            | ast::Expression::VarSlice(ast::VarSliceExpression { base, start, end }) => {
                let base_type = self.check_expression(base, ctx)?;
                let start_type = self.check_expression(start, ctx)?;
                let end_type = self.check_expression(end, ctx)?;

                let elem_type = match &base_type {
                    Type::Array(ast::ArrayType { elem_type, .. }) => elem_type.as_ref().clone(),
                    other => {
                        return Err(SemanticError::SliceType(format!(
                            "Slice base must be of type ArrayType, but {:?} is given",
                            other
                        )));
                    }
                };
                if !matches!(&start_type, Type::Trivial(ast::TrivialType { name }) if name == "i32")
                {
                    return Err(SemanticError::SliceType(format!(
                        "Slice start must be of type i32, but {:?} is given",
                        start_type
                    )));
                }
                if !matches!(&end_type, Type::Trivial(ast::TrivialType { name }) if name == "i32") {
                    return Err(SemanticError::SliceType(format!(
                        "Slice end must be of type i32, but {:?} is given",
                        end_type
                    )));
                }
                let ty = if matches!(ast, ast::Expression::ConstSlice(_)) {
                    Type::ConstSlice(ast::ConstSliceType {
                        elem_type: Box::new(elem_type),
                    })
                } else if matches!(ast, ast::Expression::VarSlice(_)) {
                    Type::VarSlice(ast::VarSliceType {
                        elem_type: Box::new(elem_type),
                    })
                } else {
                    unreachable!("Unknown slice type");
                };
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Binary(ast::BinaryExpression { lhs, rhs, op }) => {
                let lhs_type = self.check_expression(lhs, ctx)?;
                let rhs_type = self.check_expression(rhs, ctx)?;
                if lhs_type != rhs_type {
                    return Err(SemanticError::IncompatibleType(format!(
                        "Binary expression {:?} has incompatible types {:?} and {:?}",
                        op, lhs_type, rhs_type
                    )));
                }
                self.type_of_node.insert(key, lhs_type.clone());
                Ok(lhs_type)
            }
            ast::Expression::Integer(ast::IntegerLiteral { value: _ }) => {
                let ty = Type::Trivial(ast::TrivialType {
                    name: "i32".to_string(),
                });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Identifier(ast::Identifier { name }) => {
                let props = ctx.get(name).ok_or_else(|| {
                    SemanticError::UnboundedIdentifier(format!(
                        "Identifier {} is not declared",
                        name
                    ))
                })?;
                let ty = props.ty.clone();
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
        }
    }
}
