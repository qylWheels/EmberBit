use crate::ast;

use winnow::Parser;
use winnow::Result;
use winnow::ascii::digit0;
use winnow::combinator::*;
use winnow::token::*;

macro_rules! ws {
    ($inner:expr) => {
        delimited(multispace0, inner, multispace0)
    };
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
