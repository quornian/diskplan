use anyhow::Result;
use indoc::indoc;

use crate::{
    filesystem::{Filesystem, MemoryFilesystem},
    schema::parse_schema,
    traversal::traverse,
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

#[test]
fn test_create_symlink() -> Result<()> {
    let root = parse_schema(indoc!(
        "
        subdirlink/ -> /secondary/${NAME}
            subfile
                #source /resource/file
    "
    ))?;
    // Initialize filesystem
    let fs = MemoryFilesystem::new();
    fs.create_directory("/primary")?;
    fs.create_directory("/resource")?;
    fs.create_file("/resource/file", "FILE CONTENT".into())?;

    traverse(&root, &fs, "/primary")
        .map_err(|e| anyhow::anyhow!("{}\n{:#?}", e, fs))
        .unwrap();

    // Now check the outcome
    assert_eq!(
        fs.read_link("/primary/subdirlink")
            .map_err(|e| e.to_string()),
        Ok("/secondary/subdirlink".to_owned())
    );
    assert!(fs.is_file("/secondary/subdirlink/subfile"));
    Ok(())
}
