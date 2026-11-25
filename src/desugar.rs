use crate::ast::*;
use crate::semantic;
use crate::types;

#[derive(Debug, Clone)]
pub struct Desugarer<'a> {
    semantic_checker: &'a semantic::SemanticChecker,
}

impl<'a> Desugarer<'a> {
    pub fn new(semantic_checker: &'a semantic::SemanticChecker) -> Self {
        Self { semantic_checker }
    }

    pub fn desugar_module(&self, module: &Module) -> Module {
        let mut desugared_module = Module {
            top_definitions: Vec::new(),
        };
        for def in &module.top_definitions {
            let desugared_def = match def {
                TopDefinition::Import(i) => TopDefinition::Import(self.desugar_import(i)),
                TopDefinition::Struct(s) => TopDefinition::Struct(self.desugar_struct_def(s)),
                TopDefinition::Function(f) => TopDefinition::Function(self.desugar_function(f)),
            };
            desugared_module.top_definitions.push(desugared_def);
        }
        desugared_module
    }

    pub fn desugar_import(&self, import: &ImportDefinition) -> ImportDefinition {
        import.clone()
    }

    pub fn desugar_function(&self, func: &FunctionDefinition) -> FunctionDefinition {
        let desugared_stmts = func
            .body
            .iter()
            .map(|stmt| self.desugar_statement(stmt))
            .collect::<Vec<_>>();

        FunctionDefinition {
            name: func.name.clone(),
            params: func.params.clone(),
            return_type_mutability: func.return_type_mutability.clone(),
            return_type: func.return_type.clone(),
            body: desugared_stmts,
        }
    }

    pub fn desugar_struct_def(&self, struct_def: &StructDefinition) -> StructDefinition {
        let target_type = types::Type::Trivial(types::TrivialType {
            name: struct_def.name.clone(),
        });

        // Desugar fields.
        let desugared_fields = struct_def
            .fields
            .iter()
            .map(|(visibility, name, ty)| match ty {
                types::Type::Trivial(types::TrivialType { name: type_name })
                    if type_name == "Self" =>
                {
                    (*visibility, name.clone(), target_type.clone())
                }
                _ => (*visibility, name.clone(), ty.clone()),
            })
            .collect::<Vec<_>>();

        // Desugar methods.
        let desugared_methods = struct_def
            .methods
            .iter()
            .map(|(visibility, method)| {
                // Desugar method parameters.
                let desugared_params = method
                    .params
                    .iter()
                    .map(|(mutability, name, ty)| match ty {
                        types::Type::Trivial(types::TrivialType { name: type_name })
                            if type_name == "Self" =>
                        {
                            (*mutability, name.clone(), target_type.clone())
                        }
                        _ => (*mutability, name.clone(), ty.clone()),
                    })
                    .collect::<Vec<_>>();

                // Desugar method return type.
                let desugared_return_type = match &method.return_type {
                    types::Type::Trivial(types::TrivialType { name }) if name == "Self" => {
                        target_type.clone()
                    }
                    _ => method.return_type.clone(),
                };

                // Desugar statements.
                let desugared_body = method
                    .body
                    .iter()
                    .map(|stmt| self.desugar_statement(stmt))
                    .collect::<Vec<_>>();

                (
                    visibility.clone(),
                    FunctionDefinition {
                        name: method.name.clone(),
                        params: desugared_params,
                        return_type_mutability: method.return_type_mutability.clone(),
                        return_type: desugared_return_type,
                        body: desugared_body,
                    },
                )
            })
            .collect::<Vec<_>>();

        StructDefinition {
            name: struct_def.name.clone(),
            fields: desugared_fields,
            methods: desugared_methods,
        }
    }

    pub fn desugar_statement(&self, stmt: &Statement) -> Statement {
        match stmt {
            Statement::ConstDef(ConstDefStatement { name, ty, expr }) => {
                Statement::ConstDef(ConstDefStatement {
                    name: name.clone(),
                    ty: ty.clone(),
                    expr: self.desugar_expression(expr),
                })
            }
            Statement::VarDef(VarDefStatement { name, ty, expr }) => {
                Statement::VarDef(VarDefStatement {
                    name: name.clone(),
                    ty: ty.clone(),
                    expr: self.desugar_expression(expr),
                })
            }
            Statement::Assignment(AssignmentStatement { lhs, rhs }) => {
                Statement::Assignment(AssignmentStatement {
                    lhs: self.desugar_expression(lhs),
                    rhs: self.desugar_expression(rhs),
                })
            }
            Statement::Return(ReturnStatement { expr }) => Statement::Return(ReturnStatement {
                expr: self.desugar_expression(expr),
            }),
            Statement::Expression(ExpressionStatement { expr }) => {
                Statement::Expression(ExpressionStatement {
                    expr: self.desugar_expression(expr),
                })
            }
        }
    }

    pub fn desugar_expression(&self, expr: &Expression) -> Expression {
        match expr {
            Expression::Array(ArrayExpression { elements }) => Expression::Array(ArrayExpression {
                elements: elements
                    .iter()
                    .map(|element| self.desugar_expression(element))
                    .collect(),
            }),
            Expression::Tuple(TupleExpression { elements }) => Expression::Tuple(TupleExpression {
                elements: elements
                    .iter()
                    .map(|element| self.desugar_expression(element))
                    .collect(),
            }),
            Expression::Binary(BinaryExpression { lhs, op, rhs }) => {
                Expression::Binary(BinaryExpression {
                    lhs: Box::new(self.desugar_expression(lhs)),
                    op: op.clone(),
                    rhs: Box::new(self.desugar_expression(rhs)),
                })
            }
            Expression::Slice(SliceExpression { base, start, end }) => {
                Expression::Slice(SliceExpression {
                    base: Box::new(self.desugar_expression(base)),
                    start: Box::new(self.desugar_expression(start)),
                    end: Box::new(self.desugar_expression(end)),
                })
            }
            Expression::Index(IndexExpression { base, index }) => {
                Expression::Index(IndexExpression {
                    base: Box::new(self.desugar_expression(base)),
                    index: Box::new(self.desugar_expression(index)),
                })
            }
            Expression::Call(CallExpression { callee, args }) => {
                let desugared_callee = self.desugar_expression(callee);
                let mut desugared_args = args
                    .iter()
                    .map(|arg| self.desugar_expression(arg))
                    .collect::<Vec<_>>();

                // If the callee is a method of a struct, the receiver becomes the first
                // argument. The type is looked up on the *original* base node, because
                // `type_of_node` is keyed by the address of the checked AST nodes.
                if let (
                    Expression::FieldAccess(FieldAccessExpression {
                        base: original_base,
                        field,
                    }),
                    Expression::FieldAccess(FieldAccessExpression {
                        base: desugared_base,
                        ..
                    }),
                ) = (&**callee, &desugared_callee)
                {
                    let base_type = self
                        .semantic_checker
                        .get_type_of_node(&**original_base)
                        .unwrap();
                    let struct_name = match base_type {
                        types::Type::Trivial(types::TrivialType { name }) => name,
                        _ => unreachable!(),
                    };
                    let struct_info = self
                        .semantic_checker
                        .get_struct_type_info(struct_name)
                        .unwrap();
                    let is_method = struct_info
                        .methods
                        .iter()
                        .any(|(_, func_def)| &func_def.name == &field.name);
                    if is_method {
                        desugared_args.insert(0, desugared_base.as_ref().clone());
                    }
                }

                Expression::Call(CallExpression {
                    callee: Box::new(desugared_callee),
                    args: desugared_args,
                })
            }
            Expression::Struct(StructExpression { name, fields }) => {
                Expression::Struct(StructExpression {
                    name: name.clone(),
                    fields: fields
                        .iter()
                        .map(|(field_name, expr)| {
                            (field_name.clone(), self.desugar_expression(expr))
                        })
                        .collect(),
                })
            }
            Expression::FieldAccess(FieldAccessExpression { base, field }) => {
                Expression::FieldAccess(FieldAccessExpression {
                    base: Box::new(self.desugar_expression(base)),
                    field: field.clone(),
                })
            }
            Expression::Integer(_) | Expression::Identifier(_) => expr.clone(),
        }
    }
}
