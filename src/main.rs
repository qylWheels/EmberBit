mod ast;
mod desugar;
mod interpret;
mod parser;
mod semantic;
mod types;

fn main() {
    let mut src = include_str!("test.ebt");
    let mut module = parser::parse_module(&mut src).unwrap();
    let mut checker = semantic::SemanticChecker::new();
    checker.check_module(&mut module).unwrap();
    let desugarer = desugar::Desugarer::new(&checker);
    let desugared_module = desugarer.desugar_module(&module);
    let mut interpreter = interpret::Interpreter::new();
    let result = interpreter.interpret_module(&desugared_module).unwrap();
    println!("{:?}", result);
}
