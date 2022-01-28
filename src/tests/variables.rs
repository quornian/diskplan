use anyhow::Result;

#[test]
fn match_binds_for_reuse() -> Result<()> {
    assert_effect_of! {
        applying: "
            $var/
                sub/
                    $var/
            "
        onto: "/root"
            directories:
                "/root"
                "/root/existing"
        yields:
            directories:
                "/root/existing/sub"
                "/root/existing/sub/existing"
    }
}

#[test]
fn let_binds_for_reuse() -> Result<()> {
    assert_effect_of! {
        applying: "
            :let var = explicit
            $var/
                sub/
                    $var/
            "
        onto: "/root"
            directories:
                "/root"
        yields:
            directories:
                "/root/explicit"
                "/root/explicit/sub"
                "/root/explicit/sub/explicit"
    }
}

#[test]
fn match_still_happens_with_let() -> Result<()> {
    assert_effect_of! {
        applying: "
            :let var = explicit
            $var/
                sub/
                    $var/
            "
        onto: "/root"
            directories:
                "/root"
                "/root/existing"
        yields:
            directories:
                "/root/explicit"
                "/root/explicit/sub"
                "/root/explicit/sub/explicit"
                "/root/existing/sub"
                "/root/existing/sub/existing"
    }
}

#[test]
fn let_overrides_match() -> Result<()> {
    assert_effect_of! {
        applying: "
            $var/
                :let var = explicit
                sub/
                    $var/
            "
        onto: "/root"
            directories:
                "/root"
                "/root/existing"
        yields:
            directories:
                "/root/existing/sub"
                "/root/existing/sub/explicit"
    }
}

#[test]
fn let_overrides_let() -> Result<()> {
    assert_effect_of! {
        applying: "
            :let var = first
            $var/
                :let var = second
                sub/
                    $var/
            "
        onto: "/root"
            directories:
                "/root"
        yields:
            directories:
                "/root/first"
                "/root/first/sub"
                "/root/first/sub/second"
    }
}
