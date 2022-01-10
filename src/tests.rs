use std::collections::HashSet;

use anyhow::Result;

use crate::{
    filesystem::{Filesystem, MemoryFilesystem},
    schema::parse_schema,
    traversal::traverse,
};

macro_rules! assert_effect_of {
    {
        applying:
            $text:literal
        onto:
            $root:literal
            $(directories:
                $($in_directory:literal)+ )?
            $(files:
                $($in_file:literal [ $in_content:literal ])+ )?
            $(symlinks:
                $($in_link:literal -> $in_target:literal)+ )?
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
        let mut fs = MemoryFilesystem::new();
        let root = $root;
        // containing:
        let mut expected_paths: HashSet<String> = HashSet::new();
        $($(
            fs.create_directory($in_directory)?;
            expected_paths.insert($in_directory.to_owned());
        )+)?
        $($(
            fs.create_file($in_file, $in_content.to_owned())?;
            expected_paths.insert($in_file.to_owned());
        )+)?
        $($(
            fs.create_symlink($in_link, $in_target.to_owned())?;
            expected_paths.insert($in_link.to_owned());
        )+)?
        // yields:
        traverse(&node, &mut fs, root)?;
        expected_paths.insert("/".to_owned());
        expected_paths.insert(root.to_owned());
        $($(
            assert!(fs.is_directory($out_directory));
            expected_paths.insert($out_directory.to_owned());
        )+)?
        $($(
            assert!(fs.is_file($out_file));
            assert_eq!(&fs.read_file($out_file)?, $out_content);
            expected_paths.insert($out_file.to_owned());
        )+)?
        $($(
            assert!(fs.is_link($out_link));
            assert_eq!(&fs.read_link($out_link)?, $out_target);
            expected_paths.insert($out_link.to_owned());
        )+)?
        let actual_paths = fs.to_path_set();
        let unaccounted: Vec<_> = actual_paths.difference(&expected_paths).collect();
        if !unaccounted.is_empty() {
            panic!("Paths unaccounted for: {:?}", unaccounted);
        }
    }};
}

#[test]
fn test_create_directory() -> Result<()> {
    assert_effect_of! {
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
    assert_effect_of! {
        applying: "
            subdir/
                subsubfile
                    #source /resource/file1
            subfile
                #source /resource/file2
            "
        onto: "/primary"
            directories:
                "/resource"
            files:
                "/resource/file1" ["FILE CONTENT 1"]
                "/resource/file2" ["FILE CONTENT 2"]
        yields:
            directories:
                "/primary/subdir"
            files:
                "/primary/subdir/subsubfile" ["FILE CONTENT 1"]
                "/primary/subfile" ["FILE CONTENT 2"]
    };
    Ok(())
}

#[test]
fn test_create_symlink() -> Result<()> {
    assert_effect_of! {
        applying: "
            subdirlink/ -> /secondary/${NAME}
                subfile
                    #source /resource/file
            "
        onto: "/primary"
            directories:
                "/resource"
            files:
                "/resource/file" ["FILE CONTENT"]
        yields:
            directories:
                "/secondary"
                "/secondary/subdirlink"
            files:
                "/secondary/subdirlink/subfile" ["FILE CONTENT"]
            symlinks:
                "/primary/subdirlink" -> "/secondary/subdirlink"
    };
    Ok(())
}

#[test]
fn test_use_simple() -> Result<()> {
    assert_effect_of! {
        applying: "
            #def some_def/
                sub/
            
            inner/
                #use some_def
            "
        onto: "/"
        yields:
            directories:
                "/inner"
                "/inner/sub"
    };
    Ok(())
}

#[test]
fn test_use_at_top_level() -> Result<()> {
    assert_effect_of! {
        applying: "
            #use has_sub

            #def has_sub/
                sub/
            
            inner/
                #use has_sub
            "
        onto: "/"
        yields:
            directories:
                "/sub"
                "/inner"
                "/inner/sub"
    };
    Ok(())
}

#[test]
fn use_multiple() -> Result<()> {
    assert_effect_of! {
        applying: "
            #def def_a/
                sub_a/
            #def def_b/
                sub_b/
            
            inner/
                #use def_a
                #use def_b
                sub_c/
            "
        onto: "/"
        yields:
            directories:
                "/inner"
                "/inner/sub_a"
                "/inner/sub_b"
                "/inner/sub_c"
    };
    Ok(())
}
