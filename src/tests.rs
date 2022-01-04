use anyhow::Result;

use crate::{
    filesystem::{Filesystem, MemoryFilesystem},
    schema::parse_schema,
    traversal::traverse,
};

macro_rules! assert_effect {
    {
        applying:
            $text:literal
        onto:
            $root:literal
        $(
        containing:
            $(directories:
                $($in_directory:literal)+ )?
            $(files:
                $($in_file:literal [ $in_content:literal ])+ )?
            $(symlinks:
                $($in_link:literal -> $in_target:literal)+ )?
        )?
        yields:
            $(directories:
                $($out_directory:literal)+ )?
            $(files:
                $($out_file:literal [ $out_content:literal ])+ )?
            $(symlinks:
                $($out_link:literal -> $out_target:literal)+ )?
    } => {{
        // applying:
        let node = parse_schema($text)?;
        // onto:
        let fs = MemoryFilesystem::new();
        let root = $root;
        // containing:
        $(
        $($(fs.create_directory($in_directory)?;)+)?
        $($(fs.create_file($in_file, $in_content.to_owned())?;)+)?
        $($(fs.create_symlink($in_link, $in_target.to_owned())?;)+)?
        )?
        // yields:
        traverse(&node, &fs, root)?;
        $($(assert!(fs.is_directory($out_directory));)+)?
        $($(
            assert!(fs.is_file($out_file));
            assert_eq!(&fs.read_file($out_file)?, $out_content);
        )+)?
        $($(
            assert!(fs.is_link($out_link));
            assert_eq!(&fs.read_link($out_link)?, $out_target);
        )+)?
    }};
}

#[test]
fn test_create_directory() -> Result<()> {
    assert_effect! {
        applying: "
            subdir/
                subsubdir/
            "
        onto:
            "/primary"
        yields:
            directories:
                "/primary"
                "/primary/subdir"
                "/primary/subdir/subsubdir"
    };
    Ok(())
}

#[test]
fn test_create_file() -> Result<()> {
    assert_effect! {
        applying: "
            subdir/
                subsubfile
                    #source /resource/file1
            subfile
                #source /resource/file2
            "
        onto:
            "/primary"
        containing:
            directories:
                "/resource"
            files:
                "/resource/file1" ["FILE CONTENT 1"]
                "/resource/file2" ["FILE CONTENT 2"]
        yields:
            files:
                "/primary/subdir/subsubfile" ["FILE CONTENT 1"]
                "/primary/subfile" ["FILE CONTENT 2"]
    };
    Ok(())
}

#[test]
fn test_create_symlink() -> Result<()> {
    assert_effect! {
        applying: "
            subdirlink/ -> /secondary/${NAME}
                subfile
                    #source /resource/file
            "
        onto:
            "/primary"
        containing:
            directories:
                "/resource"
            files:
                "/resource/file" ["FILE CONTENT"]
        yields:
            files:
                "/secondary/subdirlink/subfile" ["FILE CONTENT"]
            symlinks:
                "/primary/subdirlink" -> "/secondary/subdirlink"
    };
    Ok(())
}
