use anyhow::Result;

#[test]
fn test_create_directory() -> Result<()> {
    assert_effect_of! {
        applying: "
            subdir/
                subsubdir/
            "
        under: "/primary"
        onto: "/primary"
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
                    :source /resource/file1
            subfile
                :source /resource/file2
            "
        under: "/primary"
        onto: "/primary"
        with:
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
                    :source /resource/file
            "
        under: "/primary"

        applying: "
            $_a/
                $_b/
                    $_c/
            "
        under: "/secondary"

        onto: "/primary"
        with:
            directories:
                "/resource"
                "/secondary" //TODO: Can we avoid this?
            files:
                "/resource/file" ["FILE CONTENT"]
        yields:
            directories:
                "/primary"
                "/secondary"
                "/secondary/subdirlink"
            files:
                "/secondary/subdirlink/subfile" ["FILE CONTENT"]
            symlinks:
                "/primary/subdirlink" -> "/secondary/subdirlink"
    }
}

#[test]
fn test_create_relative_symlink() -> Result<()> {
    assert_effect_of! {
        applying: "
            versions/
                1.0/
                1.0.1/ -> 1.0
            "
        under: "/"
        onto: "/"
        yields:
            directories:
                "/versions"
                "/versions/1.0"
            symlinks:
                "/versions/1.0.1" -> "1.0"
    }
}

#[test]
fn symlink_two_schemas() -> Result<()> {
    assert_effect_of! {
        applying: "
            $name/ -> /remote/$PATH
                # Symlink target is created first then modified by this
                :group adm
            "
        under: "/local"

        applying: "
            $_1/
                # This applies first, but is overridden by schema above
                :group sys
            "
        under: "/remote"

        onto: "/local/example"
        yields:
            directories:
                "/local"
                "/remote/example" [ group = "adm" ]
            symlinks:
                "/local/example" -> "/remote/example"
    }
}
