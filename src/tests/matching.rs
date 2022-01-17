use anyhow::Result;

#[test]
fn test_binding_static_beats_dynamic() -> Result<()> {
    assert_effect_of! {
        applying: "
            fixed/
                MATCHED_FIXED/
            $variable/
                #match .*
                MATCHED_VARIABLE/
            "
        onto: "/"
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
                #match .*
                MATCHED_VARIABLE/
            fixed/
                MATCHED_FIXED/
            "
        onto: "/"
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
                #match .*
                MATCHED_VARIABLE_A/
            $variable_b/
                #match .*
                MATCHED_VARIABLE_B/
            "
            onto: "/"
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
            #let var = xxx
            $var/
                #match .*
                variable/
            "
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
            #let var = xxx
            $var/
                #match .*
                variable/
            xxx/
                static/
            "
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
                #match x.*
                starts
                    #source /src/empty
            $b/
                #match .*x
                ends
                    #source /src/empty
            "
        onto: "/target"
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
                #match .*
                $var/
                sub/
                    $var/
            "
        onto: "/target"
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
fn test_match_categories() -> Result<()> {
    assert_effect_of! {
        applying: "
            $building/
                #match .*shed
                BUILDING/
            $animal/
                #match .*
                #avoid .*shed
                ANIMAL/
            "
        onto: "/target"
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
