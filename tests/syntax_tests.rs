use diskplan::schema::text::parse_schema;
use indoc::indoc;

#[test]
fn test_top_level_mode() {
    parse_schema(indoc! {"
        #mode 0o755
    "})
    .unwrap();
}
