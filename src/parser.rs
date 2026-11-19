use crate::ast;

use winnow::Parser;
use winnow::Result;
use winnow::ascii::*;
use winnow::combinator::*;
use winnow::token::*;

macro_rules! ws {
    ($inner:expr) => {
        delimited(multispace0, $inner, multispace0)
    };
}

pub fn parse_module<'a>(input: &mut &'a str) -> Result<ast::Module> {
    repeat(0.., ws!(parse_function))
        .map(|funcs: Vec<ast::FunctionDefinition>| {
            let topdefs = funcs
                .iter()
                .map(|f| ast::TopDefinition::Function(f.clone()))
                .collect::<Vec<_>>();
            ast::Module {
                top_definitions: topdefs,
            }
        })
        .parse_next(input)
}

fn parse_struct_definition<'a>(input: &mut &'a str) -> Result<ast::StructDefinition> {
    let parse_field_defs = separated(0.., (ws!(parse_identifier), ':', ws!(parse_type)), ';').map(
        |fields: Vec<(ast::Expression, _, ast::Type)>| {
            fields
                .iter()
                .map(|(id, _, ty)| {
                    let id = match id {
                        ast::Expression::Identifier(id) => id.name.clone(),
                        _ => panic!("Expected identifier in struct definition"),
                    };
                    (id, ty.clone())
                })
                .collect::<Vec<_>>()
        },
    );

    let parse_struct_def = (
        "struct",
        space1,
        ws!(parse_identifier),
        '{',
        ws!(parse_field_defs),
        '}',
    )
        .map(|(_, _, id, _, fields, _)| {
            let id = match id {
                ast::Expression::Identifier(id) => id.name,
                _ => panic!("Expected identifier in struct definition"),
            };
            ast::StructDefinition { name: id, fields }
        });
    alt((parse_struct_def,)).parse_next(input)
}

fn parse_function<'a>(input: &mut &'a str) -> Result<ast::FunctionDefinition> {
    let parse_function_def = (
        "func",
        space1,
        ws!(parse_identifier),
        '(',
        ws!(separated(
            0..,
            (ws!(parse_identifier), ':', ws!(parse_type)),
            ','
        ))
        .map(|args: Vec<(ast::Expression, _, ast::Type)>| {
            args.iter()
                .map(|(id, _, ty)| {
                    let id = match id {
                        ast::Expression::Identifier(id) => id.name.clone(),
                        _ => panic!("Expected identifier in function definition"),
                    };
                    (id, ty.clone())
                })
                .collect::<Vec<_>>()
        }),
        ')',
        ws!(parse_type),
        '{',
        ws!(repeat(0.., ws!(parse_statement))),
        '}',
    )
        .map(|(_, _, id, _, params, _, ret_ty, _, body, _)| {
            let id = match id {
                ast::Expression::Identifier(id) => id.name,
                _ => panic!("Expected identifier in function definition"),
            };
            ast::FunctionDefinition {
                name: id,
                params,
                return_type: ret_ty,
                body,
            }
        });

    alt((parse_function_def,)).parse_next(input)
}

fn parse_statement<'a>(input: &mut &'a str) -> Result<ast::Statement> {
    let parse_const_statement = (
        "const",
        space1,
        ws!(parse_identifier),
        ':',
        ws!(parse_type),
        '=',
        ws!(parse_expression),
        ';',
    )
        .map(|(_, _, id, _, ty, _, expr, _)| {
            let id = match id {
                ast::Expression::Identifier(id) => id.name,
                _ => panic!("Expected identifier in lhs of const statement"),
            };
            ast::Statement::ConstDef(ast::ConstDefStatement { name: id, ty, expr })
        });

    let parse_var_statement = (
        "var",
        space1,
        ws!(parse_identifier),
        ':',
        ws!(parse_type),
        '=',
        ws!(parse_expression),
        ';',
    )
        .map(|(_, _, id, _, ty, _, expr, _)| {
            let id = match id {
                ast::Expression::Identifier(id) => id.name,
                _ => panic!("Expected identifier in lhs of var statement"),
            };
            ast::Statement::VarDef(ast::VarDefStatement { name: id, ty, expr })
        });

    let parse_assignment_statement = (ws!(parse_expression), '=', ws!(parse_expression), ';')
        .map(|(lhs, _, rhs, _)| ast::Statement::Assignment(ast::AssignmentStatement { lhs, rhs }));

    let parse_return_statement = ("return", space1, parse_expression, ';')
        .map(|(_, _, expr, _)| ast::Statement::Return(ast::ReturnStatement { expr }));

    let parse_expression_statement = (parse_expression, ';')
        .map(|(expr, _)| ast::Statement::Expression(ast::ExpressionStatement { expr }));

    alt((
        parse_const_statement,
        parse_var_statement,
        parse_assignment_statement,
        parse_return_statement,
        parse_expression_statement,
    ))
    .parse_next(input)
}

fn parse_type<'a>(input: &mut &'a str) -> Result<ast::Type> {
    let parse_trivial_type =
        parse_available_name.map(|s: String| ast::Type::Trivial(ast::TrivialType { name: s }));
    let parse_function_type = (
        '(',
        ws!(separated(0.., ws!(parse_type), ',')),
        ')',
        parse_type,
    )
        .map(|(_, types, _, return_type)| {
            ast::Type::Function(ast::FunctionType {
                param_types: types,
                return_type: Box::new(return_type),
            })
        });
    alt((parse_function_type, parse_trivial_type)).parse_next(input)
}

fn parse_expression<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    let prefix_parser = dispatch! {any;
        _ => fail
    };
    let infix_parser = dispatch! {any;
        '+' => Infix::Left(24, |_, lhs: ast::Expression, rhs: ast::Expression| {
            Ok(ast::Expression::Binary(ast::BinaryExpression {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                op: ast::BinaryOp::Add,
            }))
        }),
        '-' => Infix::Left(24, |_, lhs: ast::Expression, rhs: ast::Expression| {
            Ok(ast::Expression::Binary(ast::BinaryExpression {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                op: ast::BinaryOp::Sub,
            }))
        }),
        '*' => Infix::Left(28, |_, lhs: ast::Expression, rhs: ast::Expression| {
            Ok(ast::Expression::Binary(ast::BinaryExpression {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                op: ast::BinaryOp::Mul,
            }))
        }),
        '/' => Infix::Left(28, |_, lhs: ast::Expression, rhs: ast::Expression| {
            Ok(ast::Expression::Binary(ast::BinaryExpression {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                op: ast::BinaryOp::Div,
            }))
        }),
        _ => fail
    };
    let postfix_parser = dispatch! {any;
        '[' => Postfix(32, |content: &mut &str, base: ast::Expression| {
            let parse_index = parse_expression
                .map(|e: ast::Expression|
                    ast::Expression::Index(
                        ast::IndexExpression {
                            base: Box::new(base.clone()),
                            index: Box::new(e)
                        }
                    )
                );
            let parse_range = (parse_expression, ':', parse_expression)
                .map(|(start, _, end): (ast::Expression, _, ast::Expression)|
                    ast::Expression::Slice(
                        ast::SliceExpression {
                            base: Box::new(base.clone()),
                            start: Box::new(start.clone()),
                            end: Box::new(end.clone())
                        }
                    )
                );
            (alt((parse_range, parse_index)), ']').map(|(e, _)| e).parse_next(content)
        }),
        '(' => Postfix(31, |input: &mut &str, base: ast::Expression| {
            (separated(0.., parse_expression, ','), ')')
                .map(|(args, _)| ast::Expression::Call(ast::CallExpression {
                    callee: Box::new(base.clone()),
                    args
                }))
                .parse_next(input)
        }),
        _ => fail
    };

    expression(ws!(parse_atom_expression))
        .prefix(prefix_parser)
        .infix(infix_parser)
        .postfix(postfix_parser)
        .parse_next(input)
}

fn parse_atom_expression<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    let parse_paren_expr = delimited('(', ws!(parse_expression), ')');
    alt((parse_identifier, parse_integer, parse_paren_expr)).parse_next(input)
}

fn parse_identifier<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    parse_available_name
        .map(|s| ast::Expression::Identifier(ast::Identifier { name: s }))
        .parse_next(input)
}

fn parse_integer<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    (opt('-'), one_of('0'..='9'), digit0)
        .map(
            |(sign, first_digit, rest_digits): (Option<char>, char, _)| {
                let prefix = if matches!(sign, Some(_)) {
                    "-".to_string()
                } else {
                    "".to_string()
                };
                format!("{}{}{}", prefix, first_digit, rest_digits)
                    .parse::<i64>()
                    .unwrap()
            },
        )
        .map(|i| ast::Expression::Integer(ast::IntegerLiteral { value: i }))
        .parse_next(input)
}

fn parse_available_name<'a>(input: &mut &'a str) -> Result<String> {
    (one_of(('a'..='z', 'A'..='Z', '_')), alphanumeric0)
        .map(|(first, rest)| format!("{}{}", first, rest))
        .parse_next(input)
}
