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
        struct_def.clone()
    }
}
