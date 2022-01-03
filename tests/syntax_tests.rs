use diskplan::schema::parse_schema;

#[test]
fn test_top_level_mode() {
    parse_schema(
        "
        #mode 755
        ",
    )
    .unwrap();
}
