use crate::ast;
use crate::types;

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

macro_rules! lws {
    ($inner:expr) => {
        preceded(multispace0, $inner)
    };
}

pub fn parse_module<'a>(input: &mut &'a str) -> Result<ast::Module> {
    enum TopDefinition {
        Struct(ast::StructDefinition),
        Function(ast::FunctionDefinition),
    }

    repeat(
        0..,
        ws!(alt((
            parse_function.map(TopDefinition::Function),
            parse_struct_definition.map(TopDefinition::Struct)
        ))),
    )
    .map(|topdefs: Vec<TopDefinition>| {
        let topdefs = topdefs
            .iter()
            .map(|topdef| match topdef {
                TopDefinition::Function(f) => ast::TopDefinition::Function(f.clone()),
                TopDefinition::Struct(s) => ast::TopDefinition::Struct(s.clone()),
            })
            .collect::<Vec<_>>();
        ast::Module {
            top_definitions: topdefs,
        }
    })
    .parse_next(input)
}

fn parse_struct_definition<'a>(input: &mut &'a str) -> Result<ast::StructDefinition> {
    let parse_field_def = (parse_visibility, multispace1, parse_id_and_type)
        .map(|(visibility, _, (id, ty))| (visibility.clone(), id.clone(), ty.clone()));
    let parse_method_def = (parse_visibility, multispace1, parse_function)
        .map(|(visibility, _, method)| (visibility.clone(), method.clone()));

    let parse_struct_def = (
        "struct",
        multispace1,
        ws!(parse_available_name),
        '{',
        ws!(repeat(0.., ws!(terminated(ws!(parse_field_def), ';')))),
        ws!(repeat(0.., ws!(parse_method_def))),
        '}',
    )
        .map(|(_, _, id, _, fields, methods, _)| ast::StructDefinition {
            name: id.clone(),
            fields,
            methods,
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
            (
                ws!(parse_mutability),
                ws!(parse_available_name),
                ':',
                ws!(parse_type)
            ),
            ','
        ))
        .map(|args: Vec<(types::Mutability, String, _, types::Type)>| {
            args.iter()
                .map(|(mutability, id, _, ty)| (mutability.clone(), id.clone(), ty.clone()))
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

fn parse_type<'a>(input: &mut &'a str) -> Result<types::Type> {
    let parse_trivial_type =
        parse_available_name.map(|s: String| types::Type::Trivial(types::TrivialType { name: s }));
    let parse_function_type = (
        '(',
        ws!(separated(
            0..,
            (lws!(parse_mutability), multispace1, ws!(parse_type)),
            ','
        )),
        ')',
        ws!(parse_type),
    )
        .map(
            |(_, params, _, return_type): (
                _,
                Vec<(types::Mutability, _, types::Type)>,
                _,
                types::Type,
            )| {
                let param_mutabilities_and_types = params
                    .iter()
                    .map(|(mutability, _, ty)| (mutability.clone(), ty.clone()))
                    .collect::<Vec<_>>();
                types::Type::Function(types::FunctionType {
                    params: param_mutabilities_and_types,
                    return_type: Box::new(return_type),
                })
            },
        );
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
        '.' => Postfix(30, |input: &mut &str, base: ast::Expression| {
            parse_identifier
                .map(|expr| {
                    let id = match expr {
                        ast::Expression::Identifier(id) => id.clone(),
                        _ => panic!("Expect identifier, but got {:?}", expr),
                    };
                    ast::Expression::FieldAccess(ast::FieldAccessExpression {
                        base: Box::new(base.clone()),
                        field: id,
                    })
                })
                .parse_next(input)
        }),
        _ => fail
    };

    expression(ws!(alt((parse_struct_expr, parse_atom_expression))))
        .prefix(prefix_parser)
        .infix(infix_parser)
        .postfix(postfix_parser)
        .parse_next(input)
}

fn parse_struct_expr<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    (
        ws!(parse_available_name),
        '{',
        ws!(separated(0.., ws!(parse_id_and_expr), ',')),
        ws!(opt(',')),
        '}',
    )
        .map(|(name, _, fields, _, _)| {
            ast::Expression::Struct(ast::StructExpression { name, fields })
        })
        .parse_next(input)
}

fn parse_atom_expression<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    alt((parse_identifier, parse_integer, parse_paren_expr)).parse_next(input)
}

fn parse_paren_expr<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    delimited('(', ws!(parse_expression), ')').parse_next(input)
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

// TODO: Use this instead of implementing it manually.
fn parse_id_and_type<'a>(input: &mut &'a str) -> Result<(String, types::Type)> {
    (ws!(parse_available_name), ':', ws!(parse_type))
        .map(|(name, _, ty)| (name, ty))
        .parse_next(input)
}

fn parse_id_and_expr<'a>(input: &mut &'a str) -> Result<(String, ast::Expression)> {
    (ws!(parse_available_name), ':', ws!(parse_expression))
        .map(|(name, _, expr)| (name, expr))
        .parse_next(input)
}

fn parse_available_name<'a>(input: &mut &'a str) -> Result<String> {
    (
        one_of(('a'..='z', 'A'..='Z', '_')),
        repeat(0.., one_of(('a'..='z', 'A'..='Z', '0'..='9', '_'))),
    )
        .map(|(first, rest): (char, Vec<char>)| {
            format!("{}{}", first, rest.iter().collect::<String>())
        })
        .parse_next(input)
}

fn parse_visibility<'a>(input: &mut &'a str) -> Result<ast::Visibility> {
    alt(("public", "private"))
        .map(|s| match s {
            "public" => ast::Visibility::Public,
            "private" => ast::Visibility::Private,
            _ => panic!("Expect \"public\" or \"private\", but got {:?}", s),
        })
        .parse_next(input)
}

fn parse_mutability<'a>(input: &mut &'a str) -> Result<types::Mutability> {
    alt(("const", "var"))
        .map(|s| match s {
            "const" => types::Mutability::Const,
            "var" => types::Mutability::Var,
            _ => panic!("Expect \"const\" or \"var\", but got {:?}", s),
        })
        .parse_next(input)
}
