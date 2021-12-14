use std::{collections::HashMap, fmt::Display};

use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, line_ending, space0, space1},
    combinator::{all_consuming, consumed, eof, map, opt, recognize},
    error::{context, VerboseError, VerboseErrorKind},
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

#[derive(Debug, PartialEq)]
pub struct ParseError<'a> {
    error: String,
    text: &'a str,
    span: &'a str,
    next: Option<Box<ParseError<'a>>>,
}

impl Display for ParseError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lineno = find_line_number(self.span, self.text);
        let line = self.text.lines().nth(lineno - 1).unwrap_or("<EOF>");
        let column = self.span.as_ptr() as usize - line.as_ptr() as usize;
        write!(f, "Error: {}\n", self.error)?;
        write!(f, "     |\n")?;
        write!(f, "{:4} | {}\n", lineno, line)?;
        if column == 0 {
            write!(f, "     |\n")?;
        } else {
            write!(f, "     | {0:1$}^\n", "", column)?;
        }
        if let Some(next) = &self.next {
            write!(f, "{}", next)?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError<'_> {}

impl<'a> ParseError<'a> {
    pub fn new(
        error: String,
        text: &'a str,
        span: &'a str,
        next: Option<Box<ParseError<'a>>>,
    ) -> ParseError<'a> {
        ParseError {
            error,
            text,
            span,
            next,
        }
    }
}

pub fn parse_schema(text: &str) -> std::result::Result<Schema, ParseError> {
    // Parse and process entire schema and handle any errors that arise
    let (_, ops) = all_consuming(many0(operator(0)))(&text).map_err(|e| {
        let e = match e {
            nom::Err::Error(e) | nom::Err::Failure(e) => e,
            nom::Err::Incomplete(_) => unreachable!(),
        };
        let mut error = None;
        for (r, e) in e.errors.iter().rev() {
            error = Some(ParseError::new(
                match e {
                    VerboseErrorKind::Nom(p) => format!("Invalid token while looking for: {:?}", p),
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

fn find_line_number(pos: &str, whole: &str) -> usize {
    let pos = pos.as_ptr() as usize - whole.as_ptr() as usize;
    whole[..pos].chars().filter(|&c| c == '\n').count() + 1
}

fn schema<'a>(
    whole: &'a str,
    part: &'a str,
    ops: Vec<(&'a str, Operator<'a>)>,
    item_type: ItemType,
) -> std::result::Result<(Option<Expression>, Subschema), ParseError<'a>> {
    let mut props = Properties::default();
    for (span, op) in ops {
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
                    _ => {
                        return Err(ParseError::new(
                            "Only files can have a #source".into(),
                            whole,
                            span,
                            None,
                        ))
                    }
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
                props.entries.push(SchemaEntry { criteria, schema });
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
                    props.defs.insert(name, schema);
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
            // Files must have a #source unless they are #use-ing a definition from elsewhere
            let source = if let Some(source) = props.source {
                source
            } else if props.use_def.is_some() {
                Expression::new(vec![])
            } else {
                return Err(ParseError::new(
                    format!("File has no #source (or #use). Should this have been a directory?"),
                    whole,
                    part,
                    None,
                ));
            };
            Schema::File(FileSchema::new(props.meta.build(), source))
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
                    return Err(ParseError::new(
                        format!("File has no #source. Should this have been a directory?"),
                        whole,
                        part,
                        None,
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
fn end_of_lines(s: &str) -> Res<&str, ()> {
    // TODO: This allows trailing whitespace, disallow that?
    //value((), many0(preceded(space0, alt((line_ending, eof)))))(s)
    map(
        alt((
            recognize(many1(preceded(space0, line_ending))),
            preceded(space0, eof),
        )),
        |_| (),
    )(s)
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
