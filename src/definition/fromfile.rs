use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    iter::repeat,
    path::Path,
};

use anyhow::{Context, Result};

use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, space0, space1},
    combinator::{all_consuming, eof, map, opt, recognize, value},
    multi::{many0, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    Err, IResult,
};

use super::schema::{Schema, SchemaError};

fn map_io_err<'a>(path: &'a Path) -> impl Fn(io::Error) -> SchemaError + 'a {
    move |e| SchemaError::DirectoryIOError(path.to_owned(), e)
}

pub fn schema_from_path(path: &Path) -> Result<Schema, SchemaError> {
    let file = File::open(path).map_err(map_io_err(path))?;
    for (index, content) in BufReader::new(file).lines().enumerate() {
        let content = &content.map_err(map_io_err(path))?;
        if blank_line(content).is_ok() {
            continue;
        }
        let (_, parsed) = all_consuming(line)(content).map_err(|e| {
            let e = match e {
                Err::Error(e) | Err::Failure(e) => e,
                Err::Incomplete(_) => unreachable!(),
            };
            // Assuming single byte characters
            let col = e.input.as_ptr() as usize - content.as_ptr() as usize;
            let marker: String = repeat('-')
                .take(col)
                .chain(repeat('~').take(content.len() - col))
                .collect();
            SchemaError::SyntaxError(path.to_owned(), index + 1, content.to_owned(), marker)
        })?;
        let Line(indent, op) = parsed;
        println!("{:?}", op);
    }
    Err(SchemaError::SyntaxError(
        path.to_owned(),
        0,
        "".to_owned(),
        "".to_owned(),
    ))
}

struct Line<'a>(Indentation, Operator<'a>);

fn blank_line(s: &str) -> IResult<&str, ()> {
    value((), terminated(space0, eof))(s)
}

fn line(s: &str) -> IResult<&str, Line> {
    map(
        terminated(tuple((indentation, operator)), eof),
        |(indent, item)| Line(indent, item),
    )(s)
}

struct Indentation(usize);

fn indentation(s: &str) -> IResult<&str, Indentation> {
    map(space0, |x: &str| Indentation(x.len()))(s)
}

#[derive(Debug, Clone, PartialEq)]
enum Binding<'a> {
    Static(&'a str),
    Dynamic(Identifier<'a>),
}

fn binding(s: &str) -> IResult<&str, Binding<'_>> {
    alt((
        map(preceded(char('$'), identifier), |i| Binding::Dynamic(i)),
        map(filename, Binding::Static),
    ))(s)
}

fn filename(s: &str) -> IResult<&str, &str> {
    recognize(many1(alt((alphanumeric1, is_a("_-.@^+%=")))))(s)
}

fn filepath(s: &str) -> IResult<&str, &str> {
    recognize(many1(alt((alphanumeric1, is_a("_-.@^+%=")))))(s)
}

fn operator(s: &str) -> IResult<&str, Operator<'_>> {
    alt((
        preceded(
            char('#'),
            alt((
                let_op, def_op, use_op, match_op, mode_op, owner_op, group_op, source_op,
            )),
        ),
        item,
    ))(s)
}

fn item(s: &str) -> IResult<&str, Operator> {
    // [$]name[/][ -> link]
    // $name/ -> link
    // name   -> link
    map(
        tuple((
            binding,
            map(opt(char('/')), |o| o.is_some()),
            opt(preceded(tuple((space1, tag("->"), space1)), expression)),
        )),
        |(binding, is_directory, link)| Operator::Item {
            binding,
            is_directory,
            link,
        },
    )(s)
}

fn let_op(s: &str) -> IResult<&str, Operator<'_>> {
    // let x = $var1/$var2/three
    map(
        tuple((
            preceded(tuple((tag("let"), space1)), identifier),
            preceded(tuple((space0, char('='), space0)), expression),
        )),
        |(name, expr)| Operator::Let { name, expr },
    )(s)
}

fn def_op(s: &str) -> IResult<&str, Operator<'_>> {
    // #def name/
    // #def name -> link
    map(
        preceded(
            tuple((tag("def"), space1)),
            tuple((
                identifier,
                map(opt(char('/')), |o| o.is_some()),
                opt(preceded(tuple((space0, tag("->"), space0)), expression)),
            )),
        ),
        |(name, is_directory, link)| Operator::Def {
            name,
            is_directory,
            link,
        },
    )(s)
}

fn use_op(s: &str) -> IResult<&str, Operator<'_>> {
    // #use name
    map(preceded(tuple((tag("use"), space1)), identifier), |name| {
        Operator::Use { name }
    })(s)
}

fn match_op(s: &str) -> IResult<&str, Operator<'_>> {
    // #match patternexpr
    map(
        preceded(tuple((tag("match"), space1)), expression),
        Operator::Match,
    )(s)
}
fn mode_op(s: &str) -> IResult<&str, Operator<'_>> {
    // #mode 755
    map(
        preceded(tuple((tag("mode"), space1)), is_a("01234567")),
        |mode| Operator::Mode(u16::from_str_radix(mode, 8).unwrap()),
    )(s)
}
fn owner_op(s: &str) -> IResult<&str, Operator<'_>> {
    // #owner user
    map(
        preceded(tuple((tag("owner"), space1)), username),
        Operator::Owner,
    )(s)
}
fn group_op(s: &str) -> IResult<&str, Operator<'_>> {
    // #group user
    map(
        preceded(tuple((tag("group"), space1)), username),
        Operator::Group,
    )(s)
}
fn source_op(s: &str) -> IResult<&str, Operator<'_>> {
    // #source path
    map(
        preceded(tuple((tag("source"), space1)), expression),
        Operator::Source,
    )(s)
}
fn username(s: &str) -> IResult<&str, &str> {
    recognize(many1(alt((alphanumeric1, tag("-")))))(s)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Identifier<'a>(&'a str);

fn identifier(s: &str) -> IResult<&str, Identifier> {
    map(
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_")))),
        )),
        Identifier,
    )(s)
}

#[derive(Debug, Clone, PartialEq)]
struct Expression<'a>(Vec<Token<'a>>);

#[derive(Debug, Clone, PartialEq)]
enum Operator<'a> {
    /// `[$]name[/][ -> link]`
    Item {
        binding: Binding<'a>,
        /// `[/]`
        is_directory: bool,
        /// ` -> link`
        link: Option<Expression<'a>>,
    },

    /// `#let name = expr`
    Let {
        name: Identifier<'a>,
        expr: Expression<'a>,
    },
    /// `#def name`
    Def {
        name: Identifier<'a>,
        is_directory: bool,
        link: Option<Expression<'a>>,
    },
    /// `#use name`
    Use { name: Identifier<'a> },
    /// `#match regex
    Match(Expression<'a>),
    /// `#mode 755`
    Mode(u16),
    /// `#owner user`
    Owner(&'a str),
    /// `#group user`
    Group(&'a str),
    /// `#source path`
    Source(Expression<'a>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr<'a>(Vec<Token<'a>>);

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    Text(&'a str),
    Variable(Identifier<'a>),
    Builtin(Builtin),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Builtin {
    Parent,
    Path,
    Name,
}

impl<'a> Expr<'a> {
    pub fn tokens(&self) -> &Vec<Token<'a>> {
        &self.0
    }
}

/// Expression, such as "static/$varA/${varB}v2/${NAME}"
///
fn expression(input: &str) -> IResult<&str, Expression> {
    map(many0(alt((non_variable, variable))), Expression)(input)
}

/// A sequence of characters that are not part of any variable
///
fn non_variable(input: &str) -> IResult<&str, Token<'_>> {
    is_not("$")(input).map(|(rem, text)| (rem, Token::Text(text)))
}

/// A variable name, optionally braced, prefixed by a dollar sign, such as `${example}`
///
fn variable(input: &str) -> IResult<&str, Token<'_>> {
    map(
        preceded(
            char('$'),
            alt((delimited(char('{'), identifier, char('}')), identifier)),
        ),
        Token::Variable,
    )(input)
}

/// A builtin variable
///
fn builtin(input: &str) -> IResult<&str, Token<'_>> {
    alt((
        value(Token::Builtin(Builtin::Parent), tag("PARENT")),
        value(Token::Builtin(Builtin::Path), tag("PATH")),
        value(Token::Builtin(Builtin::Name), tag("NAME")),
    ))(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_() {
        const CONTENT: &'static str = include_str!("../../directorystructure.dsch");
    }

    #[test]
    fn test_let() {
        assert_eq!(
            operator("#let something = expr"),
            Ok((
                "",
                Operator::Let {
                    name: Identifier("something"),
                    expr: Expression(vec![Token::Text("expr")])
                }
            ))
        );
    }

    #[test]
    fn test_let_underscores() {
        assert_eq!(
            operator("#let with_underscores = expr"),
            Ok((
                "",
                Operator::Let {
                    name: Identifier("with_underscores"),
                    expr: Expression(vec![Token::Text("expr")])
                }
            ))
        );
    }

    #[test]
    fn test_let_underscore_first_and_last() {
        assert_eq!(
            operator("#let _with_underscores_ = expr"),
            Ok((
                "",
                Operator::Let {
                    name: Identifier("_with_underscores_"),
                    expr: Expression(vec![Token::Text("expr")])
                }
            ))
        );
    }
}
