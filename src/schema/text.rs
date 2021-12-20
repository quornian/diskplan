use std::fmt::Display;

use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, line_ending, space0, space1},
    combinator::{all_consuming, consumed, eof, map, opt, recognize},
    error::{context, VerboseError, VerboseErrorKind},
    multi::{count, many0, many1},
    sequence::{delimited, pair, preceded, tuple},
    IResult, Parser,
};

use crate::schema::{
    criteria::Match,
    expr::{Expression, Identifier, Token},
    Schema, Subschema,
};

type Res<T, U> = IResult<T, U, VerboseError<T>>;

mod properties;
use properties::Properties;

mod error;
pub use error::ParseError;

pub fn parse_schema(text: &str) -> std::result::Result<Schema, ParseError> {
    // Parse and process entire schema and handle any errors that arise
    let (_, ops) =
        all_consuming(preceded(many0(blank_line), many0(operator(0))))(&text).map_err(|e| {
            let e = match e {
                nom::Err::Error(e) | nom::Err::Failure(e) => e,
                nom::Err::Incomplete(_) => unreachable!(),
            };
            let mut error = None;
            for (r, e) in e.errors.iter().rev() {
                error = Some(ParseError::new(
                    match e {
                        VerboseErrorKind::Nom(p) => {
                            format!("Invalid token while looking for: {:?}", p)
                        }
                        _ => format!("Error parsing {:?}", e),
                    },
                    text,
                    r,
                    error.map(Box::new),
                ));
            }
            error.unwrap()
        })?;
    let (match_regex, subschema) = schema(text, text, ops, ItemType::Directory)?;
    if let Some(_) = match_regex {
        return Err(ParseError::new(
            "Top level #match is not allowed".into(),
            text,
            text.find("\n#match")
                .map(|pos| &text[pos + 1..pos + 7])
                .unwrap_or(text),
            None,
        ));
    }
    match subschema {
        Subschema::Referenced { .. } => Err(ParseError::new(
            "Top level #use is not allowed".into(),
            text,
            text.find("\n#use")
                .map(|pos| &text[pos + 1..pos + 7])
                .unwrap_or(text),
            None,
        )),
        Subschema::Original(schema) => Ok(schema),
    }
}

fn schema<'a>(
    whole: &'a str,
    part: &'a str,
    ops: Vec<(&'a str, Operator<'a>)>,
    item_type: ItemType,
) -> std::result::Result<(Option<Expression>, Subschema), ParseError<'a>> {
    let mut props = Properties::new(&item_type);
    for (span, op) in ops {
        match op {
            // Operators that affect the parent (when looking up this item)
            Operator::Match(expr) => props.match_expr(expr),

            // Operators that apply to this item
            Operator::Use { name } => props.use_definition(name),
            Operator::Mode(mode) => props.mode(mode),
            Operator::Owner(owner) => props.owner(owner),
            Operator::Group(group) => props.group(group),
            Operator::Source(source) => match item_type {
                ItemType::File => props.source(source),
                _ => {
                    return Err(ParseError::new(
                        "Only files can have a #source".into(),
                        whole,
                        span,
                        None,
                    ));
                }
            },

            // Operators that apply to child items
            Operator::Let { name, expr } => props.let_var(name, expr),
            Operator::Item {
                binding,
                is_directory,
                link,
                children,
            } => {
                if let ItemType::File = item_type {
                    return Err(ParseError::new(
                        "Files cannot have child items. If this was meant to be a directory, add a /".into(),
                        whole,
                        span,
                        None,
                    ));
                }
                let sub_item_type = match (link, is_directory) {
                    (Some(target), is_directory) => ItemType::Symlink {
                        target,
                        is_directory,
                    },
                    (_, false) => ItemType::File,
                    (_, true) => ItemType::Directory,
                };
                let (pattern, schema) =
                    schema(whole, span, children, sub_item_type).map_err(|e| {
                        ParseError::new(
                            format!("Problem within \"{}\"", binding),
                            whole,
                            span,
                            Some(Box::new(e)),
                        )
                    })?;
                let criteria = match binding {
                    Binding::Static(s) => Match::fixed(s),
                    Binding::Dynamic(binding) => Match::Variable {
                        order: 0,
                        pattern,
                        binding,
                    },
                };
                props.add_entry(criteria, schema)
            }
            Operator::Def {
                name,
                is_directory,
                link,
                children,
            } => {
                if let ItemType::File = item_type {
                    return Err(ParseError::new(
                        format!("Files cannot have child items"),
                        whole,
                        span,
                        None,
                    ));
                }
                let sub_item_type = match (link, is_directory) {
                    (Some(target), is_directory) => ItemType::Symlink {
                        target,
                        is_directory,
                    },
                    (_, false) => ItemType::File,
                    (_, true) => ItemType::Directory,
                };
                let (pattern, schema) =
                    schema(whole, span, children, sub_item_type).map_err(|e| {
                        ParseError::new(
                            format!("Error within definition \"{}\"", name),
                            whole,
                            span,
                            Some(Box::new(e)),
                        )
                    })?;

                if pattern.is_some() {
                    return Err(ParseError::new(
                        format!("#def has own #match"),
                        whole,
                        span,
                        None,
                    ));
                }
                if let Subschema::Original(schema) = schema {
                    props.define(name, schema)
                } else {
                    // TODO: This may be okay
                    return Err(ParseError::new(
                        format!("#def has own #use"),
                        whole,
                        span,
                        None,
                    ));
                }
            }
        }.map_err(|s| ParseError::new(s, whole, span, None))?
    }
    props
        .to_mapped_subschema()
        .map_err(|s| ParseError::new(s, whole, part, None))
}

fn indentation(level: usize) -> impl Fn(&str) -> Res<&str, &str> {
    move |s: &str| recognize(count(tag("    "), level))(s)
}

fn operator(level: usize) -> impl Fn(&str) -> Res<&str, (&str, Operator)> {
    // This is really just to make the op definitions tidier
    fn op<'a, O, P>(op: &'static str, second: P) -> impl FnMut(&'a str) -> Res<&'a str, O>
    where
        P: Parser<&'a str, O, VerboseError<&'a str>>,
    {
        context("op", preceded(tuple((tag(op), space1)), second))
    }

    move |s: &str| {
        let sep = |ch, second| preceded(delimited(space0, char(ch), space0), second);

        let let_op = tuple((op("let", identifier), sep('=', expression)));
        let use_op = op("use", identifier);
        let match_op = op("match", expression);
        let mode_op = op("mode", octal);
        let owner_op = op("owner", username);
        let group_op = op("group", username);
        let source_op = op("source", expression);

        consumed(alt((
            delimited(
                tuple((indentation(level), char('#'))),
                alt((
                    map(let_op, |(name, expr)| Operator::Let { name, expr }),
                    map(use_op, |name| Operator::Use { name }),
                    map(match_op, Operator::Match),
                    map(mode_op, Operator::Mode),
                    map(owner_op, Operator::Owner),
                    map(group_op, Operator::Group),
                    map(source_op, Operator::Source),
                )),
                end_of_lines,
            ),
            map(
                // $binding/ -> link
                //     children...
                tuple((
                    delimited(indentation(level), item_header, end_of_lines),
                    many0(operator(level + 1)),
                )),
                |((binding, is_directory, link), children)| Operator::Item {
                    binding,
                    is_directory,
                    link,
                    children,
                },
            ),
            map(
                tuple((
                    delimited(indentation(level), def_header, end_of_lines),
                    many0(operator(level + 1)),
                )),
                |((name, is_directory, link), children)| Operator::Def {
                    name,
                    is_directory,
                    link,
                    children,
                },
            ),
        )))(s)
    }
}

enum ItemType {
    Directory,
    File,
    Symlink {
        target: Expression,
        is_directory: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum Binding<'a> {
    Static(&'a str),
    Dynamic(Identifier),
}

impl Display for Binding<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Binding::Static(s) => write!(f, "{}", s),
            Binding::Dynamic(id) => write!(f, "${}", id.value()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Operator<'a> {
    Item {
        binding: Binding<'a>,
        is_directory: bool,
        link: Option<Expression>,
        children: Vec<(&'a str, Operator<'a>)>,
    },
    Let {
        name: Identifier,
        expr: Expression,
    },
    Def {
        name: Identifier,
        is_directory: bool,
        link: Option<Expression>,
        children: Vec<(&'a str, Operator<'a>)>,
    },
    Use {
        name: Identifier,
    },
    Match(Expression),
    Mode(u16),
    Owner(&'a str),
    Group(&'a str),
    Source(Expression),
}

fn blank_line(s: &str) -> Res<&str, &str> {
    recognize(alt((tuple((space0, line_ending)), tuple((space1, eof)))))(s)
}

/// Match and consume line endings and any following blank lines, or EOF
fn end_of_lines(s: &str) -> Res<&str, &str> {
    recognize(tuple((alt((line_ending, eof)), many0(blank_line))))(s)
}

fn binding(s: &str) -> Res<&str, Binding<'_>> {
    alt((
        map(preceded(char('$'), identifier), |i| Binding::Dynamic(i)),
        map(filename, Binding::Static),
    ))(s)
}

fn filename(s: &str) -> Res<&str, &str> {
    recognize(many1(alt((alphanumeric1, is_a("_-.@^+%=")))))(s)
}

// $name/ -> link
// name
fn item_header(s: &str) -> Res<&str, (Binding, bool, Option<Expression>)> {
    tuple((
        binding,
        map(opt(char('/')), |o| o.is_some()),
        opt(preceded(tuple((space1, tag("->"), space1)), expression)),
    ))(s)
}

// #def name/
// #def name -> link
fn def_header(s: &str) -> Res<&str, (Identifier, bool, Option<Expression>)> {
    preceded(
        tuple((tag("#def"), space1)),
        tuple((
            identifier,
            map(opt(char('/')), |o| o.is_some()),
            opt(preceded(tuple((space0, tag("->"), space0)), expression)),
        )),
    )(s)
}

fn octal(s: &str) -> Res<&str, u16> {
    map(is_a("01234567"), |mode| {
        u16::from_str_radix(mode, 8).unwrap()
    })(s)
}

fn username(s: &str) -> Res<&str, &str> {
    recognize(many1(alt((alphanumeric1, tag("-"), tag("_")))))(s)
}

fn identifier(s: &str) -> Res<&str, Identifier> {
    map(
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_")))),
        )),
        Identifier::new,
    )(s)
}

/// Expression, such as "static/$varA/${varB}v2/${NAME}"
///
fn expression(s: &str) -> Res<&str, Expression> {
    map(many1(alt((non_variable, variable))), Expression::new)(s)
}

/// A sequence of characters that are not part of any variable
///
fn non_variable(s: &str) -> Res<&str, Token> {
    map(is_not("$\n"), Token::text)(s)
}

/// A variable name, optionally braced, prefixed by a dollar sign, such as `${example}`
///
fn variable(s: &str) -> Res<&str, Token> {
    map(
        preceded(
            char('$'),
            alt((delimited(char('{'), identifier, char('}')), identifier)),
        ),
        Token::Variable,
    )(s)
}

#[cfg(test)]
mod tests;
