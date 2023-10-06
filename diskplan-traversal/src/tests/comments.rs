use crate::{filesystem::DEFAULT_DIRECTORY_MODE, schema::parse_schema};
use anyhow::Result;

#[test]
fn empty_text_yields_empty_schema() -> Result<()> {
    assert_effect_of! {
        under: "/target" applying: "" onto: "/target"
        yields: directories: "/target" [mode = DEFAULT_DIRECTORY_MODE]
    }
}

#[test]
fn various_whitespace_parses_ok() -> Result<()> {
    for text in ["", "\n", " ", "    ", " \n", "\n  ", "\n  \n"] {
        assert_eq!(
            parse_schema(text)?
                .schema
                .as_directory()
                .unwrap()
                .entries()
                .len(),
            0
        );
    }
    Ok(())
}

#[test]
fn various_comments_parse_ok() -> Result<()> {
    for text in [
        "# Comment",
        "# Comment\n",
        " # Comment",
        "    # Comment",
        " # Comment\n",
        "\n  # Comment",
        "\n  # Comment  \n",
    ] {
        assert_eq!(
            parse_schema(text)?
                .schema
                .as_directory()
                .unwrap()
                .entries()
                .len(),
            0
        );
    }
    Ok(())
}

#[test]
fn commented_out_has_no_effect() -> Result<()> {
    assert_effect_of! {
        under: "/target"
        applying: "
            dir_a/
                :mode 123
            dir_b/
                # :mode 123
            "
        onto: "/target"
        yields:
            directories:
                "/target/dir_a" [mode = 0o123]
                "/target/dir_b" [mode = DEFAULT_DIRECTORY_MODE]
    }
}
