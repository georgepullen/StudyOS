use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use studyos_cli::{App, AppBootstrap, AppServerTransport};
use studyos_core::{
    AppConfig, AppDatabase, AppPaths, AppSnapshot, BootstrapStudyContext, ContentBlock,
    LocalContext,
};

type RuntimeHandle = Arc<dyn AppServerTransport>;
type RuntimeFactory = Arc<dyn Fn() -> Result<RuntimeHandle> + Send + Sync>;

pub fn temp_data_root(label: &str) -> PathBuf {
    let path = env::temp_dir().join(format!(
        "studyos-integration-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap_or_else(|err| panic!("temp dir create failed: {err}"));
    path
}

pub fn build_app(
    base: &Path,
    runtime: Option<RuntimeHandle>,
    runtime_factory: Option<RuntimeFactory>,
) -> App {
    let paths = AppPaths::discover(base);
    paths
        .ensure()
        .unwrap_or_else(|err| panic!("path ensure failed: {err}"));
    let database = AppDatabase::open(&paths.database_path)
        .unwrap_or_else(|err| panic!("database open failed: {err}"));
    let config = AppConfig::default();
    let stats = database
        .stats()
        .unwrap_or_else(|err| panic!("stats query failed: {err}"));
    let snapshot = AppSnapshot::bootstrap(&config, &stats, &BootstrapStudyContext::default());

    App::new(AppBootstrap {
        database,
        paths,
        config,
        stats,
        local_context: LocalContext::default(),
        snapshot,
        runtime,
        runtime_factory,
        runtime_error: None,
        resume_state: None,
    })
}

pub fn app_has_question(app: &App) -> bool {
    app.snapshot
        .transcript
        .iter()
        .any(|block| matches!(block, ContentBlock::QuestionCard(_)))
}

pub fn app_has_parse_warning(app: &App) -> bool {
    app.snapshot.transcript.iter().any(|block| {
        matches!(
            block,
            ContentBlock::WarningBox(warning)
                if warning.title.contains("parse failed")
        )
    })
}
