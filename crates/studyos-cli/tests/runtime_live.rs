mod support;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use studyos_cli::{AppAction, CodexAppServerTransport};
use studyos_core::{ContentBlock, ResponseWidget};

use crate::support::{app_has_parse_warning, app_has_question, build_app, temp_data_root};

#[test]
#[ignore = "requires a working local codex app-server and networked model access"]
fn runtime_live_reaches_first_question() {
    if std::env::var("STUDYOS_CODEX_AVAILABLE").ok().as_deref() != Some("1") {
        eprintln!("Skipping live runtime test because STUDYOS_CODEX_AVAILABLE != 1");
        return;
    }

    let base = temp_data_root("runtime-live");
    let runtime_factory = Arc::new(CodexAppServerTransport::spawn);
    let runtime = runtime_factory()
        .unwrap_or_else(|err| panic!("failed to spawn live app-server runtime: {err}"));
    let mut app = build_app(&base, Some(runtime), Some(runtime_factory));
    app.bootstrap_runtime()
        .unwrap_or_else(|err| panic!("live runtime bootstrap failed: {err}"));

    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        app.poll_runtime();
        if app_has_question(&app) && app.active_widget().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    assert!(
        app_has_question(&app) && app.active_widget().is_some(),
        "live runtime never produced a ready question widget"
    );
    assert!(
        !app_has_parse_warning(&app),
        "live runtime produced a parse warning in the transcript"
    );
}

#[test]
#[ignore = "requires a working local codex app-server and networked model access"]
fn runtime_live_completes_submission_round_trip() {
    if std::env::var("STUDYOS_CODEX_AVAILABLE").ok().as_deref() != Some("1") {
        eprintln!("Skipping live runtime test because STUDYOS_CODEX_AVAILABLE != 1");
        return;
    }

    let base = temp_data_root("runtime-live-roundtrip");
    let runtime_factory = Arc::new(CodexAppServerTransport::spawn);
    let runtime = runtime_factory()
        .unwrap_or_else(|err| panic!("failed to spawn live app-server runtime: {err}"));
    let mut app = build_app(&base, Some(runtime), Some(runtime_factory));
    app.bootstrap_runtime()
        .unwrap_or_else(|err| panic!("live runtime bootstrap failed: {err}"));

    let first_deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < first_deadline {
        app.poll_runtime();
        if app_has_question(&app) && app.active_widget().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    assert!(
        app_has_question(&app) && app.active_widget().is_some(),
        "live runtime never produced an initial ready question widget"
    );

    fill_active_widget(&mut app);
    let starting_attempts = app
        .database
        .stats()
        .unwrap_or_else(|err| panic!("stats query failed: {err}"))
        .total_attempts;
    let initial_transcript_len = app.snapshot.transcript.len();
    app.execute_action(AppAction::SubmitCurrentAnswer);

    let second_deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < second_deadline {
        app.poll_runtime();
        let attempts = app
            .database
            .stats()
            .unwrap_or_else(|err| panic!("stats query failed: {err}"))
            .total_attempts;
        if attempts > starting_attempts && app.snapshot.transcript.len() > initial_transcript_len {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let ending_attempts = app
        .database
        .stats()
        .unwrap_or_else(|err| panic!("stats query failed: {err}"))
        .total_attempts;
    if ending_attempts <= starting_attempts {
        eprintln!("runtime log:\n{}", app.runtime_log_summary().join("\n"));
        eprintln!("transcript:\n{}", transcript_debug_dump(&app));
    }
    assert!(
        ending_attempts > starting_attempts,
        "live runtime never persisted a graded attempt after submission"
    );
    assert!(
        app.snapshot
            .activity
            .iter()
            .any(|item| item.name == "Evidence"),
        "live runtime never surfaced evaluation evidence in the activity panel"
    );
    assert!(
        !app_has_parse_warning(&app),
        "live runtime produced a parse warning after submission"
    );
}

fn fill_active_widget(app: &mut studyos_cli::App) {
    let widget = app
        .active_widget_mut()
        .unwrap_or_else(|| panic!("active widget should be present for the live question"));

    match widget {
        ResponseWidget::MatrixGrid(state) => {
            state.cells[0][0] = "0".to_string();
        }
        ResponseWidget::WorkingAnswer(state) => {
            state.working = "I set up the method and simplified it.".to_string();
            state.final_answer = "0".to_string();
        }
        ResponseWidget::StepList(state) => {
            state.steps[0] = "I recalled the core definition and applied it.".to_string();
        }
        ResponseWidget::RetrievalResponse(state) => {
            state.response = "I think the key idea is linearity.".to_string();
        }
    }
}

fn transcript_debug_dump(app: &studyos_cli::App) -> String {
    app.snapshot
        .transcript
        .iter()
        .map(|block| match block {
            ContentBlock::Paragraph(paragraph) => format!("paragraph: {}", paragraph.text),
            ContentBlock::Heading(heading) => format!("heading: {}", heading.text),
            ContentBlock::BulletList(items) => format!("bullets: {}", items.join(" | ")),
            ContentBlock::MathBlock(block) => format!("latex: {}", block.fallback_text),
            ContentBlock::MatrixBlock(block) => format!("matrix: {}", block.title),
            ContentBlock::QuestionCard(card) => {
                format!("question: {} | {}", card.title, card.prompt)
            }
            ContentBlock::HintCard(card) => format!("hint: {}", card.title),
            ContentBlock::RecapBox(boxed) => format!("recap: {}", boxed.title),
            ContentBlock::WarningBox(boxed) => format!("warning: {} | {}", boxed.title, boxed.body),
            ContentBlock::Divider => "divider".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
