use anyhow::Result;
use indoc::indoc;

use crate::{
    filesystem::{Filesystem, MemoryFilesystem},
    schema::parse_schema,
    traverse::traverse,
};

#[test]
fn test_traverse() -> Result<()> {
    let schema_root = parse_schema(indoc!(
        "
        subdir/
        $any/
            deeper
                #source res/$any
        ",
    ))
    .unwrap();
    let filesystem = MemoryFilesystem::new();
    traverse(&schema_root, &filesystem, "/")?;
    assert!(filesystem.is_directory("/"));
    Ok(())
}

#[test]
fn test_create_directory() -> Result<()> {
    let root = parse_schema(indoc!(
        "
        subdir/
            subfile
                #source something
        $var/
            foo
                #source /tmp/resource/$var.foo
    "
    ))?;
    // Initialize root for filesystem
    let fs = MemoryFilesystem::new();
    fs.create_directory("/tmp")?;
    fs.create_directory("/tmp/new")?;

    // Initialize an existing directory that will match the $var pattern
    fs.create_directory("/tmp/new/one")?;

    // TRAVERSE
    traverse(&root, &fs, "/tmp/new")?;

    // Now check the outcome
    assert!(fs.is_directory("/tmp/new/one"));
    assert!(fs.is_file("/tmp/new/one/foo"));
    Ok(())
}
