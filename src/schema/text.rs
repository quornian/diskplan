use std::{
    collections::HashMap,
    fmt::{Display, Write},
    iter::repeat,
};

use anyhow::{anyhow, Result};

use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, line_ending, space0, space1},
    combinator::{all_consuming, consumed, eof, map, opt, recognize},
    error::{context, VerboseError},
    multi::{count, many0, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult, Parser,
};

use crate::schema::{
    criteria::Match,
    expr::{Expression, Identifier, Token},
    meta::MetaBuilder,
    DirectorySchema, FileSchema, LinkSchema, Schema, SchemaEntry, Subschema,
};

type Res<T, U> = IResult<T, U, VerboseError<T>>;

pub fn parse_schema(text: &str) -> Result<Schema> {
    // Parse and process entire schema and handle any errors that arise
    let (_, ops) = all_consuming(many0(operator(0)))(&text).map_err(|e| {
        let e = match e {
            nom::Err::Error(e) | nom::Err::Failure(e) => e,
            nom::Err::Incomplete(_) => unreachable!(),
        };
        // Create a nice syntax error message
        let mut details = String::new();
        for (r, e) in e.errors.iter().rev() {
            let err_pos = r.as_ptr() as usize - text.as_ptr() as usize;
            let line_number = text[..err_pos].chars().filter(|&c| c == '\n').count() + 1;
            let line_start = text[..err_pos].rfind("\n").map(|n| n + 1).unwrap_or(0);
            let column = err_pos - line_start;
            let line = text[line_start..].split("\n").next().unwrap().to_owned();
            let marker: String = repeat(' ').take(column).chain("^~~~".chars()).collect();
            write!(
                details,
                "\n     |\n{:4} | {}\n     : {}",
                line_number, line, marker
            )
            .unwrap();
            write!(details, "{:?}", e).unwrap();
        }
        anyhow!("Syntax error: {}", details)
    })?;
    let (match_regex, subschema) =
        schema(text, text, ops, ItemType::Directory).map_err(|(pos, err)| {
            anyhow!(
                "Error in block starting on line {}...\n{}",
                find_line_number(pos, text),
                err
            )
        })?;
    if let Some(_) = match_regex {
        return Err(anyhow!("Top level #match is not allowed"));
    }
    match subschema {
        Subschema::Referenced { .. } => Err(anyhow!("Top level #use is not allowed")),
        Subschema::Original(schema) => Ok(schema),
    }
}

fn find_line_number(pos: &str, whole: &str) -> usize {
    let pos = pos.as_ptr() as usize - whole.as_ptr() as usize;
    whole[..pos].chars().filter(|&c| c == '\n').count() + 1
}

fn schema<'a>(
    whole: &'a str,
    part: &'a str,
    ops: Vec<(&'a str, Operator<'a>)>,
    item_type: ItemType,
) -> std::result::Result<(Option<Expression>, Subschema), (&'a str, String)> {
    let mut props = Properties::default();
    for (pos, op) in ops {
        match op {
            // Operators that affect the parent (when looking up this item)
            Operator::Match(expr) => props.match_regex = Some(expr),

            // Operators that apply to this item
            Operator::Use { name } => props.use_def = Some(name),
            Operator::Mode(mode) => {
                props.meta.mode(mode);
            }
            Operator::Owner(owner) => {
                props.meta.owner(owner);
            }
            Operator::Group(group) => {
                props.meta.group(group);
            }
            Operator::Source(source) => {
                match item_type {
                    ItemType::File => (),
                    _ => return Err((pos, format!("Only files can have a #source"))),
                }
                props.source = Some(source)
            }

            // Operators that apply to child items
            Operator::Let { name, expr } => drop(props.vars.insert(name, expr)),
            Operator::Item {
                binding,
                is_directory,
                link,
                children,
            } => {
                if let ItemType::File = item_type {
                    return Err((pos, format!("Files cannot have child items")));
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
                    schema(whole, pos, children, sub_item_type).map_err(|(err_pos, err)| {
                        (
                            pos,
                            format!(
                                "Error within \"{}\" on line {}\n{}",
                                binding,
                                find_line_number(err_pos, whole),
                                err
                            ),
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
                props.entries.push(SchemaEntry { criteria, schema });
            }
            Operator::Def {
                name,
                is_directory,
                link,
                children,
            } => {
                if let ItemType::File = item_type {
                    return Err((pos, format!("Files cannot have child items")));
                }
                let sub_item_type = match (link, is_directory) {
                    (Some(target), is_directory) => ItemType::Symlink {
                        target,
                        is_directory,
                    },
                    (_, false) => ItemType::File,
                    (_, true) => ItemType::Directory,
                };
                let (pattern, schema) = schema(whole, pos, children, sub_item_type)?;
                if pattern.is_some() {
                    return Err((pos, format!("#def has own #match")));
                }
                if let Subschema::Original(schema) = schema {
                    props.defs.insert(name, schema);
                } else {
                    // TODO: This may be okay
                    return Err((pos, format!("#def has own #use")));
                }
            }
        }
    }

    let schema = match &item_type {
        ItemType::Directory => Schema::Directory(DirectorySchema::new(
            props.vars,
            props.defs,
            props.meta.build(),
            props.entries,
        )),
        ItemType::File => {
            if let Some(source) = props.source {
                Schema::File(FileSchema::new(props.meta.build(), source))
            } else {
                return Err((
                    part,
                    format!("File has no #source. Should this have been a directory?"),
                ));
            }
        }
        ItemType::Symlink {
            target,
            is_directory,
        } => Schema::Symlink({
            let schema = if props.vars.is_empty()
                && props.defs.is_empty()
                && props.meta.is_empty()
                && props.entries.is_empty()
            {
                None
            } else if *is_directory {
                Some(Box::new(Schema::Directory(DirectorySchema::new(
                    props.vars,
                    props.defs,
                    props.meta.build(),
                    props.entries,
                ))))
            } else {
                Some(Box::new(if let Some(source) = props.source {
                    Schema::File(FileSchema::new(props.meta.build(), source))
                } else {
                    return Err((
                        part,
                        format!("File has no #source. Should this have been a directory?"),
                    ));
                }))
            };
            LinkSchema::new(target.clone(), schema)
        }),
    };
    Ok((
        props.match_regex,
        match props.use_def {
            Some(use_def) => Subschema::Referenced {
                definition: use_def,
                overrides: schema,
            },
            None => Subschema::Original(schema),
        },
    ))
}

fn indentation(level: usize) -> impl Fn(&str) -> Res<&str, &str> {
    move |s: &str| recognize(count(tag("    "), level))(s)
}

fn operator(level: usize) -> impl Fn(&str) -> Res<&str, (&str, Operator)> {
    // This is really just to make the op definitions tidier
    fn op<'a, O, P>(
        level: usize,
        op: &'static str,
        second: P,
    ) -> impl FnMut(&'a str) -> Res<&'a str, O>
    where
        P: Parser<&'a str, O, VerboseError<&'a str>>,
    {
        context(
            "op",
            preceded(tuple((indentation(level), tag(op), space1)), second),
        )
    }

    move |s: &str| {
        let sep = |ch, second| preceded(delimited(space0, char(ch), space0), second);

        let let_op = tuple((op(level, "#let", identifier), sep('=', expression)));
        let use_op = op(level, "#use", identifier);
        let match_op = op(level, "#match", expression);
        let mode_op = op(level, "#mode", octal);
        let owner_op = op(level, "#owner", username);
        let group_op = op(level, "#group", username);
        let source_op = op(level, "#source", expression);

        consumed(alt((
            terminated(
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
                    terminated(preceded(indentation(level), item_header), end_of_lines),
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
                    terminated(preceded(indentation(level), def_header), end_of_lines),
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

#[derive(Default)]
struct Properties {
    match_regex: Option<Expression>,
    vars: HashMap<Identifier, Expression>,
    defs: HashMap<Identifier, Schema>,
    meta: MetaBuilder,
    // Directory only
    entries: Vec<SchemaEntry>,
    // File only
    source: Option<Expression>,

    // Set if this schema inherits a definition from elsewhere
    use_def: Option<Identifier>,
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

/// Match and consume line endings and any following blank lines, or EOF
fn end_of_lines(s: &str) -> Res<&str, &str> {
    // TODO: This allows trailing whitespace, disallow that?
    //value((), many0(preceded(space0, alt((line_ending, eof)))))(s)
    alt((
        recognize(many0(preceded(space0, line_ending))),
        preceded(space0, eof),
    ))(s)
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
    recognize(many1(alt((alphanumeric1, tag("-")))))(s)
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
mod test {
    use std::vec;

    use crate::schema::meta::Meta;

    use super::*;

    #[test]
    fn test_invalid_space() {
        assert!(parse_schema("okay_entry/").is_ok());
        assert!(parse_schema("invalid entry/").is_err());
    }
    #[test]
    fn test_invalid_child() {
        assert!(parse_schema(concat!(
            "okay_entry\n", //
            "    #source /tmp\n",
        ))
        .is_ok());
        assert!(parse_schema(concat!(
            "okay_entry/\n", //
            "    child\n",
            "        #source /tmp\n",
        ))
        .is_ok());
        assert!(parse_schema(concat!(
            "okay_entry\n", //
            "    child\n",
            "        #source /tmp\n",
        ))
        .is_err());
    }

    #[test]
    fn test_let() {
        let s = "#let something = expr";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Let {
                        name: Identifier::new("something"),
                        expr: Expression::new(vec![Token::text("expr")])
                    }
                )
            ))
        );
        let s = "#let with_underscores = expr";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Let {
                        name: Identifier::new("with_underscores"),
                        expr: Expression::new(vec![Token::text("expr")])
                    }
                )
            ))
        );
        let s = "#let _with_underscores_ = expr";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Let {
                        name: Identifier::new("_with_underscores_"),
                        expr: Expression::new(vec![Token::text("expr")])
                    }
                )
            ))
        );
    }

    #[test]
    fn test_def_header() {
        assert_eq!(
            def_header("#def something"),
            Ok(("", (Identifier::new("something"), false, None)))
        );
        assert_eq!(
            def_header("#def something/"),
            Ok(("", (Identifier::new("something"), true, None,)))
        );
    }

    #[test]
    fn test_def_op_no_children() {
        let s0 = "#def something_";
        let level = 0;
        let (s1, o1) = terminated(
            preceded(indentation(level), def_header),
            alt((line_ending, eof)),
        )(s0)
        .unwrap();
        assert_eq!(o1, (Identifier::new("something_"), false, None));
        let (s2, o2) = many0(operator(level + 1))(s1).unwrap();
        assert_eq!(o2, vec![]);
        assert_eq!(s2, "");

        let s = "#def something_";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Def {
                        name: Identifier::new("something_"),
                        is_directory: false,
                        link: None,
                        children: vec![],
                    }
                )
            ))
        );
        let s = "#def something/-";
        assert_eq!(
            operator(0)(s),
            Ok((
                "-",
                (
                    s.strip_suffix("-").unwrap(),
                    Operator::Def {
                        name: Identifier::new("something"),
                        is_directory: true,
                        link: None,
                        children: vec![],
                    }
                )
            ))
        );
        let s = "#def something -> /somewhere/else";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Def {
                        name: Identifier::new("something"),
                        is_directory: false,
                        link: Some(Expression::new(vec![Token::text("/somewhere/else")])),
                        children: vec![],
                    }
                )
            ))
        );
    }

    #[test]
    fn test_def_op_with_children() {
        let s = "#def something -> /some$where/else";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Def {
                        name: Identifier::new("something"),
                        is_directory: false,
                        link: Some(Expression::new(vec![
                            Token::text("/some"),
                            Token::Variable(Identifier::new("where")),
                            Token::text("/else")
                        ])),
                        children: vec![],
                    }
                )
            ))
        );
    }

    #[test]
    fn test_unterminated_line() {
        let s = "";
        assert_eq!(end_of_lines(s), Ok(("", s)));
    }
    #[test]
    fn test_blank_line() {
        let s = "\n";
        assert_eq!(end_of_lines(s), Ok(("", s)));
    }

    #[test]
    fn test_blankish_line() {
        let s = "    \n";
        assert_eq!(end_of_lines(s), Ok(("", s)));
    }

    #[test]
    fn test_blankish_lines() {
        let s = "    \n\n    \n\n";
        assert_eq!(end_of_lines(s), Ok(("", s)));
    }

    #[test]
    fn test_single_line_mode_op() {
        let s = "#mode 777";
        assert_eq!(operator(0)(s), Ok(("", (s, Operator::Mode(0o777)))));
    }

    #[test]
    fn test_multiline_meta_ops() {
        let s = "#mode 777\n\
                 #owner usr-1\n\
                 #group grpX";
        let t = "#owner usr-1\n\
                 #group grpX";
        let u = "#group grpX";
        assert_eq!(operator(0)(s), Ok((t, (&s[0..10], Operator::Mode(0o777)))));
        assert_eq!(
            operator(0)(t),
            Ok((u, (&s[10..23], Operator::Owner("usr-1"))))
        );
        assert_eq!(
            operator(0)(u),
            Ok(("", (&s[23..], Operator::Group("grpX"))))
        );
    }

    #[test]
    fn test_match_pattern() {
        let s = "#match [A-Z][A-Za-z]+";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Match(Expression::new(vec![Token::text("[A-Z][A-Za-z]+")]))
                )
            ))
        )
    }

    #[test]
    fn test_source_pattern() {
        let s = "#source /a/file/path";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Source(Expression::new(vec![Token::text("/a/file/path")]))
                )
            ))
        )
    }

    #[test]
    fn test_def_with_newline() {
        let s = "#def defined/\n";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Def {
                        name: Identifier::new("defined"),
                        is_directory: true,
                        link: None,
                        children: vec![]
                    }
                )
            ))
        );
    }

    #[test]
    fn test_def_with_block() {
        let s = "#def defined/\
               \n    file\
               \n    dir/";
        assert_eq!(
            operator(0)(s),
            Ok((
                "",
                (
                    s,
                    Operator::Def {
                        name: Identifier::new("defined"),
                        is_directory: true,
                        link: None,
                        children: vec![
                            (
                                &s[14..23],
                                Operator::Item {
                                    binding: Binding::Static("file"),
                                    is_directory: false,
                                    link: None,
                                    children: vec![],
                                }
                            ),
                            (
                                &s[23..],
                                Operator::Item {
                                    binding: Binding::Static("dir"),
                                    is_directory: true,
                                    link: None,
                                    children: vec![],
                                }
                            )
                        ]
                    }
                )
            ))
        );
    }

    #[test]
    fn test_usage() {
        let s = concat!(
            "#def defined/\n",
            "    file\n",
            "        #source $emptyfile\n",
            "usage/\n",
            "    #use defined\n"
        );
        // Some important positions
        let def_pos = 0;
        let file_pos = s.find("    file").unwrap();
        let source_pos = s.find("        #source").unwrap();
        let usage_pos = s.find("usage").unwrap();
        let use_pos = s.find("    #use").unwrap();

        // Test raw operators parsed from the "file"
        let ops = many0(operator(0))(s);
        assert_eq!(
            ops,
            Ok((
                "",
                vec![
                    (
                        &s[def_pos..usage_pos],
                        Operator::Def {
                            name: Identifier::new("defined"),
                            is_directory: true,
                            link: None,
                            children: vec![(
                                &s[file_pos..usage_pos],
                                Operator::Item {
                                    binding: Binding::Static("file"),
                                    is_directory: false,
                                    link: None,
                                    children: vec![(
                                        &s[source_pos..usage_pos],
                                        Operator::Source(Expression::new(vec![Token::Variable(
                                            Identifier::new("emptyfile")
                                        )]))
                                    )],
                                }
                            )],
                        }
                    ),
                    (
                        &s[usage_pos..],
                        Operator::Item {
                            binding: Binding::Static("usage"),
                            is_directory: true,
                            link: None,
                            children: vec![(
                                &s[use_pos..],
                                Operator::Use {
                                    name: Identifier::new("defined")
                                }
                            )]
                        }
                    )
                ]
            ))
        );

        // Check the schema this builds
        let no_vars = || HashMap::default();
        let no_defs = || HashMap::default();
        let no_meta = || Meta::default();
        assert_eq!(
            schema(s, s, ops.unwrap().1, ItemType::Directory).map_err(|e| format!("{}", e.1)),
            Ok((
                None,
                Subschema::Original(Schema::Directory({
                    let mut defs = HashMap::new();
                    defs.insert(
                        Identifier::new("defined"),
                        Schema::Directory({
                            DirectorySchema::new(
                                no_vars(),
                                no_defs(),
                                no_meta(),
                                vec![SchemaEntry {
                                    criteria: Match::fixed("file"),
                                    schema: Subschema::Original(Schema::File(FileSchema::new(
                                        no_meta(),
                                        Expression::new(vec![Token::Variable(Identifier::new(
                                            "emptyfile",
                                        ))]),
                                    ))),
                                }],
                            )
                        }),
                    );
                    let entries = vec![SchemaEntry {
                        criteria: Match::fixed("usage"),
                        schema: Subschema::Referenced {
                            definition: Identifier::new("defined"),
                            overrides: Schema::Directory(DirectorySchema::new(
                                no_vars(),
                                no_defs(),
                                no_meta(),
                                vec![],
                            )),
                        },
                    }];
                    DirectorySchema::new(no_vars(), defs, no_meta(), entries)
                }),)
            ))
        );
    }
}
