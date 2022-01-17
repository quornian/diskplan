use anyhow::Result;

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
