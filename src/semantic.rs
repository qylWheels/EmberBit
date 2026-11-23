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
    ArgumentType(Vec<SemanticError>),
    Visibility(String),
    DuplicateField(String),
}

pub type Result<T> = std::result::Result<T, SemanticError>;

type Type = ast::Type;
type TrivialType = ast::TrivialType;
type ArrayType = ast::ArrayType;
type ConstSliceType = ast::SliceType;
type TupleType = ast::TupleType;
type FunctionType = ast::FunctionType;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionTypeVariant {
    Normal,
    Method,
    Associated,
}
type Visibility = ast::Visibility;

#[derive(Debug, Clone)]
pub struct IdentifierProperties {
    pub ty: Type,
    pub variable: bool,
    pub func_type: Option<FunctionTypeVariant>,
}

pub struct SemanticChecker {
    /// usize: address of AST node
    type_of_node: HashMap<usize, Type>,

    /// Struct type information.
    struct_type_info: HashMap<String, ast::StructDefinition>,
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
            struct_type_info: HashMap::new(),
        }
    }

    pub fn get_type_of_node<T>(&self, node: &T) -> Option<&Type> {
        self.type_of_node.get(&(node as *const T as usize))
    }

    pub fn type_annotation_to_type(&self, type_annotation: &ast::Type) -> Result<Type> {
        Ok(type_annotation.clone())
    }

    fn desugar_self_type_in_struct_def(&mut self, struct_def: &mut ast::StructDefinition) {
        let target_type = Type::Trivial(TrivialType {
            name: struct_def.name.clone(),
        });

        // Desugar fields.
        for field in &mut struct_def.fields {
            if let Type::Trivial(TrivialType { name }) = &field.2
                && name == "Self"
            {
                field.2 = target_type.clone();
            }
        }

        // Desugar methods.
        for method in &mut struct_def.methods {
            for param in &mut method.1.params {
                if let Type::Trivial(TrivialType { name }) = &param.2
                    && name == "Self"
                {
                    param.2 = target_type.clone();
                }
            }
        }
    }

    pub fn check_module(&mut self, ast: &mut ast::Module) -> Result<()> {
        // Collect all top definitions into context first.
        let mut ctx = HashMap::new();
        for topdef in &mut ast.top_definitions {
            match topdef {
                ast::TopDefinition::Import(_i) => {}
                ast::TopDefinition::Struct(s) => {
                    // Desugar all `Self` occurrences to struct type.
                    self.desugar_self_type_in_struct_def(s);

                    // Insert struct type into context.
                    self.struct_type_info.insert(s.name.clone(), s.clone());
                }
                ast::TopDefinition::Function(f) => {
                    let param_mutabilities = f
                        .params
                        .iter()
                        .map(|(mutability, _, _)| mutability.clone())
                        .collect::<Vec<_>>();
                    let annotated_param_types: Vec<Type> = f
                        .params
                        .iter()
                        .map(|(_, _, ty)| self.type_annotation_to_type(ty))
                        .collect::<Result<Vec<_>>>()?;
                    let annotated_return_type = self.type_annotation_to_type(&f.return_type)?;
                    let func_type = Type::Function(ast::FunctionType {
                        params: param_mutabilities
                            .iter()
                            .zip(annotated_param_types.iter())
                            .map(|(mutability, ty)| (mutability.clone(), ty.clone()))
                            .collect(),
                        return_type: Box::new(annotated_return_type.clone()),
                    });
                    ctx.insert(
                        f.name.clone(),
                        IdentifierProperties {
                            ty: func_type.clone(),
                            variable: false,
                            func_type: Some(FunctionTypeVariant::Normal),
                        },
                    );
                }
            }
        }

        for topdef in &mut ast.top_definitions {
            match topdef {
                ast::TopDefinition::Import(i) => self.check_import(i)?,
                ast::TopDefinition::Struct(s) => self.check_struct(s)?,
                ast::TopDefinition::Function(f) => {
                    self.check_function(f, &ctx)?;
                }
            }
        }
        Ok(())
    }

    pub fn check_import(&mut self, _ast: &ast::ImportDefinition) -> Result<()> {
        Ok(())
    }

    pub fn check_struct(&mut self, struct_def: &mut ast::StructDefinition) -> Result<()> {
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
            let mut ctx = HashMap::new();
            for param in &method.1.params {
                // Add parameter to context.
                ctx.insert(
                    param.1.clone(),
                    IdentifierProperties {
                        ty: param.2.clone(),
                        variable: param.0 == ast::Mutability::Var,
                        func_type: None,
                    },
                );
            }

            self.check_function(&method.1, &ctx)?;
        }

        Ok(())
    }

    pub fn check_function(
        &mut self,
        ast: &ast::FunctionDefinition,
        ctx: &HashMap<String, IdentifierProperties>,
    ) -> Result<ast::FunctionType> {
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

        // Check parameter types.
        let annotated_param_types: Vec<Type> = ast
            .params
            .iter()
            .map(|(_, _, ty)| self.type_annotation_to_type(ty))
            .collect::<Result<Vec<_>>>()?;
        let annotated_return_type = self.type_annotation_to_type(&ast.return_type)?;

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
                    variable: *mutability == ast::Mutability::Var,
                    func_type: None,
                },
            );
        }

        for stmt in &ast.body {
            curr_ctx = self.check_statement(stmt, &curr_ctx)?;
        }

        let last_stmt = ast
            .body
            .last()
            .ok_or_else(|| SemanticError::IncompatibleType("Empty function body".into()))?;

        let actual_return_type =
            if let ast::Statement::Return(ast::ReturnStatement { expr }) = last_stmt {
                self.check_expression(expr, &curr_ctx)?
            } else {
                Type::Tuple(ast::TupleType { elem_types: vec![] })
            };

        if annotated_return_type != actual_return_type {
            return Err(SemanticError::IncompatibleType(format!(
                "Function {} has incompatible return types {:?} (annotation) and {:?} (actual)",
                ast.name, annotated_return_type, actual_return_type
            )));
        }

        let ty = FunctionType {
            params: params
                .iter()
                .map(|(mutability, _, ty)| (mutability.clone(), ty.clone()))
                .collect(),
            return_type: Box::new(actual_return_type),
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
                        func_type: None,
                    },
                );
                Ok(new_ctx)
            }
            ast::Statement::VarDef(ast::VarDefStatement { name, ty, expr }) => {
                // It is not allowed to create variable reference on not-assignable expression.
                let expr_assignable = match expr {
                    ast::Expression::Slice(ast::SliceExpression {
                        base,
                        start: _,
                        end: _,
                    }) => self.check_expression_assignable(base, ctx)?,
                    ast::Expression::Index(ast::IndexExpression { base, index: _ }) => {
                        self.check_expression_assignable(base, ctx)?
                    }
                    ast::Expression::Identifier(_) => {
                        self.check_expression_assignable(expr, ctx)?
                    }
                    ast::Expression::FieldAccess(_) => {
                        self.check_expression_assignable(expr, ctx)?
                    }
                    _ => true,
                };
                if !expr_assignable {
                    return Err(SemanticError::Variable(
                        "Variable reference on non-assignable expression".into(),
                    ));
                }

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
                        func_type: None,
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
                    Type::Array(_) | Type::Slice(_) => {
                        self.check_expression_assignable(base, ctx)?
                    }
                    _ => false,
                };
                return Ok(variable);
            }
            ast::Expression::FieldAccess(ast::FieldAccessExpression { base, field }) => {
                // Check if base is assignable.
                if !self.check_expression_assignable(base, ctx)? {
                    return Ok(false);
                }

                let base_type = self.check_expression(base, ctx)?;

                // Get struct actual type.
                let struct_type = match &base_type {
                    Type::Trivial(ty) => self.struct_type_info.get(&ty.name).ok_or(
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
                // TODO: It is ok to visit private fields in struct methods.
                if field_visibility_and_type.0 != Visibility::Public {
                    return Err(SemanticError::Visibility(format!(
                        "Field {} is not visible outside struct definition",
                        field.name
                    )));
                }

                return Ok(true);
            }
            _ => Ok(false),
        }
    }

    fn is_method_of_struct(
        &mut self,
        expr: &ast::Expression,
        ctx: &HashMap<String, IdentifierProperties>,
    ) -> Result<bool> {
        match expr {
            ast::Expression::FieldAccess(ast::FieldAccessExpression { base, field }) => {
                let base_type = self.check_expression(base, ctx)?;
                let struct_type = match &base_type {
                    Type::Trivial(ty) => self.struct_type_info.get(&ty.name).ok_or(
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
        ast: &mut ast::Expression,
        ctx: &HashMap<String, IdentifierProperties>,
    ) -> Result<Type> {
        let key = ast as *const ast::Expression as usize;
        match ast {
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
                    Type::Slice(ast::SliceType { elem_type, .. }) => *elem_type,
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
            ast::Expression::Slice(ast::SliceExpression { base, start, end }) => {
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

                let ty = Type::Slice(ast::SliceType {
                    elem_type: Box::new(elem_type),
                });

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
            ast::Expression::Call(ast::CallExpression { callee, args }) => {
                let callee_type = self.check_expression(callee, ctx)?;
                match callee_type {
                    Type::Function(func_type) => {
                        // If callee is method of struct, desugar it.
                        if self.is_method_of_struct(callee, ctx)? {
                            match &**callee {
                                ast::Expression::FieldAccess(ast::FieldAccessExpression {
                                    base,
                                    field,
                                }) => {
                                    args.insert(0, (**base).clone());
                                }
                                _ => unreachable!(),
                            }
                        }

                        // Infer parameter types.
                        let actual_param_types = args
                            .iter()
                            .map(|arg| self.check_expression(arg, ctx))
                            .collect::<Result<Vec<_>>>()?;

                        // Check if the number of arguments matches the number of parameters.
                        if actual_param_types.len() != func_type.params.len() {
                            return Err(SemanticError::IncompatibleType(format!(
                                "Function has {} parameters, but {} arguments are given",
                                func_type.params.len(),
                                args.len()
                            )));
                        }

                        // Check mutability of parameters.
                        let annotated_mutabilities = func_type
                            .params
                            .iter()
                            .map(|(mutability, _)| *mutability == ast::Mutability::Var)
                            .collect::<Vec<_>>();
                        let param_mutabilities = args
                            .iter()
                            .map(|arg| self.check_expression_assignable(arg, ctx))
                            .collect::<Result<Vec<_>>>()?;
                        for i in 0..annotated_mutabilities.len() {
                            if annotated_mutabilities[i] == true && param_mutabilities[i] == false {
                                return Err(SemanticError::Variable(format!(
                                    "Parameter {} requires mutable, but a constant is given",
                                    i
                                )));
                            }
                        }

                        // Check if the parameter types match.
                        if actual_param_types
                            != func_type
                                .params
                                .iter()
                                .map(|(_, ty)| ty.clone())
                                .collect::<Vec<_>>()
                        {
                            return Err(SemanticError::IncompatibleType(format!(
                                "Function has incompatible parameter types: {:?}(declared) vs {:?}(given)",
                                &actual_param_types, &func_type.params
                            )));
                        }

                        Ok((*func_type.return_type).clone())
                    }
                    other => {
                        return Err(SemanticError::IncompatibleType(format!(
                            "Callee must be of type Function, but {:?} is given",
                            other
                        )));
                    }
                }
            }
            ast::Expression::Struct(ast::StructExpression { name, fields }) => {
                // Get struct type information by name.
                let struct_def = self.struct_type_info.get(name).ok_or_else(|| {
                    SemanticError::NoType(format!("Type {} is not defined", name))
                })?;
                let struct_name = struct_def.name.clone();
                let struct_fields = struct_def.fields.clone();

                // If any field is private, it is not allowed to construct struct object
                // by struct expression.
                for (visibility, _, _) in &struct_fields {
                    if *visibility != Visibility::Public {
                        return Err(SemanticError::Visibility(format!(
                            "Struct {} has private fields, for which constructing struct object by struct expression is not allowed",
                            struct_name
                        )));
                    }
                }

                // Infer type of each field expression.
                let field_types = fields
                    .iter()
                    .map(|(_, expr)| self.check_expression(expr, ctx))
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

                let ty = Type::Trivial(TrivialType { name: struct_name });
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
            ast::Expression::FieldAccess(ast::FieldAccessExpression { base, field }) => {
                // Check type of base expression.
                let base_type = self.check_expression(base, ctx)?;
                let struct_name = match base_type {
                    Type::Trivial(TrivialType { name }) => name,
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

                    // TODO: Allow accessing private fields in struct methods.
                    if *field_visibility != Visibility::Public {
                        return Err(SemanticError::Visibility(format!(
                            "Field {} is private, for which accessing it by field access expression is not allowed",
                            &field.name
                        )));
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

                    // TODO: Allow accessing private methods in struct methods.
                    if *method_visibility != Visibility::Public {
                        return Err(SemanticError::Visibility(format!(
                            "Method {} is private, for which accessing it by field access expression is not allowed",
                            &field.name
                        )));
                    }

                    // Get method type.
                    let ty = self.check_function(&method_def.clone(), ctx)?;

                    Type::Function(ty)
                };

                // Store type of node.
                self.type_of_node.insert(key, ty.clone());
                Ok(ty)
            }
        }
    }
}
