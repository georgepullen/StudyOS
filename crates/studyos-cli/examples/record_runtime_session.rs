use std::{env, fs};

use anyhow::Result;
use studyos_cli::{App, AppBootstrap, capture_runtime_fixture, tutor_output_schema};
use studyos_core::{
    AppConfig, AppDatabase, AppPaths, AppSnapshot, BootstrapStudyContext, LocalContext,
};

fn main() -> Result<()> {
    let repo_root = env::current_dir()?;
    let temp_root = env::temp_dir().join(format!(
        "studyos-runtime-record-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&temp_root)?;

    let paths = AppPaths::discover(&temp_root);
    paths.ensure()?;
    let database = AppDatabase::open(&paths.database_path)?;
    let config = AppConfig::default();
    let stats = database.stats()?;
    let snapshot = AppSnapshot::bootstrap(&config, &stats, &BootstrapStudyContext::default());
    let app = App::new(AppBootstrap {
        database,
        paths: paths.clone(),
        config,
        stats,
        local_context: LocalContext::default(),
        snapshot,
        runtime: None,
        runtime_factory: None,
        runtime_error: None,
        resume_state: None,
    });

    let fixture_path =
        repo_root.join("crates/studyos-cli/tests/fixtures/runtime/opening-turn.jsonl");
    let stderr_path =
        repo_root.join("crates/studyos-cli/tests/fixtures/runtime/opening-turn.stderr.log");
    let cwd = repo_root.as_path();

    capture_runtime_fixture(
        cwd,
        &app.developer_instructions(),
        &app.build_opening_prompt(),
        tutor_output_schema(),
        &fixture_path,
        &stderr_path,
    )?;

    println!("Recorded runtime fixture at {}", fixture_path.display());
    Ok(())
}
