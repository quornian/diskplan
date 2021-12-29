use indoc::indoc;

use crate::schema::parse_schema;

use super::traverse;

#[test]
fn test_traverse() {
    let schema_root = parse_schema(indoc!(
        "
        subdir/
        $any/
            deeper
                #source res/$any
        ",
    ))
    .unwrap();
    traverse(&schema_root).unwrap();
}
