mod support;

use std::{path::PathBuf, time::Duration};

use studyos_cli::ReplayAppServerTransport;

use crate::support::{app_has_parse_warning, app_has_question, build_app, temp_data_root};

#[test]
fn runtime_replay_reaches_first_question_without_parse_warning() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = repo_root.join("tests/fixtures/runtime/opening-turn.jsonl");
    assert!(
        fixture_path.exists(),
        "runtime fixture missing: {}",
        fixture_path.display()
    );

    let runtime = ReplayAppServerTransport::from_fixture(&fixture_path)
        .unwrap_or_else(|err| panic!("replay transport init failed: {err}"));
    let base = temp_data_root("runtime-replay");
    let mut app = build_app(&base, Some(runtime), None);
    app.bootstrap_runtime()
        .unwrap_or_else(|err| panic!("runtime bootstrap failed: {err}"));

    for _ in 0..8 {
        app.poll_runtime();
        if app_has_question(&app) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        app_has_question(&app),
        "replay run never produced a question card"
    );
    assert!(
        !app_has_parse_warning(&app),
        "replay run produced a parse warning in the transcript"
    );
}
