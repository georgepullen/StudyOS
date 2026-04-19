use std::process::Command;

#[test]
fn tour_command_mentions_materials_and_runtime_trace() {
    let binary = env!("CARGO_BIN_EXE_studyos-cli");
    let output = Command::new(binary)
        .arg("tour")
        .output()
        .unwrap_or_else(|err| panic!("failed to run studyos-cli tour: {err}"));

    assert!(
        output.status.success(),
        "tour command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("materials ingest"));
    assert!(stdout.contains("--log-json"));
    assert!(stdout.contains("Ctrl+R"));
}
