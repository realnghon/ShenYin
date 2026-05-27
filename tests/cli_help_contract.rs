use std::process::Command;

#[test]
fn help_prints_usage_text() {
    let output = Command::new(env!("CARGO_BIN_EXE_foldbox"))
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("foldbox pack <input-file> [output-file]"));
}
