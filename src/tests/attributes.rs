use anyhow::Result;

#[test]
#[should_panic]
fn test_incorrect_attribute_assertion() {
    (|| -> Result<()> {
        assert_effect_of! {
            applying: "
                dir/
                    :mode 640
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
                :mode 640
            another/
                :owner daemon
                :group sys
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
            :mode 640
            :owner daemon
            :group sys
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
            :let x = dae
            :let y = s
            attrs/
                :owner ${x}mon
                :group ${y}y${y}
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
    assert_effect_of! {
        applying: "
            dir/
                :mode 750
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