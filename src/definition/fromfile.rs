use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, Read},
    iter::{once, repeat},
    path::Path,
};

use anyhow::Result;

use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, line_ending, space0, space1},
    combinator::{all_consuming, eof, map, opt, recognize, value},
    multi::{many0, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};

use super::{
    meta::Meta,
    schema::{DirectorySchema, Schema, SchemaError},
};

pub fn schema_from_path(path: &Path) -> Result<Schema, SchemaError> {
    let content = (|| -> Result<String, io::Error> {
        let mut file = File::open(path)?;
        let mut content = String::with_capacity(file.metadata()?.len() as usize);
        file.read_to_string(&mut content)?;
        Ok(content)
    })()
    .map_err(|e| SchemaError::IOError(path.to_owned(), e))?;

    // Parse and process entire schema and handle any errors that arise
    let (_, schema) = all_consuming(schema(0))(&content).map_err(|e| {
        let e = match e {
            nom::Err::Error(e) | nom::Err::Failure(e) => e,
            nom::Err::Incomplete(_) => unreachable!(),
        };
        // Create a nice syntax error message
        let err_pos = e.input.as_ptr() as usize - content.as_ptr() as usize;
        let line_number = content[..err_pos].chars().filter(|&c| c == '\n').count() + 1;
        let line_start = content[..err_pos].rfind("\n").map(|n| n + 1).unwrap_or(0);
        let column = err_pos - line_start;
        let line = content[line_start..].split("\n").next().unwrap().to_owned();
        let marker: String = repeat(' ').take(column).chain("^~~~".chars()).collect();
        SchemaError::SyntaxError {
            path: path.to_owned(),
            details: format!("\n     |\n{:4} | {}\n     : {}", line_number, line, marker),
        }
    })?;
    Ok(schema)
}

fn schema(current_indent: usize) -> impl Fn(&str) -> IResult<&str, Schema> {
    move |s: &str| -> IResult<&str, Schema> {
        let mut vars = HashMap::new();
        let mut defs = HashMap::new();
        let mut meta = Meta::default();
        let mut entries = Vec::new();
        let mut remaining = s;
        loop {
            // Read and parse a line
            let (left, line) = alt((value(None, blank_line), map(line, Some)))(remaining)?;
            remaining = left;

            if let Some(Line(line_indent, op)) = line {
                // if line_indent != current_indent {
                //     return Err(nom::Err::Failure(nom::error::Error::new(
                //         s,
                //         nom::error::ErrorKind::Fail,
                //     )));
                // }
                println!("{:?}", op);
            }
            if s.len() == 0 {
                break;
            }
        }
        Ok((
            s,
            Schema::Directory(DirectorySchema::new(vars, defs, meta, entries)),
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Line<'a>(usize, Operator<'a>);

#[derive(Debug, Clone, PartialEq)]
enum Binding<'a> {
    Static(&'a str),
    Dynamic(Identifier<'a>),
}

#[derive(Debug, Clone, PartialEq)]
struct Identifier<'a>(&'a str);

#[derive(Debug, Clone, PartialEq)]
struct Expression<'a>(Vec<Token<'a>>);

#[derive(Debug, Clone, PartialEq)]
enum Token<'a> {
    Text(&'a str),
    Variable(Identifier<'a>),
}

#[derive(Debug, Clone, PartialEq)]
enum Operator<'a> {
    Item {
        binding: Binding<'a>,
        is_directory: bool,
        link: Option<Expression<'a>>,
    },
    Let {
        name: Identifier<'a>,
        expr: Expression<'a>,
    },
    Def {
        name: Identifier<'a>,
        is_directory: bool,
        link: Option<Expression<'a>>,
    },
    Use {
        name: Identifier<'a>,
    },
    Match(Expression<'a>),
    Mode(u16),
    Owner(&'a str),
    Group(&'a str),
    Source(Expression<'a>),
}

fn blank_line(s: &str) -> IResult<&str, ()> {
    value((), terminated(space0, alt((line_ending, eof))))(s)
}

fn line(s: &str) -> IResult<&str, Line> {
    map(
        terminated(tuple((indentation, operator)), alt((line_ending, eof))),
        |(indent, item)| Line(indent, item),
    )(s)
}

fn indentation(s: &str) -> IResult<&str, usize> {
    map(space0, |x: &str| x.len())(s)
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

// $name/ -> link
// name   -> link
fn item(s: &str) -> IResult<&str, Operator> {
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

// let x = $var1/$var2/three
fn let_op(s: &str) -> IResult<&str, Operator<'_>> {
    map(
        tuple((
            preceded(tuple((tag("let"), space1)), identifier),
            preceded(tuple((space0, char('='), space0)), expression),
        )),
        |(name, expr)| Operator::Let { name, expr },
    )(s)
}

// #def name/
// #def name -> link
fn def_op(s: &str) -> IResult<&str, Operator<'_>> {
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

// #use name
fn use_op(s: &str) -> IResult<&str, Operator<'_>> {
    map(preceded(tuple((tag("use"), space1)), identifier), |name| {
        Operator::Use { name }
    })(s)
}

// #match patternexpr
fn match_op(s: &str) -> IResult<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("match"), space1)), expression),
        Operator::Match,
    )(s)
}

// #mode 755
fn mode_op(s: &str) -> IResult<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("mode"), space1)), is_a("01234567")),
        |mode| Operator::Mode(u16::from_str_radix(mode, 8).unwrap()),
    )(s)
}

// #owner user
fn owner_op(s: &str) -> IResult<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("owner"), space1)), username),
        Operator::Owner,
    )(s)
}

// #group user
fn group_op(s: &str) -> IResult<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("group"), space1)), username),
        Operator::Group,
    )(s)
}

// #source path
fn source_op(s: &str) -> IResult<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("source"), space1)), expression),
        Operator::Source,
    )(s)
}

fn username(s: &str) -> IResult<&str, &str> {
    recognize(many1(alt((alphanumeric1, tag("-")))))(s)
}

fn identifier(s: &str) -> IResult<&str, Identifier> {
    map(
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_")))),
        )),
        Identifier,
    )(s)
}

/// Expression, such as "static/$varA/${varB}v2/${NAME}"
///
fn expression(s: &str) -> IResult<&str, Expression> {
    map(many1(alt((non_variable, variable))), Expression)(s)
}

/// A sequence of characters that are not part of any variable
///
fn non_variable(s: &str) -> IResult<&str, Token<'_>> {
    map(is_not("$\n"), Token::Text)(s)
}

/// A variable name, optionally braced, prefixed by a dollar sign, such as `${example}`
///
fn variable(s: &str) -> IResult<&str, Token<'_>> {
    map(
        preceded(
            char('$'),
            alt((delimited(char('{'), identifier, char('}')), identifier)),
        ),
        Token::Variable,
    )(s)
}

#[cfg(test)]
mod test {
    use super::*;

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

    #[test]
    fn test_def() {
        assert_eq!(
            operator("#def something"),
            Ok((
                "",
                Operator::Def {
                    name: Identifier("something"),
                    is_directory: false,
                    link: None,
                }
            ))
        );
        assert_eq!(
            operator("#def something/"),
            Ok((
                "",
                Operator::Def {
                    name: Identifier("something"),
                    is_directory: true,
                    link: None,
                }
            ))
        );
        assert_eq!(
            operator("#def something_"),
            Ok((
                "",
                Operator::Def {
                    name: Identifier("something_"),
                    is_directory: false,
                    link: None,
                }
            ))
        );
        assert_eq!(
            operator("#def something/-"),
            Ok((
                "-",
                Operator::Def {
                    name: Identifier("something"),
                    is_directory: true,
                    link: None,
                }
            ))
        );
        assert_eq!(
            operator("#def something -> /somewhere/else"),
            Ok((
                "",
                Operator::Def {
                    name: Identifier("something"),
                    is_directory: false,
                    link: Some(Expression(vec![Token::Text("/somewhere/else")])),
                }
            ))
        );
        assert_eq!(
            operator("#def something -> /some$where/else"),
            Ok((
                "",
                Operator::Def {
                    name: Identifier("something"),
                    is_directory: false,
                    link: Some(Expression(vec![
                        Token::Text("/some"),
                        Token::Variable(Identifier("where")),
                        Token::Text("/else")
                    ])),
                }
            ))
        );
    }

    #[test]
    fn test_unterminated_line() {
        let s = "";
        assert_eq!(blank_line(s), Ok(("", ())));
        assert!(line(s).is_err());
    }
    #[test]
    fn test_blank_line() {
        let s = "\n";
        assert_eq!(blank_line(s), Ok(("", ())));
        assert!(line(s).is_err());
    }

    #[test]
    fn test_blankish_line() {
        let s = "    \n";
        assert_eq!(blank_line(s), Ok(("", ())));
        assert!(line(s).is_err());
    }

    #[test]
    fn test_single_line_mode_op() {
        assert_eq!(line("#mode 777"), Ok(("", Line(0, Operator::Mode(0o777)))));
    }

    #[test]
    fn test_multiline_meta_ops() {
        let s = "#mode 777\n\
                 #owner usr-1\n\
                 #group grpX";
        let t = "#owner usr-1\n\
                 #group grpX";
        let u = "#group grpX";
        assert_eq!(line(s), Ok((t, Line(0, Operator::Mode(0o777)))));
        assert_eq!(line(t), Ok((u, Line(0, Operator::Owner("usr-1")))));
        assert_eq!(line(u), Ok(("", Line(0, Operator::Group("grpX")))));
    }

    #[test]
    fn test_match_pattern() {
        let s = "#match [A-Z][A-Za-z]+";
        assert_eq!(
            line(s),
            Ok((
                "",
                Line(
                    0,
                    Operator::Match(Expression(vec![Token::Text("[A-Z][A-Za-z]+")]))
                )
            ))
        )
    }

    #[test]
    fn test_source_pattern() {
        let s = "#source /a/file/path";
        assert_eq!(
            line(s),
            Ok((
                "",
                Line(
                    0,
                    Operator::Source(Expression(vec![Token::Text("/a/file/path")]))
                )
            ))
        )
    }

    #[test]
    fn test_multiline_with_break() {
        let s = "#def defined/\n";
        assert_eq!(
            line(s),
            Ok((
                "",
                Line(
                    0,
                    Operator::Def {
                        name: Identifier("defined"),
                        is_directory: true,
                        link: None,
                    }
                )
            ))
        );
    }
}
