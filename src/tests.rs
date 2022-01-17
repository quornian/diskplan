use std::{collections::HashSet, fmt::Debug};

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
                $($in_d_path:literal $([
                    $(owner = $in_d_owner:literal)?
                    $(group = $in_d_group:literal)?
                    $(mode = $in_d_mode:expr)? ])? )+ )?
            $(files:
                $($in_f_path:literal [
                    $in_content:literal
                    $(owner = $in_f_owner:literal)?
                    $(group = $in_f_group:literal)?
                    $(mode = $in_f_mode:expr)? ])+ )?
            $(symlinks:
                $($in_l_path:literal -> $in_l_target:literal)+ )?
        yields:
            $(directories:
                $($out_d_path:literal $([
                    $(owner = $out_d_owner:literal)?
                    $(group = $out_d_group:literal)?
                    $(mode = $out_d_mode:expr)? ])? )+ )?
            $(files:
                $($out_f_path:literal [
                    $content:literal
                    $(owner = $out_f_owner:literal)?
                    $(group = $out_f_group:literal)?
                    $(mode = $out_f_mode:expr)? ])+ )?
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
            #[allow(unused_mut)]
            let mut attrs = SetAttrs::default();
            $(
                $(attrs.owner = Some($in_d_owner);)?
                $(attrs.group = Some($in_d_group);)?
                $(attrs.mode = Some($in_d_mode);)?
            )?
            fs.create_directory($in_d_path, attrs)?;
            expected_paths.insert($in_d_path.to_owned());
        )+)?
        $($(
            #[allow(unused_mut)]
            let mut attrs = SetAttrs::default();
            $(
                $(attrs.owner = Some($in_f_owner);)?
                $(attrs.group = Some($in_f_group);)?
                $(attrs.mode = Some($in_f_mode);)?
            )?
            fs.create_file($in_f_path, attrs, $in_content.to_owned())?;
            expected_paths.insert($in_f_path.to_owned());
        )+)?
        $($(
            fs.create_symlink($in_l_path, $in_l_target.to_owned())?;
            expected_paths.insert($in_l_path.to_owned());
        )+)?
        // yields:
        traverse(&node, &mut fs, root)?;
        expected_paths.insert("/".to_owned());
        expected_paths.insert(root.to_owned());
        $($(
            assert!(fs.is_directory($out_d_path));
            $(
                let attrs = fs.attributes($out_d_path)?;
                $(assert_eq!(attrs.owner.as_ref(), $out_d_owner);)?
                $(assert_eq!(attrs.group.as_ref(), $out_d_group);)?
                $(assert_eq!(Mode(attrs.mode), Mode($out_d_mode));)?
            )?
            expected_paths.insert($out_d_path.to_owned());
        )+)?
        $($(
            assert!(fs.is_file($out_f_path));
            $(
                let attrs = fs.attributes($out_f_path)?;
                $(assert_eq!(attrs.owner.as_ref(), $out_f_owner);)?
                $(assert_eq!(attrs.group.as_ref(), $out_f_group);)?
                $(assert_eq!(Mode(attrs.mode), Mode($out_f_mode));)?
            )?
            assert_eq!(&fs.read_file($out_f_path)?, $content);
            expected_paths.insert($out_f_path.to_owned());
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

#[derive(PartialEq)]
struct Mode(u16);

impl Debug for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mode(0o{:03o})", self.0)
    }
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
                #owner daemon
                #group sys
            "
        onto: "/target"
        yields:
            directories:
                "/target" [mode = DEFAULT_DIRECTORY_MODE]
                "/target/dir" [mode = 0o640]
                "/target/another" [
                    owner = "daemon"
                    group = "sys"
                    mode = DEFAULT_DIRECTORY_MODE]
    }
}

#[test]
fn test_top_level_attributes() -> Result<()> {
    use crate::filesystem::DEFAULT_DIRECTORY_MODE;
    assert_effect_of! {
        applying: "
            #mode 640
            #owner daemon
            #group sys
            sub/
            "
        onto: "/target"
        yields:
            directories:
                "/target" [
                    owner = "daemon"
                    group = "sys"
                    mode = 0o640]
                "/target/sub" [
                    owner = "root"
                    group = "root"
                    mode = DEFAULT_DIRECTORY_MODE]
    }
}

#[test]
fn test_attribute_expressions() -> Result<()> {
    use crate::filesystem::DEFAULT_DIRECTORY_MODE;
    assert_effect_of! {
        applying: "
            #let x = dae
            #let y = s
            attrs/
                #owner ${x}mon
                #group ${y}y${y}
            "
        onto: "/target"
        yields:
            directories:
                "/target/attrs" [
                    owner = "daemon"
                    group = "sys"
                    mode = DEFAULT_DIRECTORY_MODE]
    }
}

#[test]
fn test_changing_attributes() -> Result<()> {
    use crate::filesystem::DEFAULT_DIRECTORY_MODE;
    assert_effect_of! {
        applying: "
            dir/
                #mode 750
            "
        onto: "/target"
            directories:
                "/target"
                "/target/control" [mode = 0o555]
                "/target/dir" [mode = 0o555]
        yields:
            directories:
                "/target/control" [mode = 0o555]
                "/target/dir" [mode = 0o750]
    }
}

#[test]
fn test_binding_static_beats_dynamic() -> Result<()> {
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
fn test_binding_static_beats_dynamic_reordered() -> Result<()> {
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
                "/fixed/MATCHED_FIXED"
    }
}

#[test]
#[should_panic(
    expected = "'existing' matches multiple dynamic bindings '$variable_a' and '$variable_b'"
)]
fn test_binding_multiple_variable_error() {
    (|| -> Result<()> {
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
                    "/existing"
            yields:
                directories:
                    "/existing/MATCHED_VARIABLE_A"
        }
    })()
    .unwrap();
}

#[test]
#[should_panic(
    expected = "'duplicate' matches multiple static bindings 'duplicate' and 'duplicate'"
)]
fn test_binding_multiple_static_error() {
    (|| -> Result<()> {
        assert_effect_of! {
            applying: "
            duplicate/
            duplicate/
            "
            onto: "/"
            yields:
        }
    })()
    .unwrap();
}

#[test]
fn test_match_let_variable() -> Result<()> {
    assert_effect_of! {
        applying: "
            #let var = xxx
            $var/
                #match .*
                variable/
            "
        onto: "/target"
        yields:
            directories:
                "/target/xxx"
                "/target/xxx/variable"
    }
}

#[test]
fn test_match_let_variable_overridden_by_static() -> Result<()> {
    // TODO: Consider if this should fail
    assert_effect_of! {
        applying: "
            #let var = xxx
            $var/
                #match .*
                variable/
            xxx/
                static/
            "
        onto: "/target"
        yields:
            directories:
                "/target/xxx"
                "/target/xxx/static"
    }
}

#[test]
fn test_match_variable() -> Result<()> {
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
            files:
                "/src/empty" [""]
        yields:
            files:
                "/target/x_at_the_beginning/starts" [""]
                "/target/ends_with_an_x/ends" [""]
    }
}

#[test]
fn test_match_variable_inherited() -> Result<()> {
    assert_effect_of! {
        applying: "
            $var/
                #match .*
                $var/
                sub/
                    $var/
            "
        onto: "/target"
            directories:
                "/target"
                "/target/VALUE"
        yields:
            directories:
                "/target/VALUE/VALUE"
                "/target/VALUE/sub"
                "/target/VALUE/sub/VALUE"
    }
}

#[test]
fn test_match_categories() -> Result<()> {
    assert_effect_of! {
        applying: "
            $building/
                #match .*shed
                BUILDING/
            $animal/
                #match .*
                #avoid .*shed
                ANIMAL/
            "
        onto: "/target"
            directories:
                "/target"
                "/target/cow"
                "/target/shed"
                "/target/cow_shed"
                "/target/chicken"
        yields:
            directories:
                "/target/cow/ANIMAL"
                "/target/shed/BUILDING"
                "/target/cow_shed/BUILDING"
                "/target/chicken/ANIMAL"
    }
}
