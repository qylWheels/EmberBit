use crate::ast;
use crate::types;
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
    ArgumentType(Vec<SemanticError>),
    Visibility(String),
    DuplicateField(String),
}

pub type Result<T> = std::result::Result<T, SemanticError>;

type Visibility = ast::Visibility;

#[derive(Debug, Clone)]
pub struct IdentifierProperties {
    pub ty: types::Type,
    pub variable: bool,
}

#[derive(Debug)]
pub struct SemanticChecker {
    /// Struct type information.
    struct_type_info: HashMap<String, ast::StructDefinition>,

    /// Type of node. usize is node address.
    type_of_node: HashMap<usize, types::Type>,
}

impl SemanticChecker {
    pub fn new() -> Self {
        Self {
            struct_type_info: HashMap::new(),
            type_of_node: HashMap::new(),
        }
    }

    pub fn get_type_of_node<T>(&self, node: &T) -> Option<&types::Type> {
        self.type_of_node.get(&(node as *const _ as usize))
    }

    pub fn get_struct_type_info(&self, struct_name: &str) -> Option<&ast::StructDefinition> {
        self.struct_type_info.get(struct_name)
    }

    pub fn type_annotation_to_type(&self, type_annotation: &types::Type) -> Result<types::Type> {
        Ok(type_annotation.clone())
    }

    pub fn check_module(&mut self, ast: &ast::Module) -> Result<()> {
        // Collect all top definitions into context first.
        let mut ctx = HashMap::new();
        for topdef in &ast.top_definitions {
            match topdef {
                ast::TopDefinition::Import(_i) => {}
                ast::TopDefinition::Struct(s) => {
                    // Insert struct type into context.
                    self.struct_type_info.insert(s.name.clone(), s.clone());
                }
                ast::TopDefinition::Function(f) => {
                    let param_mutabilities = f
                        .params
                        .iter()
                        .map(|(mutability, _, _)| mutability.clone())
                        .collect::<Vec<_>>();
                    let annotated_param_types: Vec<types::Type> = f
                        .params
                        .iter()
                        .map(|(_, _, ty)| self.type_annotation_to_type(ty))
                        .collect::<Result<Vec<_>>>()?;
                    let annotated_return_type = self.type_annotation_to_type(&f.return_type)?;
                    let func_type = types::Type::Function(types::FunctionType {
                        params: param_mutabilities
                            .iter()
                            .zip(annotated_param_types.iter())
                            .map(|(mutability, ty)| (mutability.clone(), ty.clone()))
                            .collect(),
                        return_type_mutability: f.return_type_mutability.clone(),
                        return_type: Box::new(annotated_return_type.clone()),
                    });
                    ctx.insert(
                        f.name.clone(),
                        IdentifierProperties {
                            ty: func_type.clone(),
                            variable: false,
                        },
                    );
                }
            }
        }

        for topdef in &ast.top_definitions {
            match topdef {
                ast::TopDefinition::Import(i) => self.check_import(i)?,
                ast::TopDefinition::Struct(s) => self.check_struct(s)?,
                ast::TopDefinition::Function(f) => {
                    self.check_function(f, &ctx, None)?;
                }
            }
        }
        Ok(())
    }

    pub fn check_import(&mut self, _ast: &ast::ImportDefinition) -> Result<()> {
        Ok(())
    }

    pub fn check_struct(&mut self, struct_def: &ast::StructDefinition) -> Result<()> {
        // Check struct fields and methods.
        // It is not allowed to use duplicate field/method names.
        let mut names = HashSet::new();
        for field in &struct_def.fields {
            let field_name = field.1.clone();
            if names.contains(&field_name) {
                return Err(SemanticError::DuplicateField(field_name));
            }
            names.insert(field_name);
        }
        for method in &struct_def.methods {
            let method_name = method.1.name.clone();
            if names.contains(&method_name) {
                return Err(SemanticError::DuplicateField(method_name));
            }
            names.insert(method_name);
        }

        // Check methods in struct.
        for method in &struct_def.methods {
            self.check_function(&method.1, &HashMap::new(), Some(&struct_def.name))?;
        }

        Ok(())
    }

    pub fn check_function(
        &mut self,
        ast: &ast::FunctionDefinition,
        ctx: &HashMap<String, IdentifierProperties>,
        curr_struct_scope: Option<&str>,
    ) -> Result<types::FunctionType> {
        let mut curr_ctx = ctx.clone();

        // Get parameter mutabilities.
        let param_mutabilities = ast
            .params
            .iter()
            .map(|(mutability, _, _)| mutability.clone())
            .collect::<Vec<_>>();

        // Get parameter names.
        let param_names = ast
            .params
            .iter()
            .map(|(_, name, _)| name.clone())
            .collect::<Vec<_>>();

        // Get annotated parameter types.
        let annotated_param_types: Vec<types::Type> = ast
            .params
            .iter()
            .map(|(_, _, ty)| {
                if curr_struct_scope.is_some() {
                    match ty {
                        types::Type::Trivial(types::TrivialType { name }) if name == "Self" => {
                            Ok(types::Type::Trivial(types::TrivialType {
                                name: curr_struct_scope.unwrap().to_string(),
                            }))
                        }
                        _ => self.type_annotation_to_type(ty),
                    }
                } else {
                    self.type_annotation_to_type(ty)
                }
            })
            .collect::<Result<Vec<_>>>()?;

        // Get return type.
        let annotated_return_type = match curr_struct_scope {
            Some(struct_name) => match &ast.return_type {
                types::Type::Trivial(types::TrivialType { name }) if name == "Self" => {
                    Ok(types::Type::Trivial(types::TrivialType {
                        name: struct_name.to_string(),
                    }))
                }
                _ => self.type_annotation_to_type(&ast.return_type),
            },
            None => self.type_annotation_to_type(&ast.return_type),
        }?;

        let params = param_mutabilities
            .iter()
            .zip(&param_names)
            .zip(&annotated_param_types)
            .map(|((mutability, name), ty)| (mutability.clone(), name.clone(), ty.clone()))
            .collect::<Vec<_>>();

        // Add parameters to context.
        for (mutability, name, ty) in params.iter() {
            curr_ctx.insert(
                name.clone(),
                IdentifierProperties {
                    ty: ty.clone(),
                    variable: *mutability == types::Mutability::Var,
                },
            );
        }

        for stmt in &ast.body {
            curr_ctx = self.check_statement(stmt, &curr_ctx, curr_struct_scope)?;
        }

        let last_stmt = ast
            .body
            .last()
            .ok_or_else(|| SemanticError::IncompatibleType("Empty function body".into()))?;

        let actual_return_type =
            if let ast::Statement::Return(ast::ReturnStatement { expr }) = last_stmt {
                self.check_expression(expr, &curr_ctx, curr_struct_scope)?
            } else {
                types::Type::Tuple(types::TupleType { elem_types: vec![] })
            };

        if annotated_return_type != actual_return_type {
            return Err(SemanticError::IncompatibleType(format!(
                "Function {} has incompatible return types {:?} (annotation) and {:?} (actual)",
                ast.name, annotated_return_type, actual_return_type
            )));
        }

        let ty = types::FunctionType {
            params: params
                .iter()
                .map(|(mutability, _, ty)| (mutability.clone(), ty.clone()))
                .collect(),
            return_type_mutability: ast.return_type_mutability.clone(),
            return_type: Box::new(actual_return_type),
        };
        self.type_of_node.insert(
            ast as *const ast::FunctionDefinition as usize,
            types::Type::Function(ty.clone()),
        );
        Ok(ty)
    }

    pub fn check_statement(
        &mut self,
        ast: &ast::Statement,
        ctx: &HashMap<String, IdentifierProperties>,
        curr_struct_scope: Option<&str>,
    ) -> Result<HashMap<String, IdentifierProperties>> {
        match ast {
            ast::Statement::ConstDef(ast::ConstDefStatement { name, ty, expr }) => {
                let expr_type = self.check_expression(expr, ctx, curr_struct_scope)?;
                if let Some(ty) = ty {
                    let annotation = self.type_annotation_to_type(ty)?;
                    if annotation != expr_type {
                        return Err(SemanticError::IncompatibleType(format!(
                            "Const statement has incompatible types {:?} (annotation) and {:?} (actual)",
                            annotation, expr_type
                        )));
                    }
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
                // It is not allowed to create variable reference on not-assignable expression.
                let expr_mutable = self.check_expression_mutable(expr, ctx, curr_struct_scope)?;
                if !expr_mutable {
                    return Err(SemanticError::Variable(
                        "Variable reference on non-assignable expression".into(),
                    ));
                }

                let expr_type = self.check_expression(expr, ctx, curr_struct_scope)?;
                if let Some(ty) = ty {
                    let annotation = self.type_annotation_to_type(ty)?;
                    if annotation != expr_type {
                        return Err(SemanticError::IncompatibleType(format!(
                            "Var statement has incompatible types {:?} (annotation) and {:?} (actual)",
                            annotation, expr_type
                        )));
                    }
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
                if !self.check_expression_assignable(lhs, ctx, curr_struct_scope)? {
                    return Err(SemanticError::Variable(
                        "Assignment statement has non-variable lhs".into(),
                    ));
                }
                let lhs_type = self.check_expression(lhs, ctx, curr_struct_scope)?;
                let rhs_type = self.check_expression(rhs, ctx, curr_struct_scope)?;
                if lhs_type != rhs_type {
                    return Err(SemanticError::IncompatibleType(format!(
                        "Assignment statement has incompatible types {:?} (lhs) and {:?} (rhs)",
                        lhs_type, rhs_type
                    )));
                }
                Ok(ctx.clone())
            }
            ast::Statement::Return(ast::ReturnStatement { expr: _ }) => {
                // Return statement should be checked in check_function.
                Ok(ctx.clone())
            }
            ast::Statement::Expression(ast::ExpressionStatement { expr }) => {
                let _ = self.check_expression(expr, ctx, curr_struct_scope)?;
                Ok(ctx.clone())
            }
        }
    }

    pub fn check_expression_assignable(
        &mut self,
        ast: &ast::Expression,
        ctx: &HashMap<String, IdentifierProperties>,
        curr_struct_scope: Option<&str>,
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
                let base_type = self.check_expression(base, ctx, curr_struct_scope)?;
                let variable = match base_type {
                    types::Type::Array(_) | types::Type::Slice(_) => {
                        self.check_expression_assignable(base, ctx, curr_struct_scope)?
                    }
                    _ => false,
                };
                return Ok(variable);
            }
            ast::Expression::FieldAccess(ast::FieldAccessExpression { base, field }) => {
                // Check if base is assignable.
                if !self.check_expression_assignable(base, ctx, curr_struct_scope)? {
                    return Ok(false);
                }

                let base_type = self.check_expression(base, ctx, curr_struct_scope)?;

                // Get struct actual type.
                let struct_type = match &base_type {
                    types::Type::Trivial(ty) => self.struct_type_info.get(&ty.name).ok_or(
                        SemanticError::IncompatibleType(
                            "Field access expression has non-struct type".into(),
                        ),
                    )?,
                    _ => Err(SemanticError::IncompatibleType(
                        "Field access expression has non-struct type".into(),
                    ))?,
                };

                // Check if field is in struct.
                let field_visibility_and_type = struct_type
                    .fields
                    .iter()
                    .find(|(_, name, _)| name == &field.name)
                    .map(|(visibility, _, ty)| (visibility.clone(), ty.clone()))
                    .ok_or(SemanticError::UnboundedIdentifier(format!(
                        "Field {} is not defined",
                        field.name
                    )))?;

                // Check if field is visible.
                if !curr_struct_scope.is_some_and(|s| s == struct_type.name) {
                    if field_visibility_and_type.0 != Visibility::Public {
                        return Err(SemanticError::Visibility(format!(
                            "Field {} is not visible outside struct definition",
                            field.name
                        )));
                    }
                }

                return Ok(true);
            }
            ast::Expression::Call(ast::CallExpression { callee, args: _ }) => {
                let callee_type = self.check_expression(callee, ctx, curr_struct_scope)?;
                let assignable = match &callee_type {
                    types::Type::Function(types::FunctionType {
                        params: _,
                        return_type_mutability,
                        return_type: _,
                    }) => {
                        if return_type_mutability == &types::Mutability::Const {
                            Ok(false)
                        } else {
                            Ok(true)
                        }
                    }
                    other => Err(SemanticError::IncompatibleType(format!(
                        "Callee in call expression must be a function, but it has type {:?}",
                        other
                    ))),
                };

                return assignable;
            }
            _ => Ok(false),
        }
    }

    pub fn check_expression_mutable(
        &mut self,
        expr: &ast::Expression,
        ctx: &HashMap<String, IdentifierProperties>,
        curr_struct_scope: Option<&str>,
    ) -> Result<bool> {
        match expr {
            ast::Expression::Binary(ast::BinaryExpression { lhs, op: _, rhs }) => Ok(self
                .check_expression_mutable(lhs, ctx, curr_struct_scope)?
                && self.check_expression_mutable(rhs, ctx, curr_struct_scope)?),
            ast::Expression::FieldAccess(ast::FieldAccessExpression { base, field: _ }) => {
                Ok(self.check_expression_mutable(base, ctx, curr_struct_scope)?)
            }
            ast::Expression::Call(ast::CallExpression { callee, args: _ }) => {
                let callee_type = self.check_expression(callee, ctx, curr_struct_scope)?;
                match callee_type {
                    types::Type::Function(types::FunctionType {
                        params: _,
                        return_type_mutability,
                        return_type: _,
                    }) => Ok(return_type_mutability == types::Mutability::Var),
                    _ => Err(SemanticError::IncompatibleType(format!(
                        "Callee in call expression must be a function, but it has type {:?}",
                        callee_type
                    ))),
                }
            }
            ast::Expression::Identifier(ast::Identifier { name }) => match ctx.get(name) {
                Some(props) => Ok(props.variable),
                None => Err(SemanticError::UnboundedIdentifier(format!(
                    "Identifier {} is not declared",
                    name
                ))),
            },
            // Integer literal and struct expression must be mutable, or we can't assign it to var.
            ast::Expression::Integer(_) | ast::Expression::Struct(_) => Ok(true),
            // Static method access must be immutable.
            ast::Expression::StaticMethodAccess(_) => Ok(false),
            other => {
                unimplemented!("We won't implement {:?} now", other)
            }
        }
    }

    fn is_method_of_struct(
        &mut self,
        expr: &ast::Expression,
        ctx: &HashMap<String, IdentifierProperties>,
        curr_struct_scope: Option<&str>,
    ) -> Result<bool> {
        match expr {
            ast::Expression::FieldAccess(ast::FieldAccessExpression { base, field }) => {
                let base_type = self.check_expression(base, ctx, curr_struct_scope)?;
                let struct_type = match &base_type {
                    types::Type::Trivial(ty) => self.struct_type_info.get(&ty.name).ok_or(
                        SemanticError::IncompatibleType(
                            "Field access expression has non-struct type".into(),
                        ),
                    )?,
                    _ => Err(SemanticError::IncompatibleType(
                        "Field access expression has non-struct type".into(),
                    ))?,
                };
                let method = struct_type
                    .methods
                    .iter()
                    .find(|(_, func_def)| func_def.name == field.name);
                Ok(method.is_some())
            }
            _ => Ok(false),
        }
    }

    pub fn check_expression(
        &mut self,
        ast: &ast::Expression,
        ctx: &HashMap<String, IdentifierProperties>,
        curr_struct_scope: Option<&str>,
    ) -> Result<types::Type> {
        let key = ast as *const ast::Expression as usize;
        match ast {
            ast::Expression::Array(ast::ArrayExpression { elements }) => {
                let types = elements
                    .iter()
                    .map(|e| self.check_expression(e, ctx, curr_struct_scope))
                    .collect::<Result<Vec<_>>>()?;
                if types.is_empty() {
                    return Err(SemanticError::ArrayElemType(
                        "Array isn't allowed to be empty".into(),
                    ));
                }
                let type_set: HashSet<&types::Type> = types.iter().collect();
                if type_set.len() > 1 {
                    return Err(SemanticError::ArrayElemType(
                        "Array expression has elements of different types".into(),
                    ));
                }
                let ty = types::Type::Array(types::ArrayType {
                    elem_type: Box::new(types[0].clone()),
                    size: elements.len() as i64,
                });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Tuple(ast::TupleExpression { elements }) => {
                let types = elements
                    .iter()
                    .map(|e| self.check_expression(e, ctx, curr_struct_scope))
                    .collect::<Result<Vec<_>>>()?;
                let ty = types::Type::Tuple(types::TupleType { elem_types: types });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Index(ast::IndexExpression { base, index }) => {
                let base_type = self.check_expression(base, ctx, curr_struct_scope)?;
                let elem_type = match base_type {
                    types::Type::Array(types::ArrayType { elem_type, .. }) => *elem_type,
                    types::Type::Slice(types::SliceType { elem_type, .. }) => *elem_type,
                    other => {
                        return Err(SemanticError::IncompatibleType(format!(
                            "Base expression must be of type Array or Slice, but {:?} is given",
                            other
                        )));
                    }
                };
                let index_type = self.check_expression(index, ctx, curr_struct_scope)?;
                if !matches!(&index_type, types::Type::Trivial(types::TrivialType { name }) if name == "Integer")
                {
                    return Err(SemanticError::IndexType(format!(
                        "Index must be of type Integer, but {:?} is given",
                        index_type
                    )));
                }
                self.type_of_node.insert(key, elem_type.clone());
                Ok(elem_type)
            }
            ast::Expression::Slice(ast::SliceExpression { base, start, end }) => {
                let base_type = self.check_expression(base, ctx, curr_struct_scope)?;
                let start_type = self.check_expression(start, ctx, curr_struct_scope)?;
                let end_type = self.check_expression(end, ctx, curr_struct_scope)?;

                let elem_type = match &base_type {
                    types::Type::Array(types::ArrayType { elem_type, .. }) => {
                        elem_type.as_ref().clone()
                    }
                    other => {
                        return Err(SemanticError::SliceType(format!(
                            "Slice base must be of type ArrayType, but {:?} is given",
                            other
                        )));
                    }
                };

                if !matches!(&start_type, types::Type::Trivial(types::TrivialType { name }) if name == "Integer")
                {
                    return Err(SemanticError::SliceType(format!(
                        "Slice start must be of type Integer, but {:?} is given",
                        start_type
                    )));
                }
                if !matches!(&end_type, types::Type::Trivial(types::TrivialType { name }) if name == "Integer")
                {
                    return Err(SemanticError::SliceType(format!(
                        "Slice end must be of type Integer, but {:?} is given",
                        end_type
                    )));
                }

                let ty = types::Type::Slice(types::SliceType {
                    elem_type: Box::new(elem_type),
                });

                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Binary(ast::BinaryExpression { lhs, rhs, op }) => {
                let lhs_type = self.check_expression(lhs, ctx, curr_struct_scope)?;
                let rhs_type = self.check_expression(rhs, ctx, curr_struct_scope)?;
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
                let ty = types::Type::Trivial(types::TrivialType {
                    name: "Integer".to_string(),
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
            ast::Expression::Call(ast::CallExpression { callee, args }) => {
                let callee_type = self.check_expression(callee, ctx, curr_struct_scope)?;
                if !matches!(&callee_type, types::Type::Function(_)) {
                    return Err(SemanticError::IncompatibleType(format!(
                        "Callee expression must be of type Function, but {:?} is given",
                        callee_type
                    )));
                }

                // Infer parameter types of the explicit arguments.
                let arg_types = args
                    .iter()
                    .map(|arg| self.check_expression(arg, ctx, curr_struct_scope))
                    .collect::<Result<Vec<_>>>()?;

                let ty = match callee {
                    types::Type::Function(func_type) => {
                        // Check if the number of arguments matches the number of parameters.
                        let expected_param_count = func_type.params.len();
                        let given_arg_count = arg_types.len();
                        if given_arg_count != expected_param_count {
                            return Err(SemanticError::IncompatibleType(format!(
                                "Function has {} parameters, but {} arguments are given",
                                expected_param_count, given_arg_count
                            )));
                        }

                        // Check mutability of parameters.
                        let annotated_mutabilities = func_type
                            .params
                            .iter()
                            .map(|(mutability, _)| *mutability == types::Mutability::Var)
                            .collect::<Vec<_>>();
                        let mut param_mutabilities = args
                            .iter()
                            .map(|arg| self.check_expression_mutable(arg, ctx, curr_struct_scope))
                            .collect::<Result<Vec<_>>>()?;
                        for i in 0..annotated_mutabilities.len() {
                            if annotated_mutabilities[i] && !param_mutabilities[i] {
                                return Err(SemanticError::Variable(format!(
                                    "Parameter {} requires mutable, but a constant is given",
                                    i
                                )));
                            }
                        }

                        // Check if the args types match.
                        if arg_types
                            != func_type
                                .params
                                .iter()
                                .map(|(_, ty)| ty.clone())
                                .collect::<Vec<_>>()
                        {
                            return Err(SemanticError::IncompatibleType(format!(
                                "Function has incompatible parameter types: {:?}(declared) vs {:?}(given)",
                                &arg_types, &func_type.params
                            )));
                        }

                        let ty = (*func_type.return_type).clone();
                        ty
                    }
                    ast::Expression::MethodAccess(ast::MethodAccessExpression {
                        base,
                        method_name,
                    }) => {
                        let callee_type = match callee_type {
                            types::Type::Function(func_type) => func_type,
                            _ => unreachable!(),
                        };

                        // Check if the number of arguments matches the number of parameters.
                        let expected_param_count = callee_type.params.len();
                        let given_arg_count = 1 + arg_types.len();
                        if given_arg_count != expected_param_count {
                            return Err(SemanticError::IncompatibleType(format!(
                                "Function has {} parameters, but {} arguments are given",
                                expected_param_count, given_arg_count
                            )));
                        }

                        // Check mutability of parameters.
                        let param_mutabilities = callee_type
                            .params
                            .iter()
                            .map(|(mutability, _)| *mutability == types::Mutability::Var)
                            .collect::<Vec<_>>();
                        let mut arg_mutabilities = args
                            .iter()
                            .map(|arg| self.check_expression_mutable(arg, ctx, curr_struct_scope))
                            .collect::<Result<Vec<_>>>()?;
                        let receiver_mutability =
                            self.check_expression_mutable(base, ctx, curr_struct_scope)?;
                        arg_mutabilities.insert(0, receiver_mutability);
                        for i in 0..param_mutabilities.len() {
                            if param_mutabilities[i] && !arg_mutabilities[i] {
                                return Err(SemanticError::Variable(format!(
                                    "Parameter {} requires mutable, but a constant is given",
                                    i
                                )));
                            }
                        }

                        // Check if the args types match.
                        // Note: The first argument is the receiver, which must be checked.
                        let mut arg_types = arg_types.clone();
                        let base_type = self
                            .type_of_node
                            .iter()
                            .find(|(key, _)| **key == (&**base as *const ast::Expression as usize))
                            .unwrap()
                            .1;
                        arg_types.insert(0, base_type.clone());
                        if arg_types
                            != callee_type
                                .params
                                .iter()
                                .map(|(_, ty)| ty.clone())
                                .collect::<Vec<_>>()
                        {
                            return Err(SemanticError::IncompatibleType(format!(
                                "Function has incompatible parameter types: {:?}(declared) vs {:?}(given)",
                                &arg_types, &callee_type.params
                            )));
                        }

                        let ty = *(callee_type.return_type.clone());
                        ty
                    }
                    other => {
                        return Err(SemanticError::IncompatibleType(format!(
                            "Callee must be of type Function, but {:?} is given",
                            other
                        )));
                    }
                };

                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::Struct(ast::StructExpression { name, fields }) => {
                // Get struct type information by name.
                let struct_def = self.struct_type_info.get(name).ok_or_else(|| {
                    SemanticError::NoType(format!("Type {} is not defined", name))
                })?;
                let struct_name = struct_def.name.clone();
                let struct_fields = struct_def.fields.clone();

                // If any field is private, it is not allowed to construct struct object
                // by struct expression, unless current expression is in relative struct.
                if !curr_struct_scope.is_some_and(|s| s == &struct_name) {
                    for (visibility, _, _) in &struct_fields {
                        if *visibility != Visibility::Public {
                            return Err(SemanticError::Visibility(format!(
                                "Struct {} has private fields, for which constructing struct object by struct expression is not allowed",
                                struct_name
                            )));
                        }
                    }
                }

                // Infer type of each field expression.
                let field_types = fields
                    .iter()
                    .map(|(_, expr)| self.check_expression(expr, ctx, Some(&struct_name)))
                    .collect::<Result<Vec<_>>>()?;

                // Check if the field types match the declared types.
                for ((name, _), ty) in fields.iter().zip(field_types.iter()) {
                    let field_annotated_type = &struct_fields
                        .iter()
                        .find(|(_, n, _)| n == name)
                        .ok_or_else(|| {
                            SemanticError::UnboundedIdentifier(format!(
                                "Field {} is not defined",
                                name
                            ))
                        })?
                        .2;
                    if *ty != *field_annotated_type {
                        return Err(SemanticError::IncompatibleType(format!(
                            "Field {} has incompatible type {:?} (annotated) vs {:?} (given)",
                            name, field_annotated_type, ty
                        )));
                    }
                }

                let ty = types::Type::Trivial(types::TrivialType { name: struct_name });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::FieldAccess(ast::FieldAccessExpression { base, field }) => {
                // Check type of base expression.
                let base_type = self.check_expression(base, ctx, curr_struct_scope)?;
                let struct_name = match base_type {
                    types::Type::Trivial(types::TrivialType { name }) => name,
                    _ => Err(SemanticError::IncompatibleType(format!(
                        "Base expression must be of type struct, but {:?} is given",
                        base_type
                    )))?,
                };

                // Get struct type information by name.
                let struct_def = self.struct_type_info.get(&struct_name).ok_or_else(|| {
                    SemanticError::NoType(format!("Type {} is not defined", &struct_name))
                })?;

                // Check if field/method exists.
                let field_exists = struct_def
                    .fields
                    .iter()
                    .any(|(_, name, _)| name == &field.name);
                let method_exists = struct_def
                    .methods
                    .iter()
                    .any(|(_, method_def)| &method_def.name == &field.name);
                if !field_exists && !method_exists {
                    return Err(SemanticError::UnboundedIdentifier(format!(
                        "Field/method {} is not defined",
                        field.name.clone()
                    )));
                }

                // Determine type of field access expression.
                let ty = if field_exists {
                    // Get field definition.
                    let (field_visibility, _field_name, field_type) = &struct_def
                        .fields
                        .iter()
                        .find(|(_, name, _)| name == &field.name)
                        .unwrap();

                    if !curr_struct_scope.is_some_and(|s| s == struct_name) {
                        if *field_visibility != Visibility::Public {
                            return Err(SemanticError::Visibility(format!(
                                "Field {} is private, for which accessing it by field access expression is not allowed",
                                &field.name
                            )));
                        }
                    }

                    // Get field type.
                    let ty = field_type.clone();

                    ty
                } else {
                    // Get method definition.
                    let (method_visibility, method_def) = &struct_def
                        .methods
                        .iter()
                        .find(|(_, method_def)| &method_def.name == &field.name)
                        .unwrap();

                    if !curr_struct_scope.is_some_and(|s| s == struct_name) {
                        if *method_visibility != Visibility::Public {
                            return Err(SemanticError::Visibility(format!(
                                "Method {} is private, for which accessing it by field access expression is not allowed",
                                &field.name
                            )));
                        }
                    }

                    // Get method type.
                    let ty =
                        self.check_function(&mut method_def.clone(), ctx, Some(&struct_name))?;

                    types::Type::Function(ty)
                };

                // Store type of node.
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::MethodAccess(ast::MethodAccessExpression { base, method_name }) => {
                let base_type = self.check_expression(base, ctx, curr_struct_scope)?;
                let struct_name = match base_type {
                    types::Type::Trivial(types::TrivialType { name }) => name,
                    _ => Err(SemanticError::IncompatibleType(format!(
                        "Base expression must be of type struct, but {:?} is given",
                        base_type
                    )))?,
                };
                let struct_info =
                    self.struct_type_info
                        .get(&struct_name)
                        .ok_or(SemanticError::NoType(format!(
                            "Type {} is not defined",
                            &struct_name
                        )))?;
                let method = struct_info
                    .methods
                    .iter()
                    .find(|(_, method_def)| &method_def.name == &method_name.name)
                    .ok_or(SemanticError::UnboundedIdentifier(format!(
                        "Method {:?} is not defined",
                        method_name.clone()
                    )))?;
                if method.0 != Visibility::Public {
                    return Err(SemanticError::Visibility(format!(
                        "Method {:?} is private, for which accessing it by method access expression is not allowed",
                        method_name.clone()
                    )));
                }
                let method_type = self.check_function(&method.1.clone(), ctx, curr_struct_scope)?;

                let ty = types::Type::Function(method_type);
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::StaticMethodAccess(ast::StaticMethodAccessExpression {
                struct_name,
                method_name,
            }) => {
                let struct_name = struct_name.name.clone();
                let method_name = method_name.name.clone();

                // Get struct type information by name.
                let struct_def =
                    self.struct_type_info
                        .get(&struct_name)
                        .ok_or(SemanticError::NoType(format!(
                            "Type {} is not defined",
                            &struct_name
                        )))?;

                // Check if method exists.
                let (visibility, method_def) = struct_def
                    .methods
                    .iter()
                    .find(|(_, method_def)| &method_def.name == &method_name)
                    .ok_or(SemanticError::UnboundedIdentifier(format!(
                        "Method {} is not defined",
                        method_name.clone()
                    )))?;

                // Check if method is public.
                if *visibility != Visibility::Public {
                    return Err(SemanticError::Visibility(format!(
                        "Method {} is private, for which accessing it by static method access expression is not allowed",
                        method_name.clone()
                    )));
                }

                // Get method type.
                let ty = self.check_function(&method_def.clone(), ctx, Some(&struct_name))?;
                let ty = types::Type::Function(ty);

                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
        }
    }
}
