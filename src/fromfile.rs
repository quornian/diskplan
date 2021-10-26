use std::{
    collections::HashMap,
    fmt::Write,
    fs::File,
    io::{self, Read},
    iter::repeat,
    path::{Path, PathBuf},
};

use anyhow::Result;

use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, line_ending, space0, space1},
    combinator::{all_consuming, eof, map, opt, recognize, value},
    error::{context, VerboseError, VerboseErrorKind},
    multi::{many0, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use regex::Regex;

use crate::{
    apply::ApplicationError,
    schema::{
        criteria::{Match, MatchCriteria},
        expr,
        meta::{Meta, MetaBuilder, RawItemMeta},
        DirectorySchema, FileSchema, LinkSchema, Schema, SchemaError,
    },
};

type Res<T, U> = IResult<T, U, VerboseError<T>>;

pub fn schema_from_path(path: &Path) -> Result<Schema, SchemaError> {
    let content = (|| -> Result<String, io::Error> {
        let mut file = File::open(path)?;
        let mut content = String::with_capacity(file.metadata()?.len() as usize);
        file.read_to_string(&mut content)?;
        Ok(content)
    })()
    .map_err(|e| SchemaError::IOError(path.to_owned(), e))?;

    // Parse and process entire schema and handle any errors that arise
    let (_, (match_regex, schema)) =
        all_consuming(block(0, BlockType::Directory, &content))(&content).map_err(|e| {
            let e = match e {
                nom::Err::Error(e) | nom::Err::Failure(e) => e,
                nom::Err::Incomplete(_) => unreachable!(),
            };
            // Create a nice syntax error message
            let mut details = String::new();
            for (r, e) in e.errors.iter().rev() {
                let err_pos = r.as_ptr() as usize - content.as_ptr() as usize;
                let line_number = content[..err_pos].chars().filter(|&c| c == '\n').count() + 1;
                let line_start = content[..err_pos].rfind("\n").map(|n| n + 1).unwrap_or(0);
                let column = err_pos - line_start;
                let line = content[line_start..].split("\n").next().unwrap().to_owned();
                let marker: String = repeat(' ').take(column).chain("^~~~".chars()).collect();
                write!(
                    details,
                    "\n     |\n{:4} | {}\n     : {}",
                    line_number, line, marker
                )
                .unwrap();
                write!(details, "{:?}", e).unwrap();
            }
            SchemaError::SyntaxError {
                path: path.to_owned(),
                details: details,
            }
        })?;
    match match_regex {
        Some(match_regex) => Err(SchemaError::SyntaxError {
            path: path.to_owned(),
            details: format!("Top level #match is not allowed"),
        }),
        None => Ok(schema),
    }
}

enum BlockType {
    Directory,
    File,
    Symlink(expr::Expression),
}

fn block<'a>(
    block_indent: usize,
    block_type: BlockType,
    block_header: &'a str,
) -> impl Fn(&'a str) -> Res<&'a str, (Option<expr::Expression>, Schema)> {
    #[derive(Default)]
    struct Properties {
        match_regex: Option<expr::Expression>,
        vars: HashMap<expr::Identifier, expr::Expression>,
        defs: HashMap<expr::Identifier, Schema>,
        meta: MetaBuilder,
        // Directory only
        entries: Vec<(MatchCriteria, Schema)>,
        // File only
        source: Option<expr::Expression>,
        // Use only
        def_use: Option<expr::Identifier>,
    }
    move |s: &str| -> Res<&str, (Option<expr::Expression>, Schema)> {
        let mut props = Properties::default();
        let mut remaining = s;
        loop {
            // Read and parse a line
            let pre_read = remaining;

            let (left, line) = alt((value(None, blank_line), map(line, Some)))(remaining)?;
            remaining = left;

            if let Some(Line(line_indent, op)) = line {
                if line_indent < block_indent {
                    remaining = pre_read; // Roll back
                    break;
                }
                println!("{:?}", op);
                match op {
                    Operator::Item {
                        binding,
                        is_directory,
                        link,
                    } => {
                        // TODO: Fail if current block_type is BlockType::File
                        let sub_block_type = match (link, is_directory) {
                            (Some(target), _) => BlockType::Symlink(target.to_expr()),
                            (_, false) => BlockType::File,
                            (_, true) => BlockType::Directory,
                        };
                        let (left, (sub_regex, sub_schema)) = context(
                            "block",
                            block(block_indent + 4, sub_block_type, pre_read),
                        )(remaining)?;
                        remaining = left;

                        let criteria = match binding {
                            Binding::Static(s) => MatchCriteria::new(0, Match::Fixed(s.to_owned())),
                            Binding::Dynamic(ident) => MatchCriteria::new(
                                0,
                                Match::Variable {
                                    pattern: sub_regex,
                                    binding: ident.to_identifier(),
                                },
                            ),
                        };
                        props.entries.push((criteria, sub_schema));
                    }
                    Operator::Let { name, expr } => {
                        props.vars.insert(name.to_identifier(), expr.to_expr());
                    }
                    Operator::Def {
                        name,
                        is_directory,
                        link,
                    } => {
                        let sub_block_type = match (link, is_directory) {
                            (Some(target), _) => BlockType::Symlink(target.to_expr()),
                            (_, false) => BlockType::File,
                            (_, true) => BlockType::Directory,
                        };
                        let (left, (sub_regex, sub_schema)) =
                            block(block_indent + 4, sub_block_type, pre_read)(remaining)?;
                        remaining = left;
                        if sub_regex.is_some() {
                            // TODO: Better error types
                            eprintln!("#def has own #match");
                            return Err(nom::Err::Error(VerboseError {
                                errors: vec![(block_header, VerboseErrorKind::Context("#def"))],
                            }));
                        }
                        props.defs.insert(name.to_identifier(), sub_schema);
                    }
                    Operator::Use { name } => props.def_use = Some(name.to_identifier()),
                    Operator::Match(expr) => props.match_regex = Some(expr.to_expr()),
                    Operator::Mode(mode) => props.meta.mode(mode),
                    Operator::Owner(owner) => props.meta.owner(owner),
                    Operator::Group(group) => props.meta.group(group),
                    Operator::Source(source) => {
                        // TODO: Fail if current block_type is not BlockType::File
                        props.source = Some(source.to_expr())
                    }
                }
            }
            if remaining.len() == 0 {
                break;
            }
        }
        Ok((
            remaining,
            (
                props.match_regex,
                match &block_type {
                    BlockType::Directory => Schema::Directory(DirectorySchema::new(
                        props.vars,
                        props.defs,
                        props.meta.build(),
                        props.entries,
                    )),
                    BlockType::File => {
                        if let Some(source) = props.source {
                            Schema::File(FileSchema::new(props.meta.build(), source))
                        } else {
                            return Err(nom::Err::Error(VerboseError {
                                errors: vec![(
                                    block_header,
                                    VerboseErrorKind::Context("File has no #source"),
                                )],
                            }));
                        }
                    }
                    BlockType::Symlink(target) => Schema::Symlink(LinkSchema::new(
                        target.clone(),
                        // TODO: File-like symlinks
                        Schema::Directory(DirectorySchema::new(
                            props.vars,
                            props.defs,
                            props.meta.build(),
                            props.entries,
                        )),
                    )),
                },
            ),
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Identifier<'a>(&'a str);

#[derive(Debug, Clone, PartialEq)]
struct Expression<'a>(Vec<Token<'a>>);

#[derive(Debug, Clone, PartialEq)]
enum Token<'a> {
    Text(&'a str),
    Variable(Identifier<'a>),
}

impl Expression<'_> {
    pub fn to_expr(&self) -> expr::Expression {
        expr::Expression::new(self.0.iter().map(|t| t.to_token()).collect())
    }
}

impl Token<'_> {
    pub fn to_token(&self) -> expr::Token {
        match self {
            Self::Text(t) => expr::Token::Text(t.to_string()),
            Self::Variable(v) => expr::Token::Variable(v.to_identifier()),
        }
    }
}

impl Identifier<'_> {
    pub fn to_identifier(&self) -> expr::Identifier {
        expr::Identifier::new(self.0)
    }
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

fn blank_line(s: &str) -> Res<&str, ()> {
    value((), terminated(space0, alt((line_ending, eof))))(s)
}

fn line(s: &str) -> Res<&str, Line> {
    map(
        terminated(tuple((indentation, operator)), alt((line_ending, eof))),
        |(indent, item)| Line(indent, item),
    )(s)
}

fn indentation(s: &str) -> Res<&str, usize> {
    map(space0, |x: &str| x.len())(s)
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

fn operator(s: &str) -> Res<&str, Operator<'_>> {
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
fn item(s: &str) -> Res<&str, Operator> {
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
fn let_op(s: &str) -> Res<&str, Operator<'_>> {
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
fn def_op(s: &str) -> Res<&str, Operator<'_>> {
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
fn use_op(s: &str) -> Res<&str, Operator<'_>> {
    map(preceded(tuple((tag("use"), space1)), identifier), |name| {
        Operator::Use { name }
    })(s)
}

// #match patternexpr
fn match_op(s: &str) -> Res<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("match"), space1)), expression),
        Operator::Match,
    )(s)
}

// #mode 755
fn mode_op(s: &str) -> Res<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("mode"), space1)), is_a("01234567")),
        |mode| Operator::Mode(u16::from_str_radix(mode, 8).unwrap()),
    )(s)
}

// #owner user
fn owner_op(s: &str) -> Res<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("owner"), space1)), username),
        Operator::Owner,
    )(s)
}

// #group user
fn group_op(s: &str) -> Res<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("group"), space1)), username),
        Operator::Group,
    )(s)
}

// #source path
fn source_op(s: &str) -> Res<&str, Operator<'_>> {
    map(
        preceded(tuple((tag("source"), space1)), expression),
        Operator::Source,
    )(s)
}

fn username(s: &str) -> Res<&str, &str> {
    recognize(many1(alt((alphanumeric1, tag("-")))))(s)
}

fn identifier(s: &str) -> Res<&str, Identifier> {
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
fn expression(s: &str) -> Res<&str, Expression> {
    map(many1(alt((non_variable, variable))), Expression)(s)
}

/// A sequence of characters that are not part of any variable
///
fn non_variable(s: &str) -> Res<&str, Token<'_>> {
    map(is_not("$\n"), Token::Text)(s)
}

/// A variable name, optionally braced, prefixed by a dollar sign, such as `${example}`
///
fn variable(s: &str) -> Res<&str, Token<'_>> {
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
