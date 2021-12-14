use std::{collections::HashMap, vec};

use indoc::indoc;
use nom::{
    branch::alt,
    character::complete::line_ending,
    combinator::eof,
    multi::many0,
    sequence::{preceded, terminated},
};

use crate::schema::{
    criteria::Match,
    expr::{Expression, Identifier, Token},
    meta::Meta,
    text::{
        def_header, end_of_lines, indentation, operator, parse_schema, schema, Binding, ItemType,
        Operator,
    },
    DirectorySchema, FileSchema, Schema, SchemaEntry, Subschema,
};

#[test]
fn test_invalid_space() {
    assert!(parse_schema("okay_entry/").is_ok());
    assert!(parse_schema("invalid entry/").is_err());
}
#[test]
fn test_invalid_child() {
    assert!(parse_schema(indoc!(
        "
        okay_entry
            #source /tmp
        "
    ))
    .is_ok());
    assert!(parse_schema(indoc!(
        "
        okay_entry/
            child
                #source /tmp
        "
    ))
    .is_ok());
    assert!(parse_schema(indoc!(
        "
        okay_entry
            child
                #source /tmp
        "
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
    assert!(operator(0)(s).is_err());
    let s = "#def something/->";
    assert!(operator(0)(s).is_err());
    let s = "#def something/->x";
    assert!(operator(0)(s).is_ok());
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
    assert_eq!(end_of_lines(s), Ok(("", ())));
}
#[test]
fn test_blank_line() {
    let s = "\n";
    assert_eq!(end_of_lines(s), Ok(("", ())));
}

#[test]
fn test_blankish_line() {
    let s = "    \n";
    assert_eq!(end_of_lines(s), Ok(("", ())));
}

#[test]
fn test_blankish_lines() {
    let s = "    \n\n    \n\n";
    assert_eq!(end_of_lines(s), Ok(("", ())));
}

#[test]
fn test_single_line_mode_op() {
    let s = "#mode 777";
    assert_eq!(operator(0)(s), Ok(("", (s, Operator::Mode(0o777)))));
}

#[test]
fn test_single_line_mode_trailing() {
    assert!(operator(0)("#mode 777#owner x").is_err());
    assert!(operator(0)("#mode 777-").is_err());
    let s = "#mode 777 ";
    assert_eq!(operator(0)(s), Ok(("", (s, Operator::Mode(0o777)))));
    assert!(operator(0)("#mode 777 #owner x").is_err());
    assert!(operator(0)("#mode 777\n#owner x").is_ok());
}

#[test]
fn test_multiline_meta_ops() {
    let s = indoc!(
        "
        #mode 777
        #owner usr-1
        #group grpX
        "
    );
    let t = indoc!(
        "
        #owner usr-1
        #group grpX
        "
    );
    let u = indoc!(
        "
        #group grpX
        "
    );
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
    let s = indoc!(
        "
        #def defined/
            file
                #source $emptyfile
        usage/
            #use defined
        "
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
        schema(s, s, ops.unwrap().1, ItemType::Directory),
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
