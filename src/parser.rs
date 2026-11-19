use crate::ast;

use winnow::Parser;
use winnow::Result;
use winnow::ascii::alphanumeric0;
use winnow::ascii::digit0;
use winnow::combinator::*;
use winnow::token::*;

macro_rules! ws {
    ($inner:expr) => {
        delimited(multispace0, inner, multispace0)
    };
}

pub fn parse_expression<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    let prefix_parser = dispatch! {any;
        _ => fail
    };
    let infix_parser = dispatch! {any;
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
            separated(0.., parse_expression, ',')
                .map(|args| ast::Expression::Call(ast::CallExpression {
                    callee: Box::new(base.clone()),
                    args
                }))
                .parse_next(input)
        }),
        _ => fail
    };

    expression(parse_atom_expression)
        .prefix(prefix_parser)
        .infix(infix_parser)
        .postfix(postfix_parser)
        .parse_next(input)
}

fn parse_atom_expression<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    alt((parse_identifier, parse_integer)).parse_next(input)
}

fn parse_identifier<'a>(input: &mut &'a str) -> Result<ast::Expression> {
    (one_of(('a'..='z', 'A'..='Z', '_')), alphanumeric0)
        .map(|(first, rest)| format!("{}{}", first, rest))
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
