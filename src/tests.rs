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
        let root = $root;
        // containing:
        let mut expected_paths: HashSet<String> = HashSet::new();
        $($(
            #[allow(unused_mut)]
            let mut attrs = SetAttrs::default();
            $(
                $(attrs.owner = Some($in_d_owner);)?
                $(attrs.group = Some($in_d_group);)?
                $(attrs.mode = Some($in_d_mode.into());)?
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
                $(attrs.mode = Some($in_f_mode.into());)?
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
                $(assert_eq!(attrs.mode, $out_d_mode.into());)?
            )?
            expected_paths.insert($out_d_path.to_owned());
        )+)?
        $($(
            assert!(fs.is_file($out_f_path));
            $(
                let attrs = fs.attributes($out_f_path)?;
                $(assert_eq!(attrs.owner.as_ref(), $out_f_owner);)?
                $(assert_eq!(attrs.group.as_ref(), $out_f_group);)?
                $(assert_eq!(attrs.mode, $out_f_mode.into());)?
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

mod attributes;
mod creation;
mod matching;
mod reuse;
