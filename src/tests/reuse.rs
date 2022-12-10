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

#[test]
fn test_use_owner_inherited() -> Result<()> {
    // Note: these rely on the user and group existing on the system. If user "sync" or group
    // "games" do not exist, change appropriately
    assert_effect_of! {
        applying: "
            :def definition/
                :owner sync
                :group games

            usage/
                :use definition
                child/
            "
        under: "/"
        onto: "/"
        yields:
            directories:
                "/usage" [owner = "sync" group = "games"]
                "/usage/child" [owner = "sync" group = "games"]
    }
}

#[test]
fn owner_inheritance() -> Result<()> {
    assert_effect_of! {
        applying: "
            :def o_root/
                :owner root
            :def o_sys/
                :owner sys

            use_order_root_owned/
                :use o_root
                :use o_sys

            use_order_sys_owned/
                :use o_sys
                :use o_root

            local_wins_root_owned/
                :owner root
                :use o_sys

            local_wins_sys_owned/
                :use o_root
                :owner sys
            "
        under: "/"
        onto: "/"
        yields:
            directories:
                "/use_order_root_owned" [owner = "root"]
                "/use_order_sys_owned" [owner = "sys"]
                "/local_wins_root_owned" [owner = "root"]
                "/local_wins_sys_owned" [owner = "sys"]
    }
}
