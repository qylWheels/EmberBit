mod ast;
mod interpret;
mod parser;
mod semantic;

fn main() {
    let mut src = include_str!("test.ebt");
    let mut module = parser::parse_module(&mut src).unwrap();
    let mut checker = semantic::SemanticChecker::new();
    checker.check_module(&mut module).unwrap();
    let mut interpreter = interpret::Interpreter::new();
    let result = interpreter.interpret_module(&module).unwrap();
    println!("{:?}", result);
}
