use anyhow::Result;

#[test]
fn match_binds_for_reuse() -> Result<()> {
    assert_effect_of! {
        applying: "
            $var/
                sub/
                    $var/
            "
        under: "/root"
        onto: "/root"
        with:
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
        under: "/root"
        onto: "/root"
        with:
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
        under: "/root"
        onto: "/root"
        with:
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
        under: "/root"
        onto: "/root"
        with:
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
        under: "/root"
        onto: "/root"
        with:
            directories:
                "/root"
        yields:
            directories:
                "/root/first"
                "/root/first/sub"
                "/root/first/sub/second"
    }
}

#[test]
fn name_from_use_target_not_definition() -> Result<()> {
    assert_effect_of!(
        applying: "
            :def defname/
                :let complex = pre_${NAME}_post
                $complex/
            usename/
                :use defname
            "
        under: "/"
        onto: "/"
        yields:
            directories:
                "/usename"
                "/usename/pre_usename_post"
    )
}

#[test]
fn variable_not_matching_deeper() -> Result<()> {
    // TODO: Consider if this should be an error, or warning at least
    assert_effect_of!(
        applying: "
            :let variable = aaa
            
            $variable/
                :match b+
                VARIABLE/
            "
        under: "/"
        onto: "/"
        yields:
            // Doesn't create /aaa/VARIABLE
    )
}

#[test]
fn variable_will_not_match_other() -> Result<()> {
    assert_effect_of!(
        applying: "
            :let variable = aaa
            
            $variable/
                :match b+
                VARIABLE/
            $other/
                :match a+
                OTHER/
            "
        under: "/"
        onto: "/"
        yields:
            // Doesn't create /aaa/OTHER
    )
}
