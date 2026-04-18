mod app;
mod tui;

use std::env;
use std::fs;

use anyhow::Result;
use app::App;
use studyos_core::{AppConfig, AppDatabase, AppPaths, AppSnapshot, LocalContext};

fn main() -> Result<()> {
    let cwd = env::current_dir()?;
    let paths = AppPaths::discover(&cwd);
    let command = env::args().nth(1);

    match command.as_deref() {
        Some("init") => run_init(&paths),
        Some("doctor") => run_doctor(&paths),
        _ => run_interactive(&paths),
    }
}

fn run_interactive(paths: &AppPaths) -> Result<()> {
    paths.ensure()?;

    let config = AppConfig::load_or_default(&paths.config_path)?;
    let database = AppDatabase::open(&paths.database_path)?;
    let local_context = LocalContext::load(paths)?;
    let mut stats = database.stats()?;
    stats.upcoming_deadlines = local_context.upcoming_deadline_count();
    let resume_state = database.load_resume_state()?;
    let snapshot = AppSnapshot::bootstrap(&config, &stats);

    let app = App::new(
        paths.clone(),
        config,
        stats,
        snapshot,
        database,
        local_context,
        resume_state,
    );
    tui::run(app)
}

fn run_init(paths: &AppPaths) -> Result<()> {
    paths.ensure()?;

    write_if_missing(
        &paths.config_path,
        include_str!("../../../examples/studyos-config.toml"),
    )?;
    write_if_missing(
        &paths.deadlines_path,
        include_str!("../../../examples/deadlines.json"),
    )?;
    write_if_missing(
        &paths.timetable_path,
        include_str!("../../../examples/timetable.json"),
    )?;
    write_if_missing(
        &paths.materials_dir.join("manifest.json"),
        include_str!("../../../examples/materials-manifest.json"),
    )?;
    write_if_missing(
        &paths.courses_dir.join("linear-models.toml"),
        include_str!("../../../examples/linear-models.toml"),
    )?;
    write_if_missing(
        &paths.courses_dir.join("probability-stats.toml"),
        include_str!("../../../examples/probability-stats.toml"),
    )?;

    println!(
        "StudyOS local data initialized at {}",
        paths.root_dir.display()
    );
    Ok(())
}

fn run_doctor(paths: &AppPaths) -> Result<()> {
    paths.ensure()?;

    let config = AppConfig::load_or_default(&paths.config_path)?;
    let database = AppDatabase::open(&paths.database_path)?;
    let local_context = LocalContext::load(paths)?;
    let stats = database.stats()?;
    let resume = database.load_resume_state()?;

    println!("StudyOS doctor");
    println!("data_dir: {}", paths.root_dir.display());
    println!(
        "config_path: {} ({})",
        paths.config_path.display(),
        exists_flag(&paths.config_path)
    );
    println!(
        "database_path: {} ({})",
        paths.database_path.display(),
        exists_flag(&paths.database_path)
    );
    println!(
        "deadlines_path: {} ({})",
        paths.deadlines_path.display(),
        exists_flag(&paths.deadlines_path)
    );
    println!(
        "timetable_path: {} ({})",
        paths.timetable_path.display(),
        exists_flag(&paths.timetable_path)
    );
    println!("default_course: {}", config.default_course);
    println!("strictness: {:?}", config.strictness);
    println!("sessions_logged: {}", stats.total_sessions);
    println!("attempts_logged: {}", stats.total_attempts);
    println!("due_reviews: {}", stats.due_reviews);
    println!("loaded_deadlines: {}", local_context.deadlines.len());
    println!("loaded_materials: {}", local_context.materials.len());
    println!("loaded_courses: {}", local_context.courses.courses.len());
    println!("resume_state_present: {}", resume.is_some());

    Ok(())
}

fn write_if_missing(path: &std::path::Path, contents: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, contents)?;
    Ok(())
}

fn exists_flag(path: &std::path::Path) -> &'static str {
    if path.exists() { "present" } else { "missing" }
}
