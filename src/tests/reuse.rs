use anyhow::Result;

#[test]
fn test_def_use_simple() -> Result<()> {
    assert_effect_of! {
        applying: "
            :def some_def/
                sub/
            
            inner/
                :use some_def
            "
        under: "/"
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
            :use has_sub

            :def has_sub/
                sub/
            
            inner/
                :use has_sub
            "
        under: "/"
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
            :def def_a/
                sub_a/
            :def def_b/
                sub_b/
            
            inner/
                :use def_a
                :use def_b
                sub_c/
            "
        under: "/"
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
fn test_use_owner() -> Result<()> {
    // Note: these rely on the user and group existing on the system. If user "sync" or group
    // "games" do not exist, change appropriately
    assert_effect_of! {
        applying: "
            :def definition/
                :owner sync
                :group games
            
            usage/
                :use definition
            "
        under: "/"
        onto: "/"
        yields:
            directories:
                "/usage" [owner = "sync" group = "games"]
    }
}
