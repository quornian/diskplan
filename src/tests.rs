use std::collections::HashSet;

use anyhow::Result;

use crate::{
    filesystem::{Filesystem, MemoryFilesystem, SetAttrs},
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
                $($directory:literal $([
                    $(owner = $d_owner:literal)?
                    $(group = $d_group:literal)?
                    $(mode = $d_mode:expr)? ])? )+ )?
            $(files:
                $($file:literal [
                    $content:literal
                    $(owner = $f_owner:literal)?
                    $(group = $f_group:literal)?
                    $(mode = $f_mode:expr)? ])+ )?
            $(symlinks:
                $($link:literal -> $target:literal)+ )?
    } => {{
        // applying:
        let node = parse_schema($text)?;
        // onto:
        let mut fs = MemoryFilesystem::new();
        let root = $root;
        // containing:
        let mut expected_paths: HashSet<String> = HashSet::new();
        $($(
            fs.create_directory($in_directory, SetAttrs::default())?;
            expected_paths.insert($in_directory.to_owned());
        )+)?
        $($(
            fs.create_file($in_file, SetAttrs::default(), $in_content.to_owned())?;
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
            assert!(fs.is_directory($directory));
            $(
                let attrs = fs.attributes($directory)?;
                $(assert_eq!(attrs.owner.as_ref(), $d_owner);)?
                $(assert_eq!(attrs.group.as_ref(), $d_group);)?
                $(assert_eq!(attrs.mode, $d_mode);)?
            )?
            expected_paths.insert($directory.to_owned());
        )+)?
        $($(
            assert!(fs.is_file($file));
            $(
                let attrs = fs.attributes($file)?;
                $(assert_eq!(attrs.owner.as_ref(), $f_owner);)?
                $(assert_eq!(attrs.group.as_ref(), $f_group);)?
                $(assert_eq!(attrs.mode, $f_mode);)?
            )?
            assert_eq!(&fs.read_file($file)?, $content);
            expected_paths.insert($file.to_owned());
        )+)?
        $($(
            assert!(fs.is_link($link));
            assert_eq!(&fs.read_link($link)?, $target);
            expected_paths.insert($link.to_owned());
        )+)?
        let actual_paths = fs.to_path_set();
        let unaccounted: Vec<_> = actual_paths.difference(&expected_paths).collect();
        if !unaccounted.is_empty() {
            panic!("Paths unaccounted for: {:?}", unaccounted);
        }
        Ok(())
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
    }
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
    }
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
    }
}

#[test]
fn test_def_use_simple() -> Result<()> {
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
    }
}

#[test]
fn test_def_use_at_top_level() -> Result<()> {
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
    }
}

#[test]
fn test_def_use_multiple() -> Result<()> {
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
    }
}

#[test]
#[should_panic]
fn test_incorrect_attribute_assertion() {
    (|| -> Result<()> {
        assert_effect_of! {
            applying: "
                dir/
                    #mode 640
                "
            onto: "/target"
            yields:
                directories:
                    "/target/dir" [mode = 640] // This is incorrectly decimal
        }
    })()
    .unwrap_or_default();
}

#[test]
fn test_attributes() -> Result<()> {
    use crate::filesystem::DEFAULT_DIRECTORY_MODE;
    assert_effect_of! {
        applying: "
            dir/
                #mode 640
            another/
                #owner example_user
                #group example_group
            "
        onto: "/target"
        yields:
            directories:
                "/target" [mode = DEFAULT_DIRECTORY_MODE]
                "/target/dir" [mode = 0o640]
                "/target/another" [
                    owner = "example_user"
                    group = "example_group"
                    mode = DEFAULT_DIRECTORY_MODE]
    }
}

#[test]
fn test_top_level_attributes() -> Result<()> {
    use crate::filesystem::DEFAULT_DIRECTORY_MODE;
    assert_effect_of! {
        applying: "
            #mode 640
            #owner example_user
            #group example_group
            sub/
            "
        onto: "/target"
        yields:
            directories:
                "/target" [
                    owner = "example_user"
                    group = "example_group"
                    mode = 0o640]
                "/target/sub" [
                    owner = "root"
                    group = "root"
                    mode = DEFAULT_DIRECTORY_MODE]
    }
}

#[test]
fn test_winner_fixed_then_variable() -> Result<()> {
    assert_effect_of! {
        applying: "
            fixed/
                MATCHED_FIXED/
            $variable/
                #match .*
                MATCHED_VARIABLE/
            "
        onto: "/"
            directories:
                "/fixed"
        yields:
            directories:
                "/fixed/MATCHED_FIXED"
    }
}

#[test]
fn test_winner_variable_then_fixed() -> Result<()> {
    assert_effect_of! {
        applying: "
            $variable/
                #match .*
                MATCHED_VARIABLE/
            fixed/
                MATCHED_FIXED/
            "
        onto: "/"
            directories:
                "/fixed"
        yields:
            directories:
                "/fixed/MATCHED_VARIABLE"
    }
}

#[test]
fn test_winner_variable_then_variable() -> Result<()> {
    assert_effect_of! {
        applying: "
            $variable_a/
                #match .*
                MATCHED_VARIABLE_A/
            $variable_b/
                #match .*
                MATCHED_VARIABLE_B/
            "
        onto: "/"
            directories:
                "/fixed"
        yields:
            directories:
                "/fixed/MATCHED_VARIABLE_A"
    }
}

#[test]
fn test_match() -> Result<()> {
    assert_effect_of! {
        applying: "
            $a/
                #match x.*
                starts
                    #source /src/empty
            $b/
                #match .*x
                ends
                    #source /src/empty
            "
        onto: "/target"
            directories:
                "/src"
                "/target"
                "/target/has_an_x_in_it"
                "/target/x_at_the_beginning"
                "/target/ends_with_an_x"
                "/target/x_here_and_here_x" // Only first pattern will match...
            files:
                "/src/empty" [""]
        yields:
            files:
                "/target/x_at_the_beginning/starts" [""]
                "/target/ends_with_an_x/ends" [""]
                "/target/x_here_and_here_x/starts" [""]  // ...as seen here
    }
}
