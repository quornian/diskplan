use anyhow::Result;
use diskplan_filesystem::DEFAULT_DIRECTORY_MODE;

#[test]
#[should_panic]
fn incorrect_attribute_assertion() {
    (|| -> Result<()> {
        assert_effect_of! {
            under: "/target"
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
fn attributes() -> Result<()> {
    assert_effect_of! {
        under: "/target"
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
fn top_level_attributes() -> Result<()> {
    assert_effect_of! {
        under: "/target"
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
                    mode = DEFAULT_DIRECTORY_MODE]
    }
}

#[test]
fn attribute_expressions() -> Result<()> {
    assert_effect_of! {
        under: "/target"
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
fn changing_attributes() -> Result<()> {
    assert_effect_of! {
        under: "/target"
        applying: "
            dir/
                :mode 750
            "
        onto: "/target"
        with:
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
fn inherited_attributes() -> Result<()> {
    assert_effect_of! {
        under: "/target"
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
                    owner = "daemon"
                    group = "sys"
                    mode = DEFAULT_DIRECTORY_MODE]
    }
}
