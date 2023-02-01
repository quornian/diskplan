#[test]
fn check_readme_contains_quickstart_toml() -> anyhow::Result<()> {
    let readme = std::fs::read_to_string("README.md")?;
    let mut quickstart_toml = std::fs::read_to_string("examples/quickstart/diskplan.toml")?;
    quickstart_toml.insert_str(0, "```toml\n");
    quickstart_toml.push_str("```\n");
    assert!(
        readme.contains(&quickstart_toml),
        "README.md does not contain:\n{quickstart_toml}"
    );
    Ok(())
}

#[test]
fn check_readme_contains_quickstart_schema() -> anyhow::Result<()> {
    let readme = std::fs::read_to_string("README.md")?;
    let mut quickstart_schema =
        std::fs::read_to_string("examples/quickstart/simple-schema.diskplan")?;
    quickstart_schema.insert_str(0, "```sh\n");
    quickstart_schema.push_str("```\n");
    assert!(
        readme.contains(&quickstart_schema),
        "README.md does not contain:\n{quickstart_schema}"
    );
    Ok(())
}
