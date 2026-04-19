use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use studyos_cli::{App, AppBootstrap, CodexAppServerTransport, tui};
use studyos_core::{
    AppConfig, AppDatabase, AppPaths, AppSnapshot, BootstrapStudyContext, CourseCatalog,
    DeadlineEntry, LocalContext, StartupMisconceptionItem, StartupReviewItem, TimetableSlot,
    append_timetable_slot, ingest_materials, load_deadlines, load_material_ingestion_status,
    upsert_deadline,
};

fn main() -> Result<()> {
    let cwd = env::current_dir()?;
    let paths = AppPaths::discover(&cwd);
    let cli = parse_cli_args(env::args().skip(1).collect())?;
    let command = cli.command.as_deref();

    match command {
        Some("init") => run_init(&paths),
        Some("doctor") => run_doctor(&paths),
        Some("tour") => run_tour(&paths),
        Some("deadlines") => run_deadlines(&paths, &cli.rest),
        Some("attempts") => run_attempts(&paths, &cli.rest),
        Some("courses") => run_courses(&paths, &cli.rest),
        Some("materials") => run_materials(&paths, &cli.rest),
        Some("timetable") => run_timetable(&paths, &cli.rest),
        _ => run_interactive(&paths, cli.log_json_path),
    }
}

struct CliArgs {
    command: Option<String>,
    rest: Vec<String>,
    log_json_path: Option<PathBuf>,
}

fn parse_cli_args(args: Vec<String>) -> Result<CliArgs> {
    let mut log_json_path = None;
    let mut filtered = Vec::new();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "--log-json" {
            let next = iter.next();
            let explicit_path = next
                .as_deref()
                .filter(|candidate| !candidate.starts_with('-'))
                .map(PathBuf::from);
            if let Some(path) = explicit_path {
                log_json_path = Some(path);
            } else {
                log_json_path = Some(PathBuf::new());
                if let Some(token) = next {
                    filtered.push(token);
                }
            }
            continue;
        }

        filtered.push(arg);
    }

    let command = filtered.first().cloned();
    let rest = if filtered.is_empty() {
        Vec::new()
    } else {
        filtered[1..].to_vec()
    };

    Ok(CliArgs {
        command,
        rest,
        log_json_path,
    })
}

fn run_interactive(paths: &AppPaths, log_json_path: Option<PathBuf>) -> Result<()> {
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
            .list_recent_repair_signals(4)?
            .into_iter()
            .map(|item| StartupMisconceptionItem {
                concept_name: item.concept_name,
                error_type: item.error_type,
                description: item.description,
            })
            .collect(),
        last_session_recap: database.latest_session_recap()?,
        study_window: local_context.best_study_window(),
    };
    let snapshot = AppSnapshot::bootstrap(&config, &stats, &startup_context);
    let resolved_log_path = resolve_log_json_path(paths, log_json_path);
    let runtime_factory = {
        let log_path = resolved_log_path.clone();
        Arc::new(move || CodexAppServerTransport::spawn_with_log_path(log_path.clone()))
    };
    let (runtime, runtime_error) = match runtime_factory() {
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
        runtime_factory: Some(runtime_factory),
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
        &paths.materials_manifest_path,
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
    let materials_status = load_material_ingestion_status(paths)?;
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
            .list_recent_repair_signals(4)?
            .into_iter()
            .map(|item| StartupMisconceptionItem {
                concept_name: item.concept_name,
                error_type: item.error_type,
                description: item.description,
            })
            .collect(),
        last_session_recap: database.latest_session_recap()?,
        study_window: local_context.best_study_window(),
    };
    let snapshot = AppSnapshot::bootstrap(&config, &stats, &startup_context);
    let app_server = match CodexAppServerTransport::spawn() {
        Ok(runtime) => {
            let initialized = runtime.initialize().is_ok();
            format!("available (initialize={initialized})")
        }
        Err(error) => format!("unavailable ({error})"),
    };

    println!("StudyOS doctor");
    println!("data_dir: {}", paths.root_dir.display());
    println!("logs_dir: {}", paths.logs_dir.display());
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
    println!(
        "study_window: {}",
        snapshot
            .plan
            .window
            .as_ref()
            .map(|window| format!(
                "{}m {:?} @ {}",
                window.duration_minutes, window.source, window.start
            ))
            .unwrap_or_else(|| "none".to_string())
    );
    println!("sessions_logged: {}", stats.total_sessions);
    println!("attempts_logged: {}", stats.total_attempts);
    println!("due_reviews: {}", stats.due_reviews);
    println!("loaded_deadlines: {}", local_context.deadlines.len());
    println!("loaded_materials: {}", local_context.materials.len());
    println!("loaded_courses: {}", local_context.courses.courses.len());
    println!("materials_ingested: {}", materials_status.files_indexed);
    println!(
        "materials_last_run: {}",
        materials_status.last_run_at.as_deref().unwrap_or("never")
    );
    println!(
        "materials_raw_git_safe: {}",
        if raw_materials_path_looks_safe(paths) {
            "yes"
        } else {
            "warning"
        }
    );
    println!("resume_state_present: {}", resume.is_some());
    println!("app_server: {app_server}");

    Ok(())
}

fn run_tour(paths: &AppPaths) -> Result<()> {
    paths.ensure()?;
    println!("StudyOS tour");
    println!("1. Initialize local data once:");
    println!("   cargo run -p studyos-cli -- init");
    println!("2. Check local health and runtime wiring:");
    println!("   cargo run -p studyos-cli -- doctor");
    println!("3. Drop your course files into:");
    println!("   {}", paths.materials_raw_dir.display());
    println!("4. Build the distilled materials index:");
    println!("   cargo run -p studyos-cli -- materials ingest");
    println!("5. Add the next exam or coursework deadline:");
    println!(
        "   cargo run -p studyos-cli -- deadlines add --title \"Linear models exam\" --due 2026-05-12T09:00:00+01:00 --course \"Matrix Algebra & Linear Models\""
    );
    println!("6. Add upcoming timetable slots:");
    println!(
        "   cargo run -p studyos-cli -- timetable add --day Mon --start 15:00 --end 17:00 --course \"Matrix Algebra & Linear Models\" --title \"Lecture\""
    );
    println!("7. Start a study session:");
    println!("   cargo run -p studyos-cli");
    println!("8. During a session use:");
    println!("   F5 submit answer, q review-and-quit, Ctrl+R reconnect runtime, ? help");
    println!("9. Optional runtime trace:");
    println!("   cargo run -p studyos-cli -- --log-json");
    Ok(())
}

fn resolve_log_json_path(paths: &AppPaths, requested: Option<PathBuf>) -> Option<PathBuf> {
    match requested {
        None => None,
        Some(path) if path.as_os_str().is_empty() => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            Some(paths.logs_dir.join(format!("runtime-{timestamp}.jsonl")))
        }
        Some(path) if path.is_absolute() => Some(path),
        Some(path) => Some(paths.root_dir.join(path)),
    }
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

fn run_attempts(paths: &AppPaths, args: &[String]) -> Result<()> {
    paths.ensure()?;

    match args.first().map(String::as_str) {
        Some("list") | None => run_attempts_list(paths, &args[1.min(args.len())..]),
        Some(other) => Err(anyhow!(
            "unknown attempts subcommand: {other}. Use `attempts list --session <id>`."
        )),
    }
}

fn run_attempts_list(paths: &AppPaths, args: &[String]) -> Result<()> {
    let session_id = required_option(args, "--session")?;
    let database = AppDatabase::open(&paths.database_path)?;
    let attempts = database.list_attempts_for_session(&session_id)?;

    if attempts.is_empty() {
        println!("No attempts found for session {session_id}");
        return Ok(());
    }

    println!("StudyOS attempts for {session_id}");
    for attempt in attempts {
        println!(
            "- {} | concept {} | {} / {} | {} ms",
            attempt.id,
            attempt.concept_id,
            attempt.correctness,
            attempt.reasoning_quality,
            attempt.latency_ms
        );
        println!("  type: {}", attempt.question_type);
        println!("  feedback: {}", attempt.feedback_summary);
    }

    Ok(())
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
        Some("ingest") => run_materials_ingest(paths),
        Some("list") | None => run_materials_list(paths, &args[1.min(args.len())..]),
        Some("search") => run_materials_search(paths, &args[1..]),
        Some(other) => Err(anyhow!(
            "unknown materials subcommand: {other}. Use `materials ingest`, `materials list`, or `materials search`."
        )),
    }
}

fn run_materials_ingest(paths: &AppPaths) -> Result<()> {
    let courses = CourseCatalog::load(&paths.courses_dir)?;
    let manifest = ingest_materials(paths, &courses)?;
    println!(
        "Ingested {} material files from {} into {}",
        manifest.entries.len(),
        paths.materials_raw_dir.display(),
        paths.materials_manifest_path.display()
    );
    Ok(())
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
            paths.materials_manifest_path.display()
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

fn run_timetable(paths: &AppPaths, args: &[String]) -> Result<()> {
    paths.ensure()?;

    match args.first().map(String::as_str) {
        Some("show") | None => run_timetable_show(paths),
        Some("today") => run_timetable_today(paths),
        Some("add") => run_timetable_add(paths, &args[1..]),
        Some(other) => Err(anyhow!(
            "unknown timetable subcommand: {other}. Use `timetable show`, `timetable today`, or `timetable add`."
        )),
    }
}

fn run_timetable_show(paths: &AppPaths) -> Result<()> {
    let local_context = LocalContext::load(paths)?;
    let Some(timetable) = local_context.timetable else {
        println!(
            "No timetable file loaded at {}",
            paths.timetable_path.display()
        );
        return Ok(());
    };

    println!("StudyOS timetable ({})", timetable.timezone);
    for slot in timetable.slots {
        println!(
            "- {} {}-{} | {}",
            slot.day, slot.start, slot.end, slot.title
        );
    }

    Ok(())
}

fn run_timetable_today(paths: &AppPaths) -> Result<()> {
    let local_context = LocalContext::load(paths)?;
    let today_slots = local_context.today_timetable_slots();

    if today_slots.is_empty() {
        println!("No timetable slots scheduled for today.");
        return Ok(());
    }

    println!("StudyOS timetable for today");
    for slot in today_slots {
        println!("- {}-{} | {}", slot.start, slot.end, slot.title);
    }

    Ok(())
}

fn run_timetable_add(paths: &AppPaths, args: &[String]) -> Result<()> {
    let day = required_option(args, "--day")?;
    let start = required_option(args, "--start")?;
    let end = required_option(args, "--end")?;
    let title = required_option(args, "--title")?;
    let timezone = option_value(args, "--timezone").unwrap_or_else(|| "Europe/London".to_string());

    let timetable = append_timetable_slot(
        &paths.timetable_path,
        timezone,
        TimetableSlot {
            day,
            start,
            end,
            title,
        },
    )?;

    println!(
        "Saved timetable slot to {} ({} total slots).",
        paths.timetable_path.display(),
        timetable.slots.len()
    );
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

fn raw_materials_path_looks_safe(paths: &AppPaths) -> bool {
    if paths
        .materials_raw_dir
        .components()
        .any(|component| component.as_os_str() == ".studyos")
    {
        return true;
    }

    std::process::Command::new("git")
        .arg("check-ignore")
        .arg(&paths.materials_raw_dir)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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
