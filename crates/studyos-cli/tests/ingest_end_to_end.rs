use std::{fs, path::PathBuf, process::Command};

use studyos_core::MaterialManifest;

#[test]
fn ingest_end_to_end_builds_manifest_from_raw_dir() {
    let data_dir = std::env::temp_dir().join(format!(
        "studyos-ingest-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let _ = fs::remove_dir_all(&data_dir);
    fs::create_dir_all(data_dir.join("courses"))
        .unwrap_or_else(|err| panic!("failed to create courses dir: {err}"));
    fs::create_dir_all(data_dir.join("materials/raw"))
        .unwrap_or_else(|err| panic!("failed to create raw materials dir: {err}"));

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::copy(
        repo_root.join("examples/linear-models.toml"),
        data_dir.join("courses/linear-models.toml"),
    )
    .unwrap_or_else(|err| panic!("failed to copy course fixture: {err}"));
    fs::copy(
        repo_root.join("crates/studyos-core/tests/fixtures/materials/raw/linear-models.pdf"),
        data_dir.join("materials/raw/linear-models.pdf"),
    )
    .unwrap_or_else(|err| panic!("failed to copy pdf fixture: {err}"));

    let output = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("studyos-cli")
        .arg("--")
        .arg("materials")
        .arg("ingest")
        .env("STUDYOS_DATA_DIR", &data_dir)
        .current_dir(&repo_root)
        .output()
        .unwrap_or_else(|err| panic!("failed to run materials ingest: {err}"));
    assert!(
        output.status.success(),
        "materials ingest failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest_path = data_dir.join("materials/manifest.json");
    assert!(manifest_path.exists(), "manifest was not created");
    let manifest: MaterialManifest = serde_json::from_str(
        &fs::read_to_string(&manifest_path)
            .unwrap_or_else(|err| panic!("failed to read manifest: {err}")),
    )
    .unwrap_or_else(|err| panic!("failed to parse manifest: {err}"));

    assert_eq!(manifest.entries.len(), 1);
    assert!(!manifest.entries[0].snippet.trim().is_empty());
    assert_eq!(manifest.entries[0].course, "Matrix Algebra & Linear Models");

    let _ = fs::remove_dir_all(data_dir);
}
