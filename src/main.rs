mod ast;
mod interpret;
mod semantic;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(grammar);

fn main() {
    let src = include_str!("test.ebt");

    let module = grammar::ModuleParser::new().parse(src).unwrap();
    let mut checker = semantic::SemanticChecker::new();
    checker.check_module(&module).unwrap();
    let mut interpreter = interpret::Interpreter::new();
    let result = interpreter.interpret_module(&module).unwrap();
    println!("{:?}", result);
}
