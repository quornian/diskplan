use anyhow::Result;

#[test]
fn test_binding_static_beats_dynamic() -> Result<()> {
    assert_effect_of! {
        applying: "
            fixed/
                MATCHED_FIXED/
            $variable/
                :match .*
                MATCHED_VARIABLE/
            "
        under: "/"
        onto: "/"
        with:
            directories:
                "/fixed"
        yields:
            directories:
                "/fixed/MATCHED_FIXED"
    }
}

#[test]
fn test_binding_static_beats_dynamic_reordered() -> Result<()> {
    assert_effect_of! {
        applying: "
            $variable/
                :match .*
                MATCHED_VARIABLE/
            fixed/
                MATCHED_FIXED/
            "
        under: "/"
        onto: "/"
        with:
            directories:
                "/fixed"
        yields:
            directories:
                "/fixed/MATCHED_FIXED"
    }
}

#[test]
#[should_panic(
    expected = "'existing' matches multiple dynamic bindings '$variable_a' and '$variable_b'"
)]
fn test_binding_multiple_variable_error() {
    (|| -> Result<()> {
        assert_effect_of! {
            applying: "
                $variable_a/
                    :match .*
                    MATCHED_VARIABLE_A/
                $variable_b/
                    :match .*
                    MATCHED_VARIABLE_B/
                "
            under: "/"
            onto: "/"
            with:
                directories:
                    "/existing"
            yields:
                directories:
                    "/existing/MATCHED_VARIABLE_A"
        }
    })()
    .unwrap();
}

#[test]
#[should_panic(
    expected = "'duplicate' matches multiple static bindings 'duplicate' and 'duplicate'"
)]
fn test_binding_multiple_static_error() {
    (|| -> Result<()> {
        assert_effect_of! {
            applying: "
                duplicate/
                duplicate/
                "
            under: "/"
            onto: "/"
            yields:
        }
    })()
    .unwrap();
}

#[test]
fn test_match_let_variable() -> Result<()> {
    assert_effect_of! {
        applying: "
            :let var = xxx
            $var/
                :match .*
                variable/
            "
        under: "/target"
        onto: "/target"
        yields:
            directories:
                "/target/xxx"
                "/target/xxx/variable"
    }
}

#[test]
fn test_match_let_variable_overridden_by_static() -> Result<()> {
    // TODO: Consider if this should fail
    assert_effect_of! {
        applying: "
            :let var = xxx
            $var/
                :match .*
                variable/
            xxx/
                static/
            "
        under: "/target"
        onto: "/target"
        yields:
            directories:
                "/target/xxx"
                "/target/xxx/static"
    }
}

#[test]
fn test_match_variable() -> Result<()> {
    assert_effect_of! {
        applying: "
            $a/
                :match x.*
                starts
                    :source /src/empty
            $b/
                :match .*x
                ends
                    :source /src/empty
            "
        under: "/target"
        onto: "/target"
        with:
            directories:
                "/src"
                "/target"
                "/target/has_an_x_in_it"
                "/target/x_at_the_beginning"
                "/target/ends_with_an_x"
            files:
                "/src/empty" [""]
        yields:
            files:
                "/target/x_at_the_beginning/starts" [""]
                "/target/ends_with_an_x/ends" [""]
    }
}

#[test]
fn test_match_variable_inherited() -> Result<()> {
    assert_effect_of! {
        applying: "
            $var/
                :match .*
                $var/
                sub/
                    $var/
            "
        under: "/target"
        onto: "/target"
        with:
            directories:
                "/target"
                "/target/VALUE"
        yields:
            directories:
                "/target/VALUE/VALUE"
                "/target/VALUE/sub"
                "/target/VALUE/sub/VALUE"
    }
}

#[test]
fn test_match_let() -> Result<()> {
    assert_effect_of! {
        applying: "
            :let var = xxx
            $var/
                created/
            "
        under: "/target"
        onto: "/target"
        with:
            directories:
                "/target"
                "/target/yyy"
        yields:
            directories:
                "/target/xxx"
                "/target/xxx/created"
                "/target/yyy/created"
    }
}

#[test]
fn inherited_variable_can_rebind() -> Result<()> {
    assert_effect_of! {
        applying: "
            $var/
                $var/
                    inner/
            "
        under: "/"
        onto: "/"
        with:
            directories:
                "/a"
                "/a/x"
        yields:
            directories:
                "/a/a"
                "/a/a/inner"
                "/a/x"
                "/a/x/inner"
    }
}

#[test]
fn inherited_variable_with_match_avoids_rebind() -> Result<()> {
    assert_effect_of! {
        applying: "
            $var/
                $var/
                    :match $var
                    inner/
            "
        under: "/target"
        onto: "/target"
        with:
            directories:
                "/target"
                "/target/a"
                "/target/a/x"
        yields:
            directories:
                "/target/a/a"
                "/target/a/a/inner"
                // And not: /target/a/x/inner
    }
}

#[test]
fn test_match_categories() -> Result<()> {
    assert_effect_of! {
        applying: "
            $building/
                :match .*shed
                BUILDING/
            $animal/
                :match .*
                :avoid .*shed
                ANIMAL/
            "
        under: "/target"
        onto: "/target"
        with:
            directories:
                "/target"
                "/target/cow"
                "/target/shed"
                "/target/cow_shed"
                "/target/chicken"
        yields:
            directories:
                "/target/cow/ANIMAL"
                "/target/shed/BUILDING"
                "/target/cow_shed/BUILDING"
                "/target/chicken/ANIMAL"
    }
}
