use crate::ast::*;
use crate::semantic;
use crate::types;

#[derive(Debug, Clone)]
struct Desugarer<'a> {
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

    fn desugar_import(&self, import: &ImportDefinition) -> ImportDefinition {
        import.clone()
    }

    fn desugar_function(&self, func: &FunctionDefinition) -> FunctionDefinition {
        let desugared_stmts = func
            .body
            .iter()
            .map(|stmt| self.desugar_statement(stmt))
            .collect::<Vec<_>>();

        FunctionDefinition {
            name: func.name.clone(),
            params: func.params.clone(),
            return_type: func.return_type.clone(),
            body: desugared_stmts,
        }
    }

    fn desugar_struct_def(&self, struct_def: &StructDefinition) -> StructDefinition {
        let target_type = types::Type::Trivial(types::TrivialType {
            name: struct_def.name.clone(),
        });

        // Desugar fields.
        let desugared_fields = struct_def
            .fields
            .iter()
            .map(|(visibility, name, ty)| match ty {
                types::Type::Trivial(types::TrivialType { name }) if name == "Self" => {
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
                        types::Type::Trivial(types::TrivialType { name }) if name == "Self" => {
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

    fn desugar_statement(&self, stmt: &Statement) -> Statement {
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

    fn desugar_expression(&self, expr: &Expression) -> Expression {
        match expr {
            Expression::Call(CallExpression { callee, args }) => match &**callee {
                Expression::FieldAccess(FieldAccessExpression { base, field }) => {
                    let ty = self.semantic_checker.get_type_of_node(base).unwrap();
                    let struct_name = match ty {
                        types::Type::Trivial(types::TrivialType { name }) => name,
                        _ => unreachable!(),
                    };
                    let struct_info = self
                        .semantic_checker
                        .get_struct_type_info(&struct_name)
                        .unwrap();
                    let field_name = &field.name;
                    let method = struct_info
                        .methods
                        .iter()
                        .find(|(_, func_def)| &func_def.name == field_name);
                    if method.is_some() {
                        let mut new_args = args.clone();
                        new_args.insert(0, *base.clone());
                        Expression::Call(CallExpression {
                            callee: callee.clone(),
                            args: new_args,
                        })
                    } else {
                        expr.clone()
                    }
                }
                _ => expr.clone(),
            },
            other => other.clone(),
        }
    }
}
