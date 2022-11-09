use std::collections::HashMap;

use super::{
    parse_schema, Attributes, Binding, DirectorySchema, Identifier, SchemaNode, SchemaType,
};

#[test]
fn test_def_is_recorded() {
    let root = parse_schema(":def empty/").unwrap();
    let root_directory = root.schema.as_directory().unwrap();
    assert_eq!(root_directory.defs().len(), 1);

    let root = parse_schema(":def empty/\n:def another/").unwrap();
    let root_directory = root.schema.as_directory().unwrap();
    assert_eq!(root_directory.defs().len(), 2);
}

#[test]
fn test_use_is_recorded() {
    let root = parse_schema(":def empty/\nsub/\n    :use empty").unwrap();
    assert_eq!(root.uses.len(), 0);
    let root_directory = root.schema.as_directory().unwrap();
    assert_eq!(root_directory.entries.len(), 1);
    let sub = &root_directory.entries[0].1;
    assert_eq!(sub.uses.len(), 1);
}

#[test]
fn test_def_and_use_compare_equal() {
    let root = parse_schema(":def empty/\nsub/\n    :use empty").unwrap();
    let root_directory = root.schema.as_directory().unwrap();
    assert_eq!(root_directory.entries.len(), 1);
    let sub = &root_directory.entries[0].1;
    assert_eq!(sub.uses.len(), 1);
    let mut defs = root_directory.defs().keys();
    assert_eq!(defs.next(), Some(&sub.uses[0]));
    assert_eq!(defs.next(), None);
    assert!(root_directory.get_def(&"empty".into()).is_some());
    assert!(root_directory.get_def(&"none".into()).is_none());
}

#[test]
fn test_directory_binding_sort_order() {
    let empty_subdirectory = SchemaType::Directory(DirectorySchema::default());
    let empty_directory_node = SchemaNode {
        schema: empty_subdirectory,
        match_pattern: None,
        avoid_pattern: None,
        attributes: Attributes::default(),
        symlink: None,
        uses: vec![],
    };

    // Variable then static should re-order (so static is first)
    let entries = vec![
        (
            Binding::Dynamic(Identifier::new("var")),
            empty_directory_node.clone(),
        ),
        (Binding::Static("fixed"), empty_directory_node.clone()),
    ];
    let directory = DirectorySchema::new(HashMap::new(), HashMap::new(), entries);
    let entries = directory.entries();
    assert!(matches!(entries[0].0, Binding::Static(_)));
    assert!(matches!(entries[1].0, Binding::Dynamic(_)));

    // Static then variable should keep order (static first)
    let entries = vec![
        (Binding::Static("fixed"), empty_directory_node.clone()),
        (
            Binding::Dynamic(Identifier::new("var")),
            empty_directory_node.clone(),
        ),
    ];
    let directory = DirectorySchema::new(HashMap::new(), HashMap::new(), entries);
    let entries = directory.entries();
    assert!(matches!(entries[0].0, Binding::Static(_)));
    assert!(matches!(entries[1].0, Binding::Dynamic(_)));
}
