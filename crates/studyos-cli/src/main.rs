mod app;
mod runtime;
mod tui;

use std::env;
use std::fs;

use crate::runtime::AppServerClient;
use anyhow::{Result, anyhow};
use app::{App, AppBootstrap};
use studyos_core::{
    AppConfig, AppDatabase, AppPaths, AppSnapshot, BootstrapStudyContext, CourseCatalog,
    DeadlineEntry, LocalContext, StartupMisconceptionItem, StartupReviewItem, load_deadlines,
    upsert_deadline,
};

fn main() -> Result<()> {
    let cwd = env::current_dir()?;
    let paths = AppPaths::discover(&cwd);
    let args = env::args().collect::<Vec<_>>();
    let command = args.get(1).map(String::as_str);

    match command {
        Some("init") => run_init(&paths),
        Some("doctor") => run_doctor(&paths),
        Some("deadlines") => run_deadlines(&paths, &args[2..]),
        Some("courses") => run_courses(&paths, &args[2..]),
        Some("materials") => run_materials(&paths, &args[2..]),
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
    let startup_context = BootstrapStudyContext {
        due_reviews: database
            .list_due_reviews(4)?
            .into_iter()
            .map(|item| StartupReviewItem {
                concept_name: item.concept_name,
            })
            .collect(),
        recent_misconceptions: database
            .list_recent_misconceptions(4)?
            .into_iter()
            .map(|item| StartupMisconceptionItem {
                concept_name: item.concept_name,
                error_type: item.error_type,
                description: item.description,
            })
            .collect(),
        last_session_recap: database.latest_session_recap()?,
    };
    let snapshot = AppSnapshot::bootstrap(&config, &stats, &startup_context);
    let (runtime, runtime_error) = match AppServerClient::spawn() {
        Ok(runtime) => (Some(runtime), None),
        Err(error) => (None, Some(error.to_string())),
    };

    let mut app = App::new(AppBootstrap {
        database,
        paths: paths.clone(),
        config,
        stats,
        local_context,
        snapshot,
        runtime,
        runtime_error,
        resume_state,
    });
    if let Err(error) = app.bootstrap_runtime() {
        eprintln!("StudyOS runtime bootstrap warning: {error}");
    }
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
    let mut stats = database.stats()?;
    stats.upcoming_deadlines = local_context.upcoming_deadline_count();
    let resume = database.load_resume_state()?;
    let startup_context = BootstrapStudyContext {
        due_reviews: database
            .list_due_reviews(4)?
            .into_iter()
            .map(|item| StartupReviewItem {
                concept_name: item.concept_name,
            })
            .collect(),
        recent_misconceptions: database
            .list_recent_misconceptions(4)?
            .into_iter()
            .map(|item| StartupMisconceptionItem {
                concept_name: item.concept_name,
                error_type: item.error_type,
                description: item.description,
            })
            .collect(),
        last_session_recap: database.latest_session_recap()?,
    };
    let snapshot = AppSnapshot::bootstrap(&config, &stats, &startup_context);
    let app_server = match AppServerClient::spawn() {
        Ok(runtime) => {
            let initialized = runtime.initialize().is_ok();
            format!("available (initialize={initialized})")
        }
        Err(error) => format!("unavailable ({error})"),
    };

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
    println!("suggested_mode: {}", snapshot.mode.label());
    println!("mode_why_now: {}", snapshot.plan.why_now);
    println!("sessions_logged: {}", stats.total_sessions);
    println!("attempts_logged: {}", stats.total_attempts);
    println!("due_reviews: {}", stats.due_reviews);
    println!("loaded_deadlines: {}", local_context.deadlines.len());
    println!("loaded_materials: {}", local_context.materials.len());
    println!("loaded_courses: {}", local_context.courses.courses.len());
    println!("resume_state_present: {}", resume.is_some());
    println!("app_server: {}", app_server);

    Ok(())
}

fn run_deadlines(paths: &AppPaths, args: &[String]) -> Result<()> {
    paths.ensure()?;

    match args.first().map(String::as_str) {
        Some("list") | None => run_deadlines_list(paths, &args[1.min(args.len())..]),
        Some("add") => run_deadlines_add(paths, &args[1..]),
        Some(other) => Err(anyhow!(
            "unknown deadlines subcommand: {other}. Use `deadlines list` or `deadlines add`."
        )),
    }
}

fn run_courses(paths: &AppPaths, args: &[String]) -> Result<()> {
    paths.ensure()?;

    match args.first().map(String::as_str) {
        Some("list") | None => run_courses_list(paths),
        Some("use") => run_courses_use(paths, &args[1..]),
        Some(other) => Err(anyhow!(
            "unknown courses subcommand: {other}. Use `courses list` or `courses use`."
        )),
    }
}

fn run_courses_list(paths: &AppPaths) -> Result<()> {
    let config = AppConfig::load_or_default(&paths.config_path)?;
    let catalog = CourseCatalog::load(&paths.courses_dir)?;

    if catalog.courses.is_empty() {
        println!("No course files found in {}", paths.courses_dir.display());
        return Ok(());
    }

    println!("StudyOS courses");
    for course in catalog.courses {
        let marker = if course.title == config.default_course {
            "*"
        } else {
            "-"
        };
        println!(
            "{marker} {} | topics {} | concepts {}",
            course.title,
            course.topics.len(),
            course.concepts.len()
        );
    }

    Ok(())
}

fn run_courses_use(paths: &AppPaths, args: &[String]) -> Result<()> {
    let selected = required_option(args, "--title")?;
    let catalog = CourseCatalog::load(&paths.courses_dir)?;
    let known = catalog
        .courses
        .iter()
        .any(|course| course.title.to_lowercase() == selected.to_lowercase());

    if !known {
        return Err(anyhow!(
            "course not found: {selected}. Run `courses list` to inspect available titles."
        ));
    }

    let mut config = AppConfig::load_or_default(&paths.config_path)?;
    config.default_course = selected.clone();
    config.save(&paths.config_path)?;

    println!(
        "Set default course to {} in {}.",
        selected,
        paths.config_path.display()
    );
    Ok(())
}

fn run_materials(paths: &AppPaths, args: &[String]) -> Result<()> {
    paths.ensure()?;

    match args.first().map(String::as_str) {
        Some("list") | None => run_materials_list(paths, &args[1.min(args.len())..]),
        Some("search") => run_materials_search(paths, &args[1..]),
        Some(other) => Err(anyhow!(
            "unknown materials subcommand: {other}. Use `materials list` or `materials search`."
        )),
    }
}

fn run_materials_list(paths: &AppPaths, args: &[String]) -> Result<()> {
    let local_context = LocalContext::load(paths)?;
    let course_filter = option_value(args, "--course");
    let materials = local_context.search_materials(
        course_filter.as_deref(),
        &[],
        local_context.materials.len().max(1),
    );

    if materials.is_empty() {
        println!(
            "No local materials matched in {}.",
            paths.materials_dir.join("manifest.json").display()
        );
        return Ok(());
    }

    print_materials(materials);
    Ok(())
}

fn run_materials_search(paths: &AppPaths, args: &[String]) -> Result<()> {
    let query = required_option(args, "--query")?;
    let course_filter = option_value(args, "--course");
    let local_context = LocalContext::load(paths)?;
    let terms = query
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let materials = local_context.search_materials(course_filter.as_deref(), &terms, 8);

    if materials.is_empty() {
        println!("No materials matched query `{query}`.");
        return Ok(());
    }

    print_materials(materials);
    Ok(())
}

fn run_deadlines_list(paths: &AppPaths, args: &[String]) -> Result<()> {
    let course_filter = option_value(args, "--course");
    let mut deadlines = load_deadlines(&paths.deadlines_path)?;
    if let Some(course) = course_filter {
        let course = course.to_lowercase();
        deadlines.retain(|deadline| deadline.course.to_lowercase() == course);
    }

    if deadlines.is_empty() {
        println!(
            "No local deadlines recorded at {}",
            paths.deadlines_path.display()
        );
        return Ok(());
    }

    println!("StudyOS deadlines");
    for deadline in deadlines {
        println!(
            "- {} | {} | {} | weight {:.2} | id {}",
            deadline.due_at, deadline.course, deadline.title, deadline.weight, deadline.id
        );
        if !deadline.notes.trim().is_empty() {
            println!("  notes: {}", deadline.notes);
        }
    }

    Ok(())
}

fn run_deadlines_add(paths: &AppPaths, args: &[String]) -> Result<()> {
    let title = required_option(args, "--title")?;
    let due_at = required_option(args, "--due-at")?;
    let course = required_option(args, "--course")?;
    let weight = option_value(args, "--weight")
        .as_deref()
        .unwrap_or("1.0")
        .parse::<f32>()
        .map_err(|error| anyhow!("invalid --weight value: {error}"))?;
    let notes = option_value(args, "--notes").unwrap_or_default();
    let source = option_value(args, "--source").unwrap_or_else(|| "manual".to_string());
    let id = option_value(args, "--id").unwrap_or_else(|| deadline_id(&title, &due_at));

    let deadlines = upsert_deadline(
        &paths.deadlines_path,
        DeadlineEntry {
            id: id.clone(),
            source,
            title,
            due_at,
            course,
            weight,
            notes,
        },
    )?;

    println!(
        "Saved deadline {} to {} ({} total).",
        id,
        paths.deadlines_path.display(),
        deadlines.len()
    );
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

fn print_materials(materials: Vec<studyos_core::MaterialEntry>) {
    println!("StudyOS materials");
    for entry in materials {
        println!(
            "- {} | {} | {} | {}",
            entry.title, entry.course, entry.material_type, entry.path
        );
        println!("  tags: {}", entry.topic_tags.join(", "));
        println!("  snippet: {}", entry.snippet);
    }
}

fn option_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}

fn required_option(args: &[String], flag: &str) -> Result<String> {
    option_value(args, flag).ok_or_else(|| anyhow!("missing required flag {flag}"))
}

fn deadline_id(title: &str, due_at: &str) -> String {
    format!(
        "{}-{}",
        slug(title),
        slug(due_at.split('T').next().unwrap_or(due_at))
    )
}

fn slug(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
