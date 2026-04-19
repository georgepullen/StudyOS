use studyos_cli::{App, AppBootstrap};
use studyos_core::{
    AppConfig, AppDatabase, AppPaths, AppSnapshot, AppStats, BootstrapStudyContext, CourseCatalog,
    LocalContext, MaterialEntry, StudyWindow, WindowSource,
};

fn app_for_window(window: StudyWindow) -> App {
    let root = std::env::temp_dir().join(format!(
        "studyos-window-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&root);
    let paths = AppPaths::discover(&root);
    paths
        .ensure()
        .unwrap_or_else(|err| panic!("path ensure failed: {err}"));
    let database = AppDatabase::open(&paths.database_path)
        .unwrap_or_else(|err| panic!("database open failed: {err}"));
    let config = AppConfig {
        default_session_minutes: 90,
        ..AppConfig::default()
    };
    let stats = AppStats {
        due_reviews: 0,
        upcoming_deadlines: 0,
        total_attempts: 0,
        total_sessions: 0,
    };
    let snapshot = AppSnapshot::bootstrap(
        &config,
        &stats,
        &BootstrapStudyContext {
            study_window: Some(window),
            ..BootstrapStudyContext::default()
        },
    );

    App::new(AppBootstrap {
        database,
        paths,
        config,
        stats,
        local_context: LocalContext {
            materials: vec![MaterialEntry {
                id: "worksheet".to_string(),
                title: "Matrix Multiplication Worksheet".to_string(),
                course: "Matrix Algebra & Linear Models".to_string(),
                topic_tags: vec!["matrix_multiplication".to_string()],
                material_type: "worksheet".to_string(),
                path: "materials/worksheet.pdf".to_string(),
                snippet: "Practice matrix multiplication under exam timing.".to_string(),
                source_hash: String::new(),
                source_modified_at: String::new(),
            }],
            courses: CourseCatalog::default(),
            ..LocalContext::default()
        },
        snapshot,
        runtime: None,
        runtime_factory: None,
        runtime_error: None,
        resume_state: None,
    })
}

#[test]
fn opening_prompt_uses_window() {
    let short_prompt = app_for_window(StudyWindow {
        start: "2026-04-19T14:45:00Z".to_string(),
        duration_minutes: 15,
        source: WindowSource::TimetableGap,
    })
    .build_opening_prompt();
    let long_prompt = app_for_window(StudyWindow {
        start: "2026-04-19T20:00:00Z".to_string(),
        duration_minutes: 90,
        source: WindowSource::EveningBlock,
    })
    .build_opening_prompt();

    assert!(short_prompt.contains("Study window: 15 minutes"));
    assert!(long_prompt.contains("Study window: 90 minutes"));
    assert_ne!(short_prompt, long_prompt);
}
