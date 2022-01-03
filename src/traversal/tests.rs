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
            subsubdir/
    "
    ))?;
    // Initialize root for filesystem
    let fs = MemoryFilesystem::new();
    assert!(!fs.is_directory("/primary"));

    // TRAVERSE
    traverse(&root, &fs, "/primary")?;

    // Now check the outcome
    assert!(fs.is_directory("/primary"));
    assert!(fs.is_directory("/primary/subdir"));
    assert!(fs.is_directory("/primary/subdir/subsubdir"));
    Ok(())
}

#[test]
fn test_create_file() -> Result<()> {
    let root = parse_schema(indoc!(
        "
        subdirlink/
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
    assert!(fs.is_file("/primary/subdirlink/subfile"));
    assert_eq!(
        fs.read_file("/primary/subdirlink/subfile").unwrap(),
        "FILE CONTENT".to_owned()
    );
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
