use std::vec;

use nom::{
    branch::alt,
    character::complete::{alphanumeric1, line_ending},
    combinator::{eof, recognize},
    multi::many0,
    sequence::{preceded, terminated},
};

use crate::schema::{
    expr::{Expression, Identifier, Token},
    text::{blank_line, def_header, end_of_lines, indentation, operator, parse_schema, Operator},
    Binding, DirectorySchema, FileSchema, Schema, SchemaNode,
};

#[test]
fn test_invalid_space() {
    assert!(parse_schema("okay_entry/").is_ok());
    assert!(parse_schema("invalid entry/").is_err());
}

#[test]
fn test_indentation() {
    assert!(operator(0)("entry/").is_ok());
    assert!(operator(0)("  entry/").is_err());
    assert!(operator(1)("  entry/").is_err());
    assert!(operator(1)("    entry/").is_ok());

    assert!(parse_schema("entry/").is_ok());
    assert!(parse_schema("    entry/").is_ok());
}

#[test]
fn test_line_endings() {
    let text = "line1\n\nline3\n";
    let (rem, ws) = preceded(alphanumeric1, end_of_lines)(text).unwrap();
    assert_eq!(ws, "\n\n");
    let (rem, ws) = preceded(alphanumeric1, end_of_lines)(rem).unwrap();
    assert_eq!(ws, "\n");
    assert_eq!(rem, "");
}
#[test]
fn test_extraneous_whitespace() {
    // Baseline
    let text = "dir/";
    let schema = parse_schema(text).unwrap();
    assert!(
        matches!(schema, SchemaNode { schema: Schema::Directory(DirectorySchema { entries, .. }), ..} if entries.len() == 1)
    );
    // Trailing whitespace
    let text = "dir/\n\n";
    let schema = parse_schema(text).unwrap();
    assert!(
        matches!(schema, SchemaNode { schema: Schema::Directory(DirectorySchema { entries, .. }), ..} if entries.len() == 1)
    );
    // Preceding whitespace
    let text = "\n\ndir/";
    let schema = parse_schema(text).unwrap();
    assert!(
        matches!(schema, SchemaNode { schema: Schema::Directory(DirectorySchema { entries, .. }), ..} if entries.len() == 1)
    );
}

/// Operators should span a number of whole lines:
/// the actual operator, any children, and any subsequent blank lines
#[test]
fn test_operator_span() {
    // Each line is 10 characters including \n to make the indexing easier
    let text = "\
          a23456789\
        \nb23456789\
        \n         \
        \nc23456789\
        \n";
    let (rem, op) = recognize(operator(0))(text).unwrap();
    assert_eq!(op, &text[0..10]); // 1st line only
    assert_eq!(rem, &text[10..]);
    let (rem, op) = recognize(operator(0))(rem).unwrap();
    assert_eq!(op, &text[10..30]); // 2nd line and 3rd (blank) line
    assert_eq!(rem, &text[30..]);
    let (rem, op) = recognize(operator(0))(rem).unwrap();
    assert_eq!(op, &text[30..40]); // Last line
    assert_eq!(rem, "");

    let text = "\
          a2345678/\
        \n    b6789\
        \nc23456789\
        \n";
    let (rem, op) = recognize(operator(0))(text).unwrap();
    assert_eq!(op, &text[0..20]); // 1st and 2nd lines
    assert_eq!(rem, &text[20..]);
}

#[test]
fn test_invalid_child() {
    parse_schema(
        "
        okay_entry
            #source /tmp
        ",
    )
    .unwrap();
    parse_schema(
        "
        okay_entry/
            child
                #source /tmp
        ",
    )
    .unwrap();
    assert!(parse_schema(
        "
        okay_entry
            child
                #source /tmp
        "
    )
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
                    expr: Expression::from_parsed("expr", vec![Token::Text("expr")])
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
                    expr: Expression::from_parsed("expr", vec![Token::Text("expr")])
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
                    expr: Expression::from_parsed("expr", vec![Token::Text("expr")])
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
                    link: Some(Expression::from_parsed(
                        "/somewhere/else",
                        vec![Token::Text("/somewhere/else")]
                    )),
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
                    link: Some(Expression::from_parsed(
                        "/some$where/else",
                        vec![
                            Token::Text("/some"),
                            Token::Variable(Identifier::new("where")),
                            Token::Text("/else")
                        ]
                    )),
                    children: vec![],
                }
            )
        ))
    );
}

/// Line ending may be a newline or the EOF
#[test]
fn test_line_ending() {
    assert_eq!(end_of_lines(""), Ok(("", "")));
    assert_eq!(end_of_lines("\n"), Ok(("", "\n")));
}

/// Trailing whitespace is only allowed on otherwise blank lines
#[test]
fn test_no_trailing_whitespace() {
    let s = "\n    \n\n    \n\n";
    assert!(matches!(end_of_lines(s), Ok(_)));
    let s = "    \n\n    \n\n";
    assert!(matches!(end_of_lines(s), Err(_)));
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
    assert!(operator(0)("#mode 777").is_ok());
    assert!(operator(0)("#mode 777 ").is_err());
    assert!(operator(0)("#mode 777 #owner x").is_err());
    assert!(operator(0)("#mode 777\n#owner x").is_ok());
}

#[test]
fn test_trailing_whitespace() {
    parse_schema("").unwrap();
    assert!(parse_schema("dir/    \n").is_err());
    parse_schema("dir/\n").unwrap();
    parse_schema("dir/\n    \n").unwrap();
    parse_schema("dir/\n    ").unwrap();
}

#[test]
fn test_multiline_meta_ops() {
    let s = "
        #mode 777
        #owner usr-1
        #group grpX
        "
    .strip_prefix("\n")
    .unwrap();

    let line = "        #mode 777\n";
    let pos = s.find(line).unwrap();
    let end = pos + line.len();
    let t = &s[end..];
    assert_eq!(
        operator(2)(s),
        Ok((t, (&s[pos..end], Operator::Mode(0o777))))
    );

    let line = "        #owner usr-1\n";
    let pos = s.find(line).unwrap();
    let end = pos + line.len();
    let u = &s[end..];
    assert_eq!(
        operator(2)(t),
        Ok((u, (&s[pos..end], Operator::Owner("usr-1"))))
    );
    let line = "        #group grpX\n";
    let pos = s.find(line).unwrap();
    assert_eq!(
        operator(2)(u),
        Ok(("", (&s[pos..], Operator::Group("grpX"))))
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
                Operator::Match(Expression::from_parsed(
                    "[A-Z][A-Za-z]+",
                    vec![Token::Text("[A-Z][A-Za-z]+")]
                ))
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
                Operator::Source(Expression::from_parsed(
                    "/a/file/path",
                    vec![Token::Text("/a/file/path")]
                ))
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
    let s = "
        #def defined/
            file
            dir/
    ";
    assert_eq!(
        preceded(many0(blank_line), operator(2))(s),
        Ok((
            "",
            (
                &s[1..], // Skip /n
                Operator::Def {
                    name: Identifier::new("defined"),
                    is_directory: true,
                    link: None,
                    children: vec![
                        (
                            &s[s.find("            file").unwrap()
                                ..s.find("            dir").unwrap()],
                            Operator::Item {
                                binding: Binding::Static("file"),
                                is_directory: false,
                                link: None,
                                children: vec![],
                            }
                        ),
                        (
                            &s[s.find("            dir").unwrap()..],
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
    let s = "
        #def defined/
            file
                #source $emptyfile
        usage/
            #use defined
        ";
    // Some important positions
    let def_pos = s.find("        #def").unwrap();
    let file_pos = s.find("            file").unwrap();
    let source_pos = s.find("                #source").unwrap();
    let usage_pos = s.find("        usage").unwrap();
    let use_pos = s.find("            #use").unwrap();

    // Test raw operators parsed from the "file"
    let ops = preceded(many0(blank_line), many0(operator(2)))(s);
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
                                    Operator::Source(Expression::from_parsed(
                                        "$emptyfile",
                                        vec![Token::Variable(Identifier::new("emptyfile"))]
                                    ))
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
}

#[test]
fn test_duplicate() {
    let schema = "
        directory/
            #owner admin

            subdirectory/
                #owner admin
                #mode 777
                #owner admin
        ";
    let pos = schema.rfind("#owner").unwrap();
    parse_schema(&schema[..pos]).unwrap();

    let err = match parse_schema(schema) {
        Err(e) => e,
        ok => panic!("Unexpected: {:?}", ok),
    };
    let e = err.into_iter().last().unwrap();
    assert_eq!(e.line_number(), 8);
}

#[test]
fn test_symlink_directory() {
    let schema = parse_schema(
        "
        directory/ -> /another/place
        ",
    )
    .unwrap();
    let (bind, node) = match &schema {
        SchemaNode {
            schema: Schema::Directory(DirectorySchema { entries, .. }),
            ..
        } => &entries[0],
        _ => panic!(),
    };
    assert_eq!(bind, &Binding::Static("directory"));
    let (symlink, entries) = match node {
        SchemaNode {
            symlink,
            schema: Schema::Directory(DirectorySchema { entries, .. }),
            ..
        } => (symlink, entries),
        _ => panic!(),
    };
    assert_eq!(entries.len(), 0);
    assert_eq!(
        symlink,
        &Some(Expression::from_parsed(
            "/another/place",
            vec![Token::Text("/another/place")]
        ))
    );
}

#[test]
fn test_symlink_file() {
    let schema = parse_schema(
        "
        file -> /another/place
            #source xxx
        ",
    )
    .unwrap();
    let (bind, node) = match &schema {
        SchemaNode {
            schema: Schema::Directory(DirectorySchema { entries, .. }),
            ..
        } => &entries[0],
        _ => panic!(),
    };
    assert_eq!(bind, &Binding::Static("file"));
    let symlink = match node {
        SchemaNode {
            symlink,
            schema: Schema::File(FileSchema { .. }),
            ..
        } => symlink,
        _ => panic!(),
    };
    assert_eq!(
        symlink,
        &Some(Expression::from_parsed(
            "/another/place",
            vec![Token::Text("/another/place")]
        ))
    );
}
