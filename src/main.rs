mod ast;
mod semantic;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(grammar);

fn main() {
    let src = r#"
        import foo.bar;

        struct Point { x: i32; y: i32; }
        
        function main() i32 {
            var a: i32 = 1;
            const b: i32 = 2;
            const arr: [5]i32 = [1, 2, 3, 4, 5];
            const slice: []i32 = arr[2..4];
            return a + b * 3;
        }
    "#;

    let module = grammar::ModuleParser::new().parse(src).unwrap();
    let mut checker = semantic::SemanticChecker::new();
    checker.check_module(&module).unwrap();
}
