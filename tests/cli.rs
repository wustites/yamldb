use std::process::Command;

#[test]
fn default_equality_query_parses_numeric_values() {
    let dir = std::env::temp_dir().join(format!("yamldb-cli-query-{}", std::process::id()));
    let path = dir.join("data.yaml");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&path, "- id: user1\n  age: 30\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_yamldb"))
        .args([
            "--file",
            path.to_str().unwrap(),
            "query",
            "--key",
            "age",
            "--value",
            "30",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("user1:"), "{stdout}");

    let _ = std::fs::remove_dir_all(dir);
}
