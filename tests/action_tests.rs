use std::path::Path;

use diskplan::{self, context::Context, schema::MetaBuilder};
use indoc::indoc;

#[test]
fn simple_schema_actions() {
    let schema = diskplan::schema::parse_schema(indoc!(
        "
        example_directory/
        example_file
            #source emptyfile
        "
    ))
    .map_err(|e| format!("{}", e))
    .unwrap();
    let root = Path::new("/tmp/diskplan-root");
    let target = Path::new(".");
    let context = Context::new(&schema, &root, &target);
    let actions = diskplan::apply::gather_actions(&context).unwrap();
    let mut actions = actions.into_iter();
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateDirectory {
            path: root.into(),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateDirectory {
            path: root.join("example_directory"),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateFile {
            path: root.join("example_file"),
            source: "emptyfile".into(),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(actions.next(), None);
}

#[test]
fn simple_definition_schema_actions() {
    let schema = diskplan::schema::parse_schema(indoc!(
        "
        #def defined_directory/
            #owner user-a
        one/
            #use defined_directory
        two/
            #use defined_directory
            #owner user-b
        "
    ))
    .map_err(|e| format!("{}", e))
    .unwrap();
    let root = Path::new("/tmp/diskplan-root");
    let target = Path::new(".");
    let context = Context::new(&schema, &root, &target);
    let actions = diskplan::apply::gather_actions(&context).unwrap();
    let mut actions = actions.into_iter();
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateDirectory {
            path: root.into(),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateDirectory {
            path: root.join("one"),
            meta: MetaBuilder::default().owner("user-a").build(),
        })
    );
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateDirectory {
            path: root.join("two"),
            meta: MetaBuilder::default().owner("user-b").build(),
        })
    );
    assert_eq!(actions.next(), None);
}

#[test]
fn variable_in_definition_schema_actions() {
    let schema = diskplan::schema::parse_schema(indoc!(
        "
        #def dynamic_file
            #source ${level}.ext
        $level/
            file
                #use dynamic_file
        "
    ))
    .map_err(|e| format!("{}", e))
    .unwrap();
    let root = Path::new("/tmp/diskplan-root");
    std::fs::remove_dir_all(root).ok();
    std::fs::create_dir(root).unwrap();
    std::fs::create_dir(root.join("one")).unwrap();
    std::fs::create_dir(root.join("two")).unwrap();

    let target = Path::new(".");
    let context = Context::new(&schema, &root, &target);
    let actions = diskplan::apply::gather_actions(&context).unwrap();
    let mut actions = actions.into_iter();
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateDirectory {
            path: root.into(),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateDirectory {
            path: root.join("one"),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateFile {
            path: root.join("one/file"),
            source: "one.ext".into(),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateDirectory {
            path: root.join("two"),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(
        actions.next(),
        Some(diskplan::apply::Action::CreateFile {
            path: root.join("two/file"),
            source: "two.ext".into(),
            meta: MetaBuilder::default().build(),
        })
    );
    assert_eq!(actions.next(), None);
}
