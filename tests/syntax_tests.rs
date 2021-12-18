use diskplan::schema::parse_schema;
use indoc::indoc;

#[test]
fn test_top_level_mode() {
    parse_schema(indoc! {"
        #mode 755
    "})
    .unwrap();
}
