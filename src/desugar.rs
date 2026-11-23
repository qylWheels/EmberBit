use crate::ast::*;

#[derive(Debug, Clone)]
struct Desugarer {}

impl Desugarer {
    pub fn new() -> Self {
        Self {}
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

    fn desugar_function(&self, function: &FunctionDefinition) -> FunctionDefinition {
        function.clone()
    }

    fn desugar_struct_def(&self, struct_def: &StructDefinition) -> StructDefinition {
        let target_type = Type::Trivial(TrivialType {
            name: struct_def.name.clone(),
        });

        // Desugar fields.
        let desugared_fields = struct_def
            .fields
            .iter()
            .map(|(visibility, name, ty)| match ty {
                Type::Trivial(TrivialType { name }) if name == "Self" => {
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
                        Type::Trivial(TrivialType { name }) if name == "Self" => {
                            (*mutability, name.clone(), target_type.clone())
                        }
                        _ => (*mutability, name.clone(), ty.clone()),
                    })
                    .collect::<Vec<_>>();

                // Desugar method return type.
                let desugared_return_type = match &method.return_type {
                    Type::Trivial(TrivialType { name }) if name == "Self" => target_type.clone(),
                    _ => method.return_type.clone(),
                };

                (
                    *visibility,
                    FunctionDefinition {
                        name: method.name.clone(),
                        params: desugared_params,
                        return_type: desugared_return_type,
                        body: method.body.clone(),
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
}
