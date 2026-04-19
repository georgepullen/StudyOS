use std::{
    env, fs,
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use studyos_cli::{App, AppAction, AppBootstrap, CodexAppServerTransport};
use studyos_core::{
    AppConfig, AppDatabase, AppPaths, AppSnapshot, BootstrapStudyContext, DeadlineEntry,
    LocalContext, ResponseWidget, StartupMisconceptionItem, StartupReviewItem, TimetableSlot,
    append_timetable_slot, ingest_materials, upsert_deadline,
};

#[derive(Debug, Clone)]
struct SessionScenario {
    name: &'static str,
    course: &'static str,
    answer_style: AnswerStyle,
}

#[derive(Debug, Clone, Copy)]
enum AnswerStyle {
    Sparse,
    Concise,
    Reasoned,
}

#[derive(Debug)]
struct SessionRunSummary {
    name: String,
    course: String,
    mode: String,
    question_title: String,
    question_prompt: String,
    widget_kind: String,
    attempts_delta: usize,
    parse_warning: bool,
    recap_summary: String,
}

fn main() -> Result<()> {
    if env::var("STUDYOS_CODEX_AVAILABLE").ok().as_deref() != Some("1") {
        return Err(anyhow!(
            "set STUDYOS_CODEX_AVAILABLE=1 to run the live alpha readiness harness"
        ));
    }

    let repo_root = env::current_dir()?;
    let data_root = repo_root.join("target/alpha-readiness");
    let _ = fs::remove_dir_all(&data_root);
    fs::create_dir_all(&data_root)?;

    let paths = AppPaths::discover(&data_root);
    paths.ensure()?;
    seed_alpha_data(&repo_root, &paths)?;

    let scenarios = [
        SessionScenario {
            name: "matrix-baseline",
            course: "Matrix Algebra & Linear Models",
            answer_style: AnswerStyle::Reasoned,
        },
        SessionScenario {
            name: "matrix-deadline-pressure",
            course: "Matrix Algebra & Linear Models",
            answer_style: AnswerStyle::Concise,
        },
        SessionScenario {
            name: "probability-switch",
            course: "Probability & Statistics for Scientists",
            answer_style: AnswerStyle::Reasoned,
        },
        SessionScenario {
            name: "repair-followup",
            course: "Matrix Algebra & Linear Models",
            answer_style: AnswerStyle::Sparse,
        },
        SessionScenario {
            name: "probability-review-followup",
            course: "Probability & Statistics for Scientists",
            answer_style: AnswerStyle::Concise,
        },
    ];

    let mut summaries = Vec::new();
    for scenario in scenarios {
        eprintln!("starting session `{}`", scenario.name);
        let summary = run_session(&paths, scenario)?;
        eprintln!(
            "completed session `{}` with mode `{}` and widget `{}`",
            summary.name, summary.mode, summary.widget_kind
        );
        summaries.push(summary);
    }

    let report = build_report(&paths, &summaries)?;
    println!("{report}");
    Ok(())
}

fn seed_alpha_data(repo_root: &Path, paths: &AppPaths) -> Result<()> {
    fs::write(
        &paths.config_path,
        include_str!("../../../examples/studyos-config.toml"),
    )?;
    fs::write(
        paths.courses_dir.join("linear-models.toml"),
        include_str!("../../../examples/linear-models.toml"),
    )?;
    fs::write(
        paths.courses_dir.join("probability-stats.toml"),
        include_str!("../../../examples/probability-stats.toml"),
    )?;

    let urgent_deadline = DeadlineEntry {
        id: "alpha-deadline-1".to_string(),
        source: "local".to_string(),
        title: "Mock Linear Models Exam".to_string(),
        due_at: "2026-04-20T12:00:00Z".to_string(),
        course: "Matrix Algebra & Linear Models".to_string(),
        weight: 0.7,
        notes: "Alpha-readiness urgent exam drill check".to_string(),
    };
    upsert_deadline(&paths.deadlines_path, urgent_deadline)?;

    append_timetable_slot(
        &paths.timetable_path,
        "Europe/London".to_string(),
        TimetableSlot {
            day: "monday".to_string(),
            start: "09:00".to_string(),
            end: "10:00".to_string(),
            title: "Linear Models Lecture".to_string(),
        },
    )?;
    append_timetable_slot(
        &paths.timetable_path,
        "Europe/London".to_string(),
        TimetableSlot {
            day: "monday".to_string(),
            start: "14:00".to_string(),
            end: "15:00".to_string(),
            title: "Probability Workshop".to_string(),
        },
    )?;

    fs::write(
        paths.materials_raw_dir.join("matrix-algebra-notes.md"),
        r#"# Matrix algebra notes

Matrix multiplication requires inner dimensions to match.
Each entry of AB is a row of A dotted with a column of B.
Revision checklist: dimensions, row reduction, determinants, eigenvectors.
"#,
    )?;
    fs::write(
        paths.materials_raw_dir.join("linear-models-intuition.txt"),
        "Linear models map feature rows to predictions. Interpret X beta entrywise as a row dot beta. Compare geometric intuition with algebraic form.",
    )?;
    fs::write(
        paths.materials_raw_dir.join("probability-retrieval.md"),
        r#"# Probability retrieval

Know expectation linearity, variance scaling, covariance symmetry, and when independence implies zero covariance.
Be able to explain the difference between a distribution parameter and a sample estimate.
"#,
    )?;
    let fixture_pdf =
        repo_root.join("crates/studyos-core/tests/fixtures/materials/raw/linear-models.pdf");
    fs::copy(
        fixture_pdf,
        paths.materials_raw_dir.join("alpha-linear-models.pdf"),
    )?;

    let courses = studyos_core::CourseCatalog::load(&paths.courses_dir)?;
    ingest_materials(paths, &courses)?;
    Ok(())
}

fn run_session(paths: &AppPaths, scenario: SessionScenario) -> Result<SessionRunSummary> {
    let mut config = AppConfig::load_or_default(&paths.config_path)?;
    config.default_course = scenario.course.to_string();
    config.save(&paths.config_path)?;

    let database = AppDatabase::open(&paths.database_path)?;
    let local_context = LocalContext::load(paths)?;
    let mut stats = database.stats()?;
    stats.due_reviews = database.due_review_count_for_course(scenario.course)?;
    stats.upcoming_deadlines = local_context.upcoming_deadline_count_for_course(scenario.course);
    let resume_state = database.load_resume_state()?;
    let startup_context = BootstrapStudyContext {
        due_reviews: database
            .list_due_reviews_for_course(scenario.course, 4)?
            .into_iter()
            .map(|item| StartupReviewItem {
                concept_name: item.concept_name,
            })
            .collect(),
        recent_misconceptions: database
            .list_recent_repair_signals_for_course(scenario.course, 4)?
            .into_iter()
            .map(|item| StartupMisconceptionItem {
                concept_name: item.concept_name,
                error_type: item.error_type,
                description: item.description,
            })
            .collect(),
        last_session_recap: database.latest_session_recap(scenario.course)?,
        study_window: local_context.best_study_window_for_course(scenario.course),
    };
    let snapshot = AppSnapshot::bootstrap(&config, &stats, &startup_context);
    let log_path = paths
        .logs_dir
        .join(format!("alpha-{}.jsonl", scenario.name.replace(' ', "-")));
    let runtime_factory = {
        let log_path = log_path.clone();
        Arc::new(move || CodexAppServerTransport::spawn_with_log_path(Some(log_path.clone())))
    };
    let runtime = runtime_factory()?;

    let mut app = App::new(AppBootstrap {
        database,
        paths: paths.clone(),
        config,
        stats,
        local_context,
        snapshot,
        runtime: Some(runtime),
        runtime_factory: Some(runtime_factory),
        runtime_error: None,
        resume_state,
    });

    let starting_attempts = app.database.stats()?.total_attempts;
    eprintln!("  bootstrapping runtime");
    app.bootstrap_runtime()?;
    eprintln!("  waiting for first question");
    wait_for_question(&mut app, Duration::from_secs(45))?;

    let question_title = app.active_question_title();
    let question_prompt = app.active_question_prompt().unwrap_or_default();
    let widget_kind = fill_active_widget(&mut app, scenario.answer_style)?;
    eprintln!("  submitting structured answer");
    app.execute_action(AppAction::SubmitCurrentAnswer);
    wait_for_attempt_delta(&mut app, starting_attempts, Duration::from_secs(45))?;

    eprintln!("  requesting recap");
    request_and_finalize_recap(&mut app)?;

    let ending_attempts = app.database.stats()?.total_attempts;
    let parse_warning = app.snapshot.transcript.iter().any(|block| {
        matches!(
            block,
            studyos_core::ContentBlock::WarningBox(warning)
                if warning.title.contains("parse failed")
        )
    });
    let recap_summary = app
        .database
        .latest_session_recap(scenario.course)?
        .map(|recap| recap.outcome_summary)
        .unwrap_or_else(|| "no recap".to_string());

    Ok(SessionRunSummary {
        name: scenario.name.to_string(),
        course: scenario.course.to_string(),
        mode: app.current_mode_label().to_string(),
        question_title,
        question_prompt,
        widget_kind,
        attempts_delta: ending_attempts.saturating_sub(starting_attempts),
        parse_warning,
        recap_summary,
    })
}

fn wait_for_question(app: &mut App, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        app.poll_runtime();
        if app.active_widget().is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(anyhow!(
        "live runtime never produced a structured question\n{}",
        app.runtime_log_summary().join("\n")
    ))
}

fn wait_for_attempt_delta(app: &mut App, baseline: usize, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        app.poll_runtime();
        if app.database.stats()?.total_attempts > baseline {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(anyhow!(
        "live runtime never persisted a graded attempt\n{}",
        app.runtime_log_summary().join("\n")
    ))
}

fn request_and_finalize_recap(app: &mut App) -> Result<()> {
    let first_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    if let Some(action) = app.handle_key(first_q) {
        app.execute_action(action);
    }

    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        app.poll_runtime();
        if app.quit_recap_preview().is_some() && !app.quit_recap_is_preparing() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    eprintln!("  finalizing recap");
    let second_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    if let Some(action) = app.handle_key(second_q) {
        app.execute_action(action);
    }
    if !app.should_quit {
        app.finish_session()?;
    }
    Ok(())
}

fn fill_active_widget(app: &mut App, answer_style: AnswerStyle) -> Result<String> {
    let widget = app
        .active_widget_mut()
        .ok_or_else(|| anyhow!("expected an active widget"))?;

    let kind = match widget {
        ResponseWidget::MatrixGrid(state) => {
            match answer_style {
                AnswerStyle::Sparse => {
                    state.cells[0][0] = "0".to_string();
                }
                AnswerStyle::Concise => {
                    state.cells[0][0] = "1".to_string();
                    if state.cells[0].len() > 1 {
                        state.cells[0][1] = "0".to_string();
                    }
                }
                AnswerStyle::Reasoned => {
                    state.cells[0][0] = "1".to_string();
                    if state.cells.len() > 1 && state.cells[1].len() > 1 {
                        state.cells[1][1] = "1".to_string();
                    }
                }
            }
            "matrix_grid"
        }
        ResponseWidget::WorkingAnswer(state) => {
            match answer_style {
                AnswerStyle::Sparse => {
                    state.working = "Tried the setup.".to_string();
                    state.final_answer = "0".to_string();
                }
                AnswerStyle::Concise => {
                    state.working =
                        "I identified the formula and substituted the key terms.".to_string();
                    state.final_answer = "0".to_string();
                }
                AnswerStyle::Reasoned => {
                    state.working =
                        "I recalled the definition, checked dimensions, and applied it stepwise."
                            .to_string();
                    state.final_answer = "0".to_string();
                }
            }
            "working_answer"
        }
        ResponseWidget::StepList(state) => {
            let first = match answer_style {
                AnswerStyle::Sparse => "Applied the first definition.",
                AnswerStyle::Concise => "Started from the definition and simplified one step.",
                AnswerStyle::Reasoned => {
                    "Started from the definition, justified the transition, and simplified."
                }
            };
            if !state.steps.is_empty() {
                state.steps[0] = first.to_string();
            }
            "step_list"
        }
        ResponseWidget::RetrievalResponse(state) => {
            state.response = match answer_style {
                AnswerStyle::Sparse => "Linearity.",
                AnswerStyle::Concise => {
                    "Expectation is linear, so constants pull out and sums split."
                }
                AnswerStyle::Reasoned => {
                    "Expectation is linear, so I can split sums and pull out constants even without independence."
                }
            }
            .to_string();
            "retrieval_response"
        }
    };

    Ok(kind.to_string())
}

fn build_report(paths: &AppPaths, summaries: &[SessionRunSummary]) -> Result<String> {
    let database = AppDatabase::open(&paths.database_path)?;
    let stats = database.stats()?;
    let repair_signals = database.list_recent_repair_signals(5)?;

    let mut report = String::new();
    report.push_str("# Alpha Readiness Report\n\n");
    report.push_str(&format!("data root: `{}`\n\n", paths.root_dir.display()));
    report.push_str("## Session summaries\n\n");
    for summary in summaries {
        report.push_str(&format!(
            "- `{}` course=`{}` mode=`{}` widget=`{}` attempts_delta=`{}` parse_warning=`{}`\n",
            summary.name,
            summary.course,
            summary.mode,
            summary.widget_kind,
            summary.attempts_delta,
            summary.parse_warning
        ));
        report.push_str(&format!("  question: {}\n", summary.question_title));
        report.push_str(&format!("  prompt: {}\n", summary.question_prompt));
        report.push_str(&format!("  recap: {}\n", summary.recap_summary));
    }

    report.push_str("\n## Aggregate state\n\n");
    report.push_str(&format!("- sessions_logged: {}\n", stats.total_sessions));
    report.push_str(&format!("- attempts_logged: {}\n", stats.total_attempts));
    report.push_str(&format!("- due_reviews: {}\n", stats.due_reviews));
    report.push_str(&format!(
        "- recent_repair_signals: {}\n",
        repair_signals.len()
    ));
    for signal in repair_signals {
        report.push_str(&format!(
            "  - {} [{} | {}] x{}\n",
            signal.concept_name, signal.error_type, signal.status, signal.evidence_count
        ));
    }

    Ok(report)
}
