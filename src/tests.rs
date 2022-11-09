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
        use std::collections::HashSet;

        use camino::Utf8Path;

        #[allow(unused_imports)]
        use crate::{
            filesystem::{Filesystem, MemoryFilesystem, SetAttrs},
            schema::parse_schema,
            traversal::traverse,
        };

        // applying:
        let node = parse_schema($text)?;
        // onto:
        let mut fs = MemoryFilesystem::new();
        let root = Utf8Path::new($root);
        // containing:
        let mut expected_paths: HashSet<&Utf8Path> = HashSet::new();
        $($(
            #[allow(unused_mut)]
            let mut attrs = SetAttrs::default();
            $(
                $(attrs.owner = Some($in_d_owner);)?
                $(attrs.group = Some($in_d_group);)?
                $(attrs.mode = Some($in_d_mode.into());)?
            )?
            fs.create_directory(Utf8Path::new($in_d_path), attrs)?;
            expected_paths.insert(Utf8Path::new($in_d_path));
        )+)?
        $($(
            #[allow(unused_mut)]
            let mut attrs = SetAttrs::default();
            $(
                $(attrs.owner = Some($in_f_owner);)?
                $(attrs.group = Some($in_f_group);)?
                $(attrs.mode = Some($in_f_mode.into());)?
            )?
            fs.create_file(Utf8Path::new($in_f_path), attrs, String::from($in_content))?;
            expected_paths.insert(Utf8Path::new($in_f_path));
        )+)?
        $($(
            fs.create_symlink(Utf8Path::new($in_l_path), Utf8Path::new($in_l_target))?;
            expected_paths.insert(Utf8PathBuf::from($in_l_path));
        )+)?
        // yields:
        traverse(&node, &mut fs, root)?;
        expected_paths.insert(Utf8Path::new("/"));
        expected_paths.insert(Utf8Path::new(root));
        $($(
            assert!(fs.is_directory(Utf8Path::new($out_d_path)), "Expected directory was not produced: {}", $out_d_path);
            $(
                let attrs = fs.attributes(Utf8Path::new($out_d_path))?;
                $(assert_eq!(attrs.owner.as_ref(), $out_d_owner);)?
                $(assert_eq!(attrs.group.as_ref(), $out_d_group);)?
                $(assert_eq!(attrs.mode, $out_d_mode.into());)?
            )?
            expected_paths.insert(Utf8Path::new($out_d_path));
        )+)?
        $($(
            assert!(fs.is_file($out_f_path), "Expected file at: {}", $out_f_path);
            $(
                let attrs = fs.attributes(Utf8Path::new($out_f_path))?;
                $(assert_eq!(attrs.owner.as_ref(), $out_f_owner);)?
                $(assert_eq!(attrs.group.as_ref(), $out_f_group);)?
                $(assert_eq!(attrs.mode, $out_f_mode.into());)?
            )?
            assert_eq!(&fs.read_file(Utf8Path::new($out_f_path))?, $content);
            expected_paths.insert(Utf8Path::new($out_f_path));
        )+)?
        $($(
            assert!(fs.is_link(Utf8Path::new($link)), "Expected symlink at: {}", $link);
            assert_eq!(&fs.read_link(Utf8Path::new($link))?, $target, "Expected symlink: {} -> {}", $link, $target);
            expected_paths.insert(Utf8Path::new($link));
        )+)?
        let actual_paths = fs.to_path_set();
        let unaccounted: Vec<_> = actual_paths.difference(&expected_paths).collect();
        if !unaccounted.is_empty() {
            panic!("Paths unaccounted for: {:?}", unaccounted);
        }
        Ok(())
    }};
}

mod attributes;
mod comments;
mod creation;
mod matching;
mod reuse;
mod variables;
