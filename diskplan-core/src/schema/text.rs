use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, line_ending, space0, space1},
    combinator::{all_consuming, consumed, eof, map, opt, recognize, value},
    error::{context, VerboseError, VerboseErrorKind},
    multi::{count, many0, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult, Parser,
};

use super::{Binding, SchemaNode};
use crate::schema::{Expression, Identifier, Special, Token};

type Res<T, U> = IResult<T, U, VerboseError<T>>;

mod builder;
use builder::SchemaNodeBuilder;

mod error;
pub use error::ParseError;

#[derive(Debug)]
pub enum NodeType {
    Directory,
    File,
}

pub fn parse_schema(text: &str) -> std::result::Result<SchemaNode, ParseError> {
    // Strip several levels of initial indentation to help with indented literal schemas
    let any_indent = |s| {
        opt(alt((
            many1(operator(0)),
            many1(operator(1)),
            many1(operator(2)),
            many1(operator(3)),
            many1(operator(4)),
        )))(s)
    };
    // Parse and process entire schema and handle any errors that arise
    let (_, ops) = all_consuming(preceded(many0(blank_line), any_indent))(text).map_err(|e| {
        let e = match e {
            nom::Err::Error(e) | nom::Err::Failure(e) => e,
            nom::Err::Incomplete(_) => unreachable!(),
        };
        let mut error = None;
        for (r, e) in e.errors.iter().rev() {
            error = Some(ParseError::new(
                match e {
                    VerboseErrorKind::Nom(p) => {
                        format!("Invalid token while looking for: {p:?}")
                    }
                    _ => format!("Error parsing {e:?}"),
                },
                text,
                r,
                error.map(Box::new),
            ));
        }
        error.unwrap()
    })?;
    let ops = ops.unwrap_or_default();
    let schema_node = schema_node("root", text, text, false, NodeType::Directory, None, ops)?;
    if schema_node.match_pattern.is_some() {
        return Err(ParseError::new(
            "Top level :match is not allowed".into(),
            // TODO: Or is it? (Could alternatively say is_def=true)
            text,
            text.find("\n:match")
                .map(|pos| &text[pos + 1..pos + 7])
                .unwrap_or(text),
            None,
        ));
    }
    Ok(schema_node)
}

fn schema_node<'t>(
    line: &'t str,
    whole: &'t str,
    part: &'t str,
    is_def: bool,
    item_type: NodeType,
    symlink: Option<Expression<'t>>,
    ops: Vec<(&'t str, Operator<'t>)>,
) -> std::result::Result<SchemaNode<'t>, ParseError<'t>> {
    let part_parse_error = |e: anyhow::Error| ParseError::new(e.to_string(), whole, part, None);
    let mut builder = SchemaNodeBuilder::new(
        line,
        is_def,
        match item_type {
            NodeType::Directory => NodeType::Directory,
            NodeType::File => NodeType::File,
        },
        symlink,
    );
    for (span, op) in ops {
        match op {
            // Operators that affect the parent (when looking up this item)
            Operator::Match(expr) => builder.match_pattern(expr),
            Operator::Avoid(expr) => builder.avoid_pattern(expr),

            // Operators that apply to this item
            Operator::Use { name } => builder.use_definition(name),
            Operator::Mode(mode) => builder.mode(mode),
            Operator::Owner(owner) => builder.owner(owner),
            Operator::Group(group) => builder.group(group),
            Operator::Source(source) => builder.source(source),

            // Operators that apply to child items
            Operator::Let { name, expr } => builder.let_var(name, expr),
            Operator::Item {
                line,
                binding,
                is_directory,
                link,
                children,
            } => {
                let sub_item_type = match is_directory {
                    false => NodeType::File,
                    true => NodeType::Directory,
                };
                let item_node =
                    schema_node(line, whole, span, false, sub_item_type, link, children).map_err(
                        |e| {
                            ParseError::new(
                                format!(r#"Problem within "{binding}""#),
                                whole,
                                span,
                                Some(Box::new(e)),
                            )
                        },
                    )?;
                builder.add_entry(binding, item_node)
            }
            Operator::Def {
                line,
                name,
                is_directory,
                link,
                children,
            } => {
                if let NodeType::File = item_type {
                    return Err(ParseError::new(
                        "Files cannot have child items".to_string(),
                        whole,
                        span,
                        None,
                    ));
                }
                let sub_item_type = match is_directory {
                    false => NodeType::File,
                    true => NodeType::Directory,
                };
                let properties =
                    schema_node(line, whole, span, true, sub_item_type, link, children).map_err(
                        |e| {
                            ParseError::new(
                                format!(r#"Error within definition "{name}""#),
                                whole,
                                span,
                                Some(Box::new(e)),
                            )
                        },
                    )?;

                if properties.match_pattern.is_some() {
                    return Err(ParseError::new(
                        ":def has own :match".to_owned(),
                        whole,
                        span,
                        None,
                    ));
                }
                builder.define(name, properties)
            }
        }
        .map_err(|s| ParseError::new(s.to_string(), whole, span, None))?
    }
    // TODO: Handle error spans, child errors?, etc.
    builder.build().map_err(part_parse_error)
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
        let avoid_op = op("avoid", expression);
        let mode_op = op("mode", octal);
        let owner_op = op("owner", expression);
        let group_op = op("group", expression);
        let source_op = op("source", expression);

        consumed(alt((
            delimited(
                tuple((indentation(level), char(':'))),
                alt((
                    map(let_op, |(name, expr)| Operator::Let { name, expr }),
                    map(use_op, |name| Operator::Use { name }),
                    map(match_op, Operator::Match),
                    map(avoid_op, Operator::Avoid),
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
                    delimited(indentation(level), consumed(item_header), end_of_lines),
                    many0(operator(level + 1)),
                )),
                |((line, (binding, is_directory, link)), children)| Operator::Item {
                    line,
                    binding,
                    is_directory,
                    link,
                    children,
                },
            ),
            map(
                tuple((
                    delimited(indentation(level), consumed(def_header), end_of_lines),
                    many0(operator(level + 1)),
                )),
                |((line, (name, is_directory, link)), children)| Operator::Def {
                    line,
                    name,
                    is_directory,
                    link,
                    children,
                },
            ),
        )))(s)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Operator<'t> {
    Item {
        line: &'t str,
        binding: Binding<'t>,
        is_directory: bool,
        link: Option<Expression<'t>>,
        children: Vec<(&'t str, Operator<'t>)>,
    },
    Let {
        name: Identifier<'t>,
        expr: Expression<'t>,
    },
    Def {
        line: &'t str,
        name: Identifier<'t>,
        is_directory: bool,
        link: Option<Expression<'t>>,
        children: Vec<(&'t str, Operator<'t>)>,
    },
    Use {
        name: Identifier<'t>,
    },
    Match(Expression<'t>),
    Avoid(Expression<'t>),
    Mode(u16),
    Owner(Expression<'t>),
    Group(Expression<'t>),
    Source(Expression<'t>),
}

fn blank_line(s: &str) -> Res<&str, &str> {
    alt((
        recognize(tuple((space0, line_ending))),
        recognize(tuple((space1, eof))),
        recognize(tuple((space0, comment, line_ending))),
        recognize(tuple((space0, comment, eof))),
    ))(s)
}

fn comment(s: &str) -> Res<&str, &str> {
    alt((
        recognize(tuple((tag("# "), is_not("\r\n")))),
        terminated(tag("#"), eof),
    ))(s)
}

/// Match and consume line endings and any following blank lines, or EOF
fn end_of_lines(s: &str) -> Res<&str, &str> {
    alt((recognize(tuple((line_ending, many0(blank_line)))), eof))(s)
}

fn binding(s: &str) -> Res<&str, Binding<'_>> {
    alt((
        map(preceded(char('$'), identifier), Binding::Dynamic),
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

// :def name/
// :def name -> link
fn def_header(s: &str) -> Res<&str, (Identifier, bool, Option<Expression>)> {
    preceded(
        tuple((tag(":def"), space1)),
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
    map(many1(alt((non_variable, variable))), |tokens| {
        Expression::from(tokens)
    })(s)
}

/// A sequence of characters that are not part of any variable
///
fn non_variable(s: &str) -> Res<&str, Token> {
    map(is_not("$\n"), Token::Text)(s)
}

/// A variable name, optionally braced, prefixed by a dollar sign, such as `${example}`
///
fn variable(s: &str) -> Res<&str, Token> {
    let braced = |parser| alt((delimited(char('{'), parser, char('}')), parser));
    let vars = |s| {
        alt((
            value(
                Token::Special(Special::PathRelative),
                tag(Special::SAME_PATH_RELATIVE),
            ),
            value(
                Token::Special(Special::PathAbsolute),
                tag(Special::SAME_PATH_ABSOLUTE),
            ),
            value(
                Token::Special(Special::PathNameOnly),
                tag(Special::SAME_PATH_NAME),
            ),
            value(
                Token::Special(Special::ParentRelative),
                tag(Special::PARENT_PATH_RELATIVE),
            ),
            value(
                Token::Special(Special::ParentAbsolute),
                tag(Special::PARENT_PATH_ABSOLUTE),
            ),
            value(
                Token::Special(Special::ParentNameOnly),
                tag(Special::PARENT_PATH_NAME),
            ),
            value(Token::Special(Special::RootPath), tag(Special::ROOT_PATH)),
            map(identifier, Token::Variable),
        ))(s)
    };
    preceded(char('$'), braced(vars))(s)
}

#[cfg(test)]
mod tests;
