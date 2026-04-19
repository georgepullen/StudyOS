use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
    time::Instant,
};

use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use studyos_core::{
    ActivityItem, ActivityStatus, AppConfig, AppDatabase, AppPaths, AppSnapshot, AppStats,
    AttemptRecord, ContentBlock, LocalContext, MatrixDimensions, MatrixGridState,
    MisconceptionInput, PanelTab, QuestionCard, ResponseWidget, ResponseWidgetKind,
    ResumeStateRecord, RetrievalResponseState, SessionMode, SessionRecapRecord,
    SessionRecapSummary, SessionRecord, StepListState, TutorCorrectness, TutorErrorType,
    TutorEvaluation, TutorReasoningQuality, TutorSessionClosePayload, TutorTurnPayload, WarningBox,
    WorkingAnswerField, WorkingAnswerState,
};

use crate::runtime::{AppServerTransport, RuntimeEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRegion {
    Transcript,
    Panel,
    Widget,
    Scratchpad,
}

impl FocusRegion {
    pub fn label(self) -> &'static str {
        match self {
            Self::Transcript => "Transcript",
            Self::Panel => "Panel",
            Self::Widget => "Widget",
            Self::Scratchpad => "Scratchpad",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Transcript => Self::Panel,
            Self::Panel => Self::Widget,
            Self::Widget => Self::Scratchpad,
            Self::Scratchpad => Self::Transcript,
        }
    }
}

pub enum AppAction {
    SubmitCurrentAnswer,
    ReconnectRuntime,
}

pub struct AppBootstrap {
    pub database: AppDatabase,
    pub paths: AppPaths,
    pub config: AppConfig,
    pub stats: AppStats,
    pub local_context: LocalContext,
    pub snapshot: AppSnapshot,
    pub runtime: Option<Arc<dyn AppServerTransport>>,
    pub runtime_factory: Option<RuntimeFactory>,
    pub runtime_error: Option<String>,
    pub resume_state: Option<ResumeStateRecord>,
}

type RuntimeFactory = Arc<dyn Fn() -> Result<Arc<dyn AppServerTransport>> + Send + Sync>;

#[derive(Clone)]
struct PendingAttemptContext {
    question_title: String,
    question_prompt: String,
    concept_tags: Vec<String>,
    widget_kind: ResponseWidgetKind,
    student_answer: String,
    latency_ms: i64,
}

#[derive(Clone)]
struct TutorPendingTurn {
    display_user_text: Option<String>,
    attempt: Option<PendingAttemptContext>,
    retry_count: u8,
}

#[derive(Clone)]
struct PendingRecapTurn {
    fallback: SessionRecapSummary,
    retry_count: u8,
}

enum QuitState {
    Idle,
    Preparing(SessionRecapSummary),
    Ready(SessionRecapSummary),
}

pub struct App {
    pub database: AppDatabase,
    pub paths: AppPaths,
    pub config: AppConfig,
    pub stats: AppStats,
    pub local_context: LocalContext,
    pub snapshot: AppSnapshot,
    pub focus: FocusRegion,
    pub show_help: bool,
    pub should_quit: bool,
    pub transcript_scroll: u16,
    pub widget_states: HashMap<usize, ResponseWidget>,
    pub active_question_index: usize,
    runtime: Option<Arc<dyn AppServerTransport>>,
    runtime_factory: Option<RuntimeFactory>,
    runtime_thread_id: Option<String>,
    runtime_ready: bool,
    runtime_disconnected: bool,
    runtime_bootstrap_applied: bool,
    pending_structured_turns: HashMap<String, TutorPendingTurn>,
    pending_recap_turn: Option<(String, PendingRecapTurn)>,
    live_message_indices: HashMap<String, usize>,
    structured_buffers: HashMap<String, String>,
    question_presented_at: HashMap<usize, Instant>,
    current_session_id: String,
    session_started_at: Instant,
    session_finished: bool,
    session_outcomes: Vec<String>,
    quit_state: QuitState,
}

impl App {
    pub fn new(bootstrap: AppBootstrap) -> Self {
        let AppBootstrap {
            database,
            paths,
            config,
            stats,
            local_context,
            snapshot,
            runtime,
            runtime_factory,
            runtime_error,
            resume_state,
        } = bootstrap;

        let saved_course_thread = database
            .load_course_runtime_thread(&snapshot.course)
            .unwrap_or(None);

        let question_indices = Self::question_indices_from(&snapshot.transcript);
        let active_question_index = *question_indices.first().unwrap_or(&0);

        let widget_states = question_indices
            .iter()
            .filter_map(|index| {
                let ContentBlock::QuestionCard(card) = &snapshot.transcript[*index] else {
                    return None;
                };

                Some((*index, widget_state_for_question(card)))
            })
            .collect();
        let question_presented_at = question_indices
            .iter()
            .map(|index| (*index, Instant::now()))
            .collect();

        let session_seed = config.default_course.clone();

        let mut app = Self {
            database,
            paths,
            config,
            stats,
            local_context,
            snapshot,
            focus: FocusRegion::Widget,
            show_help: false,
            should_quit: false,
            transcript_scroll: 0,
            widget_states,
            active_question_index,
            runtime,
            runtime_factory,
            runtime_thread_id: saved_course_thread,
            runtime_ready: false,
            runtime_disconnected: false,
            runtime_bootstrap_applied: false,
            pending_structured_turns: HashMap::new(),
            pending_recap_turn: None,
            live_message_indices: HashMap::new(),
            structured_buffers: HashMap::new(),
            question_presented_at,
            current_session_id: make_id("session", &session_seed),
            session_started_at: Instant::now(),
            session_finished: false,
            session_outcomes: Vec::new(),
            quit_state: QuitState::Idle,
        };

        if let Some(resume) = resume_state {
            app.apply_resume_state(resume);
        }

        app.set_activity(
            "Resume",
            "Resume state is now loaded from local SQLite when available.".to_string(),
            ActivityStatus::Healthy,
        );
        app.set_activity(
            "Local context",
            format!(
                "{} deadlines, {} materials, {} course files discovered.",
                app.local_context.deadlines.len(),
                app.local_context.materials.len(),
                app.local_context.courses.courses.len()
            ),
            ActivityStatus::Healthy,
        );

        match runtime_error {
            Some(error) => app.set_activity("App-server", error, ActivityStatus::Idle),
            None if app.runtime.is_some() => app.set_activity(
                "App-server",
                "Codex app-server process spawned; waiting for initialization.".to_string(),
                ActivityStatus::Running,
            ),
            None => app.set_activity(
                "App-server",
                "Codex app-server unavailable; shell is running in local fallback mode."
                    .to_string(),
                ActivityStatus::Idle,
            ),
        }

        match app
            .start_session_record()
            .and_then(|_| app.refresh_snapshot_metrics())
        {
            Ok(()) => app.set_activity(
                "SQLite",
                "Local study memory opened, session recorded, and metrics refreshed.".to_string(),
                ActivityStatus::Healthy,
            ),
            Err(error) => app.set_activity(
                "SQLite",
                format!("Failed to start session record: {error}"),
                ActivityStatus::Idle,
            ),
        }

        app
    }

    pub fn bootstrap_runtime(&mut self) -> Result<()> {
        if self.runtime.is_none() {
            return Ok(());
        }

        let developer_instructions = self.developer_instructions();
        let cwd = self.paths.root_dir.parent().unwrap_or(&self.paths.root_dir);

        {
            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| anyhow!("runtime unavailable"))?;
            runtime.initialize()?;
            self.runtime_ready = true;

            let thread_id = if let Some(existing) = self.runtime_thread_id.as_deref() {
                runtime.resume_thread(existing, cwd)?
            } else {
                runtime.start_thread(cwd, &developer_instructions)?
            };

            self.runtime_thread_id = Some(thread_id.clone());

            let opening_prompt = self.build_opening_prompt();
            let turn_id = runtime.start_structured_turn(
                &thread_id,
                &opening_prompt,
                tutor_output_schema(),
                cwd,
            )?;
            self.pending_structured_turns.insert(
                turn_id,
                TutorPendingTurn {
                    display_user_text: None,
                    attempt: None,
                    retry_count: 0,
                },
            );
            trim_hash_map(
                &mut self.pending_structured_turns,
                MAX_PENDING_RUNTIME_MAP_ENTRIES,
            );
        }

        self.set_activity(
            "App-server",
            "Connected to Codex app-server and started structured tutor turn.".to_string(),
            ActivityStatus::Running,
        );
        self.persist_resume_state()?;
        Ok(())
    }

    pub fn poll_runtime(&mut self) {
        let Some(runtime) = &self.runtime else {
            return;
        };

        let events = runtime.poll_events();
        for event in events {
            self.handle_runtime_event(event);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<AppAction> {
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                _ => {}
            }
            return None;
        }

        match (&self.quit_state, key.code) {
            (QuitState::Preparing(fallback), KeyCode::Char('q'))
            | (QuitState::Preparing(fallback), KeyCode::Enter) => {
                let fallback = fallback.clone();
                if let Err(error) =
                    self.finalize_session_with_recap(fallback, Some("forced_during_recap"))
                {
                    self.push_block(ContentBlock::WarningBox(WarningBox {
                        title: "Session close failed".to_string(),
                        body: error.to_string(),
                    }));
                } else {
                    self.should_quit = true;
                }
                return None;
            }
            (QuitState::Ready(recap), KeyCode::Char('q'))
            | (QuitState::Ready(recap), KeyCode::Enter) => {
                let recap = recap.clone();
                if let Err(error) = self.finalize_session_with_recap(recap, None) {
                    self.push_block(ContentBlock::WarningBox(WarningBox {
                        title: "Session close failed".to_string(),
                        body: error.to_string(),
                    }));
                } else {
                    self.should_quit = true;
                }
                return None;
            }
            (QuitState::Preparing(_), KeyCode::Esc) | (QuitState::Ready(_), KeyCode::Esc) => {
                self.quit_state = QuitState::Idle;
                self.set_activity(
                    "Session close",
                    "Returned to the active study session without closing.".to_string(),
                    ActivityStatus::Healthy,
                );
                return None;
            }
            (QuitState::Preparing(_), _) | (QuitState::Ready(_), _) => return None,
            (QuitState::Idle, _) => {}
        }

        match key.code {
            KeyCode::Char('q') => {
                if let Err(error) = self.request_quit() {
                    self.push_block(ContentBlock::WarningBox(WarningBox {
                        title: "Session close failed".to_string(),
                        body: error.to_string(),
                    }));
                }
                return None;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                return None;
            }
            KeyCode::Tab => {
                self.focus = self.focus.next();
                return None;
            }
            KeyCode::F(5) => {
                return Some(AppAction::SubmitCurrentAnswer);
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(AppAction::ReconnectRuntime);
            }
            KeyCode::Char('1')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.snapshot.panel_tab = PanelTab::SessionPlan;
                return None;
            }
            KeyCode::Char('2')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.snapshot.panel_tab = PanelTab::DueReviews;
                return None;
            }
            KeyCode::Char('3')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.snapshot.panel_tab = PanelTab::Deadlines;
                return None;
            }
            KeyCode::Char('4')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.snapshot.panel_tab = PanelTab::Misconceptions;
                return None;
            }
            KeyCode::Char('5')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.snapshot.panel_tab = PanelTab::Scratchpad;
                return None;
            }
            KeyCode::Char('6')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.snapshot.panel_tab = PanelTab::Activity;
                return None;
            }
            KeyCode::Char('7')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.snapshot.panel_tab = PanelTab::RuntimeLog;
                return None;
            }
            KeyCode::Char(']')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.advance_question(1);
                return None;
            }
            KeyCode::Char('[')
                if !matches!(self.focus, FocusRegion::Widget | FocusRegion::Scratchpad) =>
            {
                self.advance_question(-1);
                return None;
            }
            _ => {}
        }

        match self.focus {
            FocusRegion::Transcript => self.handle_transcript_key(key),
            FocusRegion::Panel => self.handle_panel_key(key),
            FocusRegion::Widget => self.handle_widget_key(key),
            FocusRegion::Scratchpad => self.handle_scratchpad_key(key),
        }

        None
    }

    pub fn execute_action(&mut self, action: AppAction) {
        if let Err(error) = self.execute_action_inner(action) {
            self.push_block(ContentBlock::WarningBox(WarningBox {
                title: "Runtime action failed".to_string(),
                body: error.to_string(),
            }));
            self.set_activity("App-server", error.to_string(), ActivityStatus::Idle);
        }
    }

    pub fn finish_session(&mut self) -> Result<()> {
        if self.session_finished {
            return Ok(());
        }

        self.finalize_session_with_recap(self.fallback_session_recap(), Some("shutdown_fallback"))
    }

    pub fn active_widget(&self) -> Option<&ResponseWidget> {
        if !self.live_runtime_question_ready() {
            return None;
        }
        self.widget_states.get(&self.active_question_index)
    }

    pub fn quit_recap_preview(&self) -> Option<&SessionRecapSummary> {
        match &self.quit_state {
            QuitState::Idle => None,
            QuitState::Preparing(recap) | QuitState::Ready(recap) => Some(recap),
        }
    }

    pub fn quit_recap_is_preparing(&self) -> bool {
        matches!(self.quit_state, QuitState::Preparing(_))
    }

    pub fn current_mode_label(&self) -> &'static str {
        if self.quit_recap_preview().is_some() {
            SessionMode::Recap.label()
        } else {
            self.snapshot.mode.label()
        }
    }

    pub fn active_widget_mut(&mut self) -> Option<&mut ResponseWidget> {
        if !self.live_runtime_question_ready() {
            return None;
        }
        self.widget_states.get_mut(&self.active_question_index)
    }

    pub fn persist_resume_state(&self) -> Result<()> {
        let draft_payload = build_resume_draft_payload(self.active_widget())?;

        let record = ResumeStateRecord {
            session_id: "study-session".to_string(),
            runtime_thread_id: self.runtime_thread_id.clone(),
            active_course: self.snapshot.course.clone(),
            active_mode: self.snapshot.mode.label().to_string(),
            active_question_id: Some(self.active_question_index.to_string()),
            focused_panel: self.snapshot.panel_tab.label().to_string(),
            draft_payload,
            scratchpad_text: self.snapshot.scratchpad.clone(),
        };

        self.database.save_resume_state(&record)?;
        self.database
            .save_course_runtime_thread(&self.snapshot.course, self.runtime_thread_id.as_deref())
    }

    pub fn active_question_title(&self) -> String {
        self.snapshot
            .transcript
            .get(self.active_question_index)
            .and_then(|block| match block {
                ContentBlock::QuestionCard(card) => Some(card.title.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "Structured Answer".to_string())
    }

    pub fn active_question_prompt(&self) -> Option<String> {
        self.snapshot
            .transcript
            .get(self.active_question_index)
            .and_then(|block| match block {
                ContentBlock::QuestionCard(card) => Some(card.prompt.clone()),
                _ => None,
            })
    }

    pub fn question_indices(&self) -> Vec<usize> {
        if !self.live_runtime_question_ready() {
            return Vec::new();
        }
        Self::question_indices_from(&self.snapshot.transcript)
    }

    pub fn status_line(&self) -> String {
        let runtime_label = if self.runtime_disconnected {
            "App-server disconnected"
        } else if self.runtime.is_some() && !self.runtime_bootstrap_applied {
            "Waiting for live tutor question"
        } else if self.runtime_ready {
            "App-server connected"
        } else if self.runtime.is_some() {
            "App-server starting"
        } else {
            "Local fallback"
        };
        let quit_label = if self.quit_recap_is_preparing() {
            " | Quit recap preparing"
        } else if self.quit_recap_preview().is_some() {
            " | Quit review open"
        } else {
            ""
        };

        format!(
            "Focus: {} | Panel: {} | Strictness: {:?} | Sessions: {} | Attempts: {} | Runtime thread: {} | {}{}",
            self.focus.label(),
            self.snapshot.panel_tab.label(),
            self.config.strictness,
            self.stats.total_sessions,
            self.stats.total_attempts,
            self.runtime_thread_id.as_deref().unwrap_or("not-started"),
            runtime_label,
            quit_label,
        )
    }

    pub fn misconceptions_summary(&self) -> Vec<String> {
        match self
            .database
            .list_recent_repair_signals_for_course(&self.snapshot.course, 4)
        {
            Ok(entries) if !entries.is_empty() => {
                let mut lines = vec!["Recent repair signals:".to_string()];
                for entry in entries {
                    lines.push(format!(
                        "• {} [{} | {}] x{}",
                        entry.concept_name, entry.error_type, entry.status, entry.evidence_count
                    ));
                    lines.push(format!("  {}", entry.description));
                }
                lines
            }
            Ok(_) => vec![
                "No repair signals yet; repeated evidence will accumulate here before confirmed misconceptions are promoted.".to_string(),
            ],
            Err(error) => vec![format!("Misconception summary unavailable: {error}")],
        }
    }

    pub fn review_summary(&self) -> Vec<String> {
        match self
            .database
            .list_due_reviews_for_course(&self.snapshot.course, 4)
        {
            Ok(reviews) if !reviews.is_empty() => {
                let mut lines = vec![format!(
                    "Due review count: {}",
                    self.snapshot.metrics.due_reviews
                )];
                for review in reviews {
                    lines.push(format!(
                        "• {} due {}",
                        review.concept_name, review.next_review_at
                    ));
                }
                lines
            }
            Ok(_) => vec![
                format!("Due review count: {}", self.snapshot.metrics.due_reviews),
                "No due retrieval items yet; correct answers will schedule future reviews."
                    .to_string(),
            ],
            Err(error) => vec![format!("Review queue unavailable: {error}")],
        }
    }

    pub fn deadline_summary(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "Upcoming deadlines within 14 days: {}",
                self.snapshot.metrics.upcoming_deadlines
            ),
            "V1 uses local deadlines.json and timetable.json rather than live integrations."
                .to_string(),
            "Urgency should bias toward repair mode when deadlines draw near.".to_string(),
        ];

        if self.local_context.deadlines.is_empty() {
            lines.push(format!(
                "No local deadlines loaded. Put JSON data at {}.",
                self.paths.deadlines_path.display()
            ));
        } else {
            for deadline in self.local_context.deadlines.iter().take(3) {
                lines.push(format!("• {} ({})", deadline.title, deadline.due_at));
            }
        }

        if let Some(timetable) = &self.local_context.timetable {
            lines.push(format!(
                "Timetable loaded: {} slots in {}.",
                timetable.slots.len(),
                timetable.timezone
            ));
            for slot in self.local_context.next_timetable_slots(3) {
                lines.push(format!(
                    "• {} {}-{} {}",
                    slot.day, slot.start, slot.end, slot.title
                ));
            }
        }

        lines
    }

    pub fn runtime_log_summary(&self) -> Vec<String> {
        self.runtime
            .as_ref()
            .map(|runtime| runtime.runtime_log_lines())
            .filter(|lines| !lines.is_empty())
            .unwrap_or_else(|| vec!["No runtime log available.".to_string()])
    }

    fn execute_action_inner(&mut self, action: AppAction) -> Result<()> {
        match action {
            AppAction::SubmitCurrentAnswer => self.submit_current_answer(),
            AppAction::ReconnectRuntime => self.reconnect_runtime(),
        }
    }

    fn fallback_session_recap(&self) -> SessionRecapSummary {
        let mut weak_concepts = self
            .database
            .list_recent_repair_signals_for_course(&self.snapshot.course, 3)
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.concept_name)
            .collect::<Vec<_>>();
        weak_concepts.dedup();

        let unfinished_objectives = match self.active_question_prompt() {
            Some(prompt) if !prompt.trim().is_empty() => vec![prompt],
            _ => Vec::new(),
        };

        SessionRecapSummary {
            outcome_summary: if self.session_outcomes.is_empty() {
                "Session ended before any graded evidence was captured.".to_string()
            } else {
                self.session_outcomes.join(" | ")
            },
            demonstrated_concepts: self
                .database
                .list_due_reviews_for_course(&self.snapshot.course, 3)
                .unwrap_or_default()
                .into_iter()
                .map(|item| item.concept_name)
                .collect(),
            weak_concepts,
            next_review_items: self
                .database
                .list_due_reviews_for_course(&self.snapshot.course, 3)
                .unwrap_or_default()
                .into_iter()
                .map(|item| format!("{} at {}", item.concept_name, item.next_review_at))
                .collect(),
            unfinished_objectives,
        }
    }

    fn start_session_record(&mut self) -> Result<()> {
        let record = SessionRecord {
            id: self.current_session_id.clone(),
            planned_minutes: self.snapshot.time_remaining_minutes,
            mode: self.snapshot.mode.label().to_string(),
            course: self.snapshot.course.clone(),
        };
        self.database.start_session(&record)
    }

    fn live_runtime_question_ready(&self) -> bool {
        self.runtime.is_none() || self.runtime_bootstrap_applied
    }

    fn refresh_snapshot_metrics(&mut self) -> Result<()> {
        let mut stats = self.database.stats()?;
        stats.due_reviews = self
            .database
            .due_review_count_for_course(&self.snapshot.course)?;
        stats.upcoming_deadlines = self
            .local_context
            .upcoming_deadline_count_for_course(&self.snapshot.course);
        self.stats = stats.clone();
        self.snapshot.metrics.due_reviews = stats.due_reviews;
        self.snapshot.metrics.upcoming_deadlines = stats.upcoming_deadlines;
        self.snapshot.metrics.attempts_logged = stats.total_attempts;
        self.snapshot.metrics.sessions_logged = stats.total_sessions;
        self.snapshot.deadline_urgency = if stats.upcoming_deadlines >= 2 {
            studyos_core::DeadlineUrgency::Urgent
        } else if stats.upcoming_deadlines > 0 {
            studyos_core::DeadlineUrgency::Upcoming
        } else {
            studyos_core::DeadlineUrgency::Calm
        };
        Ok(())
    }

    fn request_quit(&mut self) -> Result<()> {
        let fallback = self.fallback_session_recap();
        if self.pending_recap_turn.is_some() {
            self.quit_state = QuitState::Preparing(fallback);
            return Ok(());
        }

        let Some(runtime) = &self.runtime else {
            self.quit_state = QuitState::Ready(fallback);
            return Ok(());
        };
        let Some(thread_id) = &self.runtime_thread_id else {
            self.quit_state = QuitState::Ready(fallback);
            return Ok(());
        };
        if !self.pending_structured_turns.is_empty() {
            self.quit_state = QuitState::Ready(fallback);
            self.set_activity(
                "Session close",
                "Tutor turn still in flight, so quit review fell back to the local recap."
                    .to_string(),
                ActivityStatus::Healthy,
            );
            return Ok(());
        }

        let cwd = self.paths.root_dir.parent().unwrap_or(&self.paths.root_dir);
        let prompt = self.build_close_prompt();
        let turn_id =
            runtime.start_structured_turn(thread_id, &prompt, tutor_close_output_schema(), cwd)?;
        self.pending_recap_turn = Some((
            turn_id,
            PendingRecapTurn {
                fallback: fallback.clone(),
                retry_count: 0,
            },
        ));
        self.quit_state = QuitState::Preparing(fallback);
        self.set_activity(
            "Session close",
            "Preparing recap without blocking the session. Press q again to force close."
                .to_string(),
            ActivityStatus::Running,
        );
        Ok(())
    }

    fn finalize_session_with_recap(
        &mut self,
        recap: SessionRecapSummary,
        aborted_reason: Option<&str>,
    ) -> Result<()> {
        if self.session_finished {
            return Ok(());
        }

        let actual_minutes = (self.session_started_at.elapsed().as_secs() / 60) as i64;
        let outcome_summary = recap.outcome_summary.clone();
        self.database.complete_session(
            &self.current_session_id,
            actual_minutes,
            &outcome_summary,
            aborted_reason,
        )?;
        self.database.save_session_recap(&SessionRecapRecord {
            session_id: self.current_session_id.clone(),
            recap,
        })?;
        self.session_finished = true;
        self.quit_state = QuitState::Idle;
        self.pending_recap_turn = None;
        self.refresh_snapshot_metrics()?;
        self.persist_resume_state()?;
        Ok(())
    }

    fn reconnect_runtime(&mut self) -> Result<()> {
        let Some(factory) = &self.runtime_factory else {
            return Err(anyhow!(
                "runtime reconnect is unavailable in this environment"
            ));
        };

        self.runtime = Some(factory()?);
        self.runtime_ready = false;
        self.runtime_disconnected = false;
        self.pending_structured_turns.clear();
        self.pending_recap_turn = None;
        self.live_message_indices.clear();
        self.structured_buffers.clear();
        self.bootstrap_runtime()?;
        self.set_activity(
            "App-server",
            "Respawned Codex app-server and attempted to resume the thread.".to_string(),
            ActivityStatus::Running,
        );
        Ok(())
    }

    fn submit_current_answer(&mut self) -> Result<()> {
        if self.runtime_disconnected {
            return Err(anyhow!(
                "app-server is disconnected; press Ctrl+R to reconnect before submitting"
            ));
        }
        if !self.runtime_ready {
            return Err(anyhow!(
                "app-server is still starting; wait for the thread to become ready before submitting"
            ));
        }

        if let Some(warning) = self.active_widget().and_then(widget_validation_warning) {
            self.push_block(ContentBlock::WarningBox(warning));
            return Ok(());
        }

        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow!("app-server runtime is unavailable"))?;
        let thread_id = self
            .runtime_thread_id
            .clone()
            .ok_or_else(|| anyhow!("no runtime thread is active"))?;
        let prompt = self.build_submission_prompt();
        let attempt = self.build_pending_attempt_context();
        let cwd = self.paths.root_dir.parent().unwrap_or(&self.paths.root_dir);
        let turn_id = runtime.start_structured_turn(
            &thread_id,
            &prompt,
            tutor_submission_output_schema(),
            cwd,
        )?;
        self.pending_structured_turns.insert(
            turn_id,
            TutorPendingTurn {
                display_user_text: Some(format!("Submitted answer: {}", attempt.question_title)),
                attempt: Some(attempt),
                retry_count: 0,
            },
        );
        trim_hash_map(
            &mut self.pending_structured_turns,
            MAX_PENDING_RUNTIME_MAP_ENTRIES,
        );
        self.set_activity(
            "App-server",
            "Submitted structured student answer for grading and next-step planning.".to_string(),
            ActivityStatus::Running,
        );
        Ok(())
    }

    pub fn build_opening_prompt(&self) -> String {
        let deadlines = if self.local_context.deadlines.is_empty() {
            "No local deadlines loaded.".to_string()
        } else {
            self.local_context
                .deadlines
                .iter()
                .take(3)
                .map(|deadline| format!("{} due {}", deadline.title, deadline.due_at))
                .collect::<Vec<_>>()
                .join("; ")
        };

        let course_names = self
            .local_context
            .courses
            .courses
            .iter()
            .map(|course| course.title.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let due_review_concepts = self
            .database
            .list_due_reviews_for_course(&self.snapshot.course, 3)
            .unwrap_or_default()
            .into_iter()
            .map(|review| review.concept_name)
            .collect::<Vec<_>>()
            .join(", ");
        let misconceptions = self
            .database
            .list_recent_repair_signals_for_course(&self.snapshot.course, 3)
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                format!(
                    "{} [{}]: {}",
                    item.concept_name, item.status, item.description
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let material_terms = self
            .database
            .list_due_reviews_for_course(&self.snapshot.course, 3)
            .unwrap_or_default()
            .into_iter()
            .map(|review| review.concept_name)
            .chain(
                self.database
                    .list_recent_repair_signals_for_course(&self.snapshot.course, 3)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|item| item.concept_name),
            )
            .collect::<Vec<_>>();
        let relevant_materials = self
            .local_context
            .search_materials(Some(&self.snapshot.course), &material_terms, 3)
            .into_iter()
            .map(|entry| {
                format!(
                    "{} [{}]: {}",
                    entry.title, entry.material_type, entry.snippet
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let timetable_summary = self
            .local_context
            .next_timetable_slots(3)
            .into_iter()
            .map(|slot| format!("{} {}-{} {}", slot.day, slot.start, slot.end, slot.title))
            .collect::<Vec<_>>()
            .join("; ");
        let study_window = self
            .snapshot
            .plan
            .window
            .as_ref()
            .map(|window| {
                format!(
                    "{} minutes via {:?} starting {}",
                    window.duration_minutes, window.source, window.start
                )
            })
            .unwrap_or_else(|| "none".to_string());

        format!(
            "You are the StudyOS tutor runtime. Return JSON matching the provided schema only.\n\
            Build a concise opening study step for a real student session.\n\
            Course focus: {course}\n\
            Available session minutes: {minutes}\n\
            Study window: {study_window}\n\
            Locally routed opening mode: {mode}\n\
            Local route rationale: {why_now}\n\
            Due review count: {due_reviews}\n\
            Upcoming deadlines: {deadlines}\n\
            Local courses loaded: {course_names}\n\
            Due review concepts: {due_review_concepts}\n\
            Recent misconceptions: {misconceptions}\n\
            Relevant local materials: {relevant_materials}\n\
            Upcoming timetable slots: {timetable_summary}\n\
            Strictness: {:?}\n\
            Requirements:\n\
            - retrieval first, not explanation first\n\
            - one short session plan\n\
            - 1 to 3 teaching blocks max before the question\n\
            - exactly one active question using one of the supported widgets\n\
            - always include matrix_dimensions; set it to null for non-matrix widgets\n\
            - evaluation must be null on this opening turn\n\
            - prefer matrix_grid for matrix algebra warmups when appropriate and set matrix_dimensions to the intended answer shape\n\
            - keep the tone direct and anti-passive",
            self.config.strictness,
            course = self.snapshot.course,
            minutes = self.snapshot.time_remaining_minutes,
            study_window = study_window,
            mode = self.snapshot.mode.label(),
            why_now = self.snapshot.plan.why_now,
            due_reviews = self.snapshot.metrics.due_reviews,
            deadlines = deadlines,
            course_names = course_names,
            due_review_concepts = if due_review_concepts.is_empty() {
                "none".to_string()
            } else {
                due_review_concepts
            },
            misconceptions = if misconceptions.is_empty() {
                "none".to_string()
            } else {
                misconceptions
            },
            relevant_materials = if relevant_materials.is_empty() {
                "none".to_string()
            } else {
                relevant_materials
            },
            timetable_summary = if timetable_summary.is_empty() {
                "none".to_string()
            } else {
                timetable_summary
            },
        )
    }

    fn build_submission_prompt(&self) -> String {
        let answer = self.widget_submission_summary();
        let title = self.active_question_title();
        let prompt = self
            .active_question_prompt()
            .unwrap_or_else(|| "No prompt recorded.".to_string());

        format!(
            "Return JSON matching the provided schema only.\n\
            The student answered a StudyOS structured question.\n\
            Current mode: {}\n\
            Question title: {}\n\
            Question prompt: {}\n\
            Student answer summary:\n{}\n\
            Requirements:\n\
            - give concise feedback through teaching_blocks\n\
            - if the answer is weak, repair the misconception before novelty\n\
            - if the answer is correct, ask one transfer or explanation question next\n\
            - keep the session plan short and updated\n\
            - always include matrix_dimensions; set it to null for non-matrix widgets\n\
            - evaluation must be a non-null object with correctness, reasoning_quality, feedback_summary, and misconception when warranted\n\
            - provide exactly one next active question",
            self.snapshot.mode.label(),
            title,
            prompt,
            answer,
        )
    }

    fn build_close_prompt(&self) -> String {
        let evidence = if self.session_outcomes.is_empty() {
            "No graded outcomes were captured in this session.".to_string()
        } else {
            self.session_outcomes.join(" | ")
        };
        let due_reviews = self
            .database
            .list_due_reviews_for_course(&self.snapshot.course, 4)
            .unwrap_or_default()
            .into_iter()
            .map(|item| format!("{} due {}", item.concept_name, item.next_review_at))
            .collect::<Vec<_>>()
            .join("; ");
        let misconceptions = self
            .database
            .list_recent_repair_signals_for_course(&self.snapshot.course, 4)
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                format!(
                    "{} [{} | {}]: {}",
                    item.concept_name, item.error_type, item.status, item.description
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let unfinished = self
            .active_question_prompt()
            .unwrap_or_else(|| "No active question remained open.".to_string());

        format!(
            "Return JSON matching the provided schema only.\n\
            Close this StudyOS session with a concise recap.\n\
            Session mode: {}\n\
            Course: {}\n\
            Session minutes planned: {}\n\
            Evidence captured this session: {}\n\
            Due reviews now: {}\n\
            Recent misconceptions: {}\n\
            If the session stopped with unfinished work, carry it into unfinished_objectives: {}\n\
            Requirements:\n\
            - produce recap only, not a new question\n\
            - outcome_summary should describe demonstrated progress honestly\n\
            - demonstrated_concepts should be concepts that were actually shown, not wishful goals\n\
            - weak_concepts should name unresolved or fragile areas\n\
            - next_review_items should be actionable and short\n\
            - unfinished_objectives should be concrete restart points for the next launch",
            self.snapshot.mode.label(),
            self.snapshot.course,
            self.snapshot.time_remaining_minutes,
            evidence,
            if due_reviews.is_empty() {
                "none".to_string()
            } else {
                due_reviews
            },
            if misconceptions.is_empty() {
                "none".to_string()
            } else {
                misconceptions
            },
            unfinished,
        )
    }

    fn widget_submission_summary(&self) -> String {
        match self.active_widget() {
            Some(ResponseWidget::MatrixGrid(state)) => {
                let rows = state
                    .cells
                    .iter()
                    .map(|row| {
                        let values = row
                            .iter()
                            .map(|cell| {
                                if cell.trim().is_empty() {
                                    "·".to_string()
                                } else {
                                    cell.clone()
                                }
                            })
                            .collect::<Vec<_>>();
                        format!("[{}]", values.join(", "))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "widget: matrix_grid\nselected cell: ({}, {})\nsubmitted matrix:\n{}",
                    state.selected_row + 1,
                    state.selected_col + 1,
                    rows
                )
            }
            Some(ResponseWidget::WorkingAnswer(state)) => format!(
                "widget: working_answer\nworking:\n{}\n\nfinal_answer:\n{}",
                state.working, state.final_answer
            ),
            Some(ResponseWidget::StepList(state)) => format!(
                "widget: step_list\n{}",
                state
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(index, step)| format!("{}. {}", index + 1, step))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            Some(ResponseWidget::RetrievalResponse(state)) => {
                format!("widget: retrieval_response\n{}", state.response)
            }
            None => "widget: none\nNo active widget state.".to_string(),
        }
    }

    fn build_pending_attempt_context(&self) -> PendingAttemptContext {
        let question = self.snapshot.transcript.get(self.active_question_index);
        let (question_title, question_prompt, concept_tags, widget_kind) = match question {
            Some(ContentBlock::QuestionCard(card)) => (
                card.title.clone(),
                card.prompt.clone(),
                card.concept_tags.clone(),
                card.widget_kind,
            ),
            _ => (
                self.active_question_title(),
                self.active_question_prompt()
                    .unwrap_or_else(|| "No prompt recorded.".to_string()),
                Vec::new(),
                self.active_widget()
                    .map(ResponseWidget::kind)
                    .unwrap_or(ResponseWidgetKind::RetrievalResponse),
            ),
        };

        let latency_ms = self
            .question_presented_at
            .get(&self.active_question_index)
            .map(|started| started.elapsed().as_millis() as i64)
            .unwrap_or(0);

        PendingAttemptContext {
            question_title,
            question_prompt,
            concept_tags,
            widget_kind,
            student_answer: self.widget_submission_summary(),
            latency_ms,
        }
    }

    fn persist_evaluation(
        &mut self,
        context: &PendingAttemptContext,
        evaluation: &TutorEvaluation,
    ) -> Result<()> {
        let concept_id = self.resolve_concept_id(&context.concept_tags);
        let correctness = correctness_label(&evaluation.correctness);
        let reasoning_quality = reasoning_quality_label(&evaluation.reasoning_quality);
        let feedback_summary = evaluation.feedback_summary.trim().to_string();
        let prompt_hash = stable_hash(&context.question_prompt);
        let attempt = AttemptRecord {
            id: make_id("attempt", &context.question_prompt),
            session_id: self.current_session_id.clone(),
            concept_id: concept_id.clone(),
            question_type: widget_kind_label(context.widget_kind).to_string(),
            prompt_hash,
            student_answer: context.student_answer.clone(),
            correctness: correctness.to_string(),
            latency_ms: context.latency_ms,
            reasoning_quality: reasoning_quality.to_string(),
            feedback_summary: feedback_summary.clone(),
        };

        let misconception = evaluation
            .misconception
            .as_ref()
            .map(|item| MisconceptionInput {
                concept_id,
                error_type: error_type_label(&item.error_type).to_string(),
                description: item.description.clone(),
            });

        self.database
            .record_attempt(&attempt, misconception.as_ref())?;
        let outcome = evaluation
            .outcome_summary
            .clone()
            .unwrap_or_else(|| format!("{}: {}", context.question_title, feedback_summary));
        self.session_outcomes.push(outcome.clone());
        self.set_activity("Evidence", outcome, ActivityStatus::Healthy);
        Ok(())
    }

    fn handle_runtime_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::ThreadReady { thread_id } => {
                self.runtime_thread_id = Some(thread_id.clone());
                self.runtime_disconnected = false;
                self.set_activity(
                    "App-server",
                    format!("Thread ready: {thread_id}"),
                    ActivityStatus::Running,
                );
            }
            RuntimeEvent::ThreadStatusChanged { status } => {
                let activity_status = if status == "idle" {
                    ActivityStatus::Healthy
                } else {
                    ActivityStatus::Running
                };
                self.set_activity(
                    "App-server",
                    format!("Thread status changed: {status}"),
                    activity_status,
                );
            }
            RuntimeEvent::TurnStarted { turn_id } => {
                self.set_activity(
                    "App-server",
                    format!("Turn started: {turn_id}"),
                    ActivityStatus::Running,
                );
            }
            RuntimeEvent::TurnCompleted { turn_id, status } => {
                self.rebind_pending_structured_turn_if_needed(&turn_id);
                if status == "failed" {
                    self.pending_structured_turns.remove(&turn_id);
                    if self
                        .pending_recap_turn
                        .as_ref()
                        .map(|(pending_turn_id, _)| pending_turn_id == &turn_id)
                        .unwrap_or(false)
                    {
                        let fallback = self
                            .pending_recap_turn
                            .as_ref()
                            .map(|(_, pending)| pending.fallback.clone())
                            .unwrap_or_else(|| self.fallback_session_recap());
                        self.quit_state = QuitState::Ready(fallback);
                        self.pending_recap_turn = None;
                    }
                }
                self.set_activity(
                    "App-server",
                    format!("Turn completed with status: {status}"),
                    ActivityStatus::Healthy,
                );
            }
            RuntimeEvent::ItemStarted { turn_id, item } => {
                self.handle_runtime_item_started(&turn_id, item);
            }
            RuntimeEvent::AgentMessageDelta {
                turn_id,
                item_id,
                delta,
            } => {
                self.rebind_pending_structured_turn_if_needed(&turn_id);
                if self.pending_structured_turns.contains_key(&turn_id) {
                    self.structured_buffers
                        .entry(item_id)
                        .or_default()
                        .push_str(&delta);
                    trim_hash_map(
                        &mut self.structured_buffers,
                        MAX_PENDING_RUNTIME_MAP_ENTRIES,
                    );
                } else if let Some(index) = self.live_message_indices.get(&item_id).copied()
                    && let Some(ContentBlock::Paragraph(paragraph)) =
                        self.snapshot.transcript.get_mut(index)
                {
                    paragraph.text.push_str(&delta);
                }
            }
            RuntimeEvent::ItemCompleted { turn_id, item } => {
                self.handle_runtime_item_completed(&turn_id, item);
            }
            RuntimeEvent::McpServerStatusUpdated { name, status } => {
                self.set_activity(
                    &format!("MCP {name}"),
                    format!("startup status: {status}"),
                    if status == "ready" {
                        ActivityStatus::Healthy
                    } else {
                        ActivityStatus::Running
                    },
                );
            }
            RuntimeEvent::Error { message } => {
                self.set_activity("App-server", message.clone(), ActivityStatus::Idle);
                if message.contains("stderr") {
                    return;
                }
                self.push_block(ContentBlock::WarningBox(WarningBox {
                    title: "Runtime notice".to_string(),
                    body: message,
                }));
            }
            RuntimeEvent::Disconnected { message } => {
                self.runtime_ready = false;
                self.runtime_disconnected = true;
                self.pending_structured_turns.clear();
                if let Some((_, pending)) = self.pending_recap_turn.take() {
                    self.quit_state = QuitState::Ready(pending.fallback);
                }
                self.live_message_indices.clear();
                self.structured_buffers.clear();
                self.set_activity("App-server", message, ActivityStatus::Idle);
                if let Err(error) = self.persist_resume_state() {
                    self.push_block(ContentBlock::WarningBox(WarningBox {
                        title: "Resume save failed".to_string(),
                        body: error.to_string(),
                    }));
                }
                self.push_block(ContentBlock::WarningBox(WarningBox {
                    title: "Tutor runtime disconnected".to_string(),
                    body: "StudyOS saved your local resume state. Press Ctrl+R to respawn the runtime and continue.".to_string(),
                }));
            }
        }
    }

    fn handle_runtime_item_started(&mut self, turn_id: &str, item: Value) {
        self.rebind_pending_structured_turn_if_needed(turn_id);
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        let item_id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let is_recap_turn = self
            .pending_recap_turn
            .as_ref()
            .map(|(pending_turn_id, _)| pending_turn_id == turn_id)
            .unwrap_or(false);

        if item_type == "agentMessage"
            && !self.pending_structured_turns.contains_key(turn_id)
            && !is_recap_turn
        {
            let index = self.snapshot.transcript.len();
            self.snapshot
                .transcript
                .push(ContentBlock::Paragraph(studyos_core::ParagraphBlock {
                    text: "Tutor: ".to_string(),
                }));
            self.live_message_indices.insert(item_id, index);
            trim_hash_map(
                &mut self.live_message_indices,
                MAX_PENDING_RUNTIME_MAP_ENTRIES,
            );
            return;
        }

        if item_type == "agentMessage"
            && (self.pending_structured_turns.contains_key(turn_id) || is_recap_turn)
        {
            self.set_activity(
                if is_recap_turn {
                    "Session close"
                } else {
                    "Tutor turn"
                },
                if is_recap_turn {
                    "Streaming recap payload...".to_string()
                } else {
                    "Streaming structured tutor payload...".to_string()
                },
                ActivityStatus::Running,
            );
        }
    }

    fn handle_runtime_item_completed(&mut self, turn_id: &str, item: Value) {
        self.rebind_pending_structured_turn_if_needed(turn_id);
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        match item_type {
            "userMessage" => {
                let display_text = self
                    .pending_structured_turns
                    .get(turn_id)
                    .and_then(|pending| pending.display_user_text.clone())
                    .or_else(|| {
                        item.get("content")
                            .and_then(Value::as_array)
                            .and_then(|content| content.first())
                            .and_then(|entry| entry.get("text"))
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    });

                if let Some(text) = display_text {
                    self.push_block(ContentBlock::Paragraph(studyos_core::ParagraphBlock {
                        text: format!("You: {text}"),
                    }));
                }
            }
            "agentMessage" => {
                let item_id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let text = item
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                let is_recap_turn = self
                    .pending_recap_turn
                    .as_ref()
                    .map(|(pending_turn_id, _)| pending_turn_id == turn_id)
                    .unwrap_or(false);

                if self.pending_structured_turns.contains_key(turn_id) || is_recap_turn {
                    let structured_text = self.structured_buffers.remove(&item_id).unwrap_or(text);
                    if is_recap_turn {
                        self.apply_structured_close_payload(turn_id, &structured_text);
                    } else {
                        self.apply_structured_tutor_payload(turn_id, &structured_text);
                        self.pending_structured_turns.remove(turn_id);
                    }
                } else if let Some(index) = self.live_message_indices.remove(&item_id) {
                    if let Some(ContentBlock::Paragraph(paragraph)) =
                        self.snapshot.transcript.get_mut(index)
                    {
                        paragraph.text = format!("Tutor: {text}");
                    }
                } else {
                    self.push_block(ContentBlock::Paragraph(studyos_core::ParagraphBlock {
                        text: format!("Tutor: {text}"),
                    }));
                }
            }
            "plan" => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    self.push_block(ContentBlock::Paragraph(studyos_core::ParagraphBlock {
                        text: format!("Plan: {text}"),
                    }));
                }
            }
            _ => {}
        }
    }

    fn apply_structured_tutor_payload(&mut self, turn_id: &str, raw: &str) {
        match parse_structured_payload::<TutorTurnPayload>(raw) {
            Ok(payload) => {
                let evaluation_context = self
                    .pending_structured_turns
                    .get(turn_id)
                    .and_then(|pending| pending.attempt.clone());

                if let (Some(evaluation), Some(context)) =
                    (payload.evaluation.as_ref(), evaluation_context.as_ref())
                {
                    if let Err(error) = self.persist_evaluation(context, evaluation) {
                        self.push_block(ContentBlock::WarningBox(WarningBox {
                            title: "Evidence logging failed".to_string(),
                            body: error.to_string(),
                        }));
                    }
                }

                if let Some(mut plan) = payload.session_plan.clone() {
                    if plan.window.is_none() {
                        plan.window = self.snapshot.plan.window.clone();
                    }
                    self.snapshot.plan = plan;
                }

                if !self.runtime_bootstrap_applied {
                    self.snapshot.transcript.clear();
                    self.widget_states.clear();
                    self.runtime_bootstrap_applied = true;
                } else {
                    self.push_block(ContentBlock::Divider);
                }

                let blocks = payload.into_content_blocks();
                let previous_len = self.snapshot.transcript.len();
                for block in blocks {
                    self.push_block(block);
                }

                self.rebuild_widget_state_from(previous_len);
                if let Err(error) = self.refresh_snapshot_metrics() {
                    self.set_activity("SQLite", error.to_string(), ActivityStatus::Idle);
                }
                self.set_activity(
                    "Tutor turn",
                    "Structured tutor payload rendered successfully.".to_string(),
                    ActivityStatus::Healthy,
                );
                self.pending_structured_turns.remove(turn_id);
            }
            Err(error) => {
                if let Err(retry_error) = self.retry_structured_tutor_turn(turn_id, raw, &error) {
                    self.push_block(ContentBlock::WarningBox(WarningBox {
                        title: "Structured payload parse failed".to_string(),
                        body: format!(
                            "{error} | Retry failed: {retry_error} | Raw response: {raw}"
                        ),
                    }));
                    self.pending_structured_turns.remove(turn_id);
                }
            }
        }
    }

    fn apply_structured_close_payload(&mut self, turn_id: &str, raw: &str) {
        match parse_structured_payload::<TutorSessionClosePayload>(raw) {
            Ok(payload) => {
                self.quit_state = QuitState::Ready(payload.recap);
                self.pending_recap_turn = None;
                self.set_activity(
                    "Session close",
                    "Recap is ready. Press q or Enter to save and quit.".to_string(),
                    ActivityStatus::Healthy,
                );
            }
            Err(error) => {
                if let Err(retry_error) = self.retry_structured_close_turn(turn_id, raw, &error) {
                    let fallback = self
                        .pending_recap_turn
                        .as_ref()
                        .map(|(_, pending)| pending.fallback.clone())
                        .unwrap_or_else(|| self.fallback_session_recap());
                    self.quit_state = QuitState::Ready(fallback);
                    self.pending_recap_turn = None;
                    self.push_block(ContentBlock::WarningBox(WarningBox {
                        title: "Session recap parse failed".to_string(),
                        body: format!(
                            "{error} | Retry failed: {retry_error} | Falling back to local recap."
                        ),
                    }));
                }
            }
        }
    }

    fn retry_structured_tutor_turn(
        &mut self,
        turn_id: &str,
        raw: &str,
        error: &anyhow::Error,
    ) -> Result<()> {
        let Some(mut pending) = self.pending_structured_turns.remove(turn_id) else {
            return Err(anyhow!("missing pending tutor turn"));
        };
        if pending.retry_count >= 1 {
            return Err(anyhow!("retry budget exhausted"));
        }

        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow!("runtime unavailable for retry"))?;
        let thread_id = self
            .runtime_thread_id
            .clone()
            .ok_or_else(|| anyhow!("runtime thread unavailable for retry"))?;
        let cwd = self.paths.root_dir.parent().unwrap_or(&self.paths.root_dir);
        let retry_prompt = format!(
            "Your previous answer did not parse as JSON for the client. Re-emit the immediately previous tutor payload as raw JSON only, with no markdown fences or commentary.\nParse error: {error}\nPrevious raw response:\n{raw}"
        );
        pending.retry_count += 1;
        pending.display_user_text = None;
        let retry_turn_id =
            runtime.start_structured_turn(&thread_id, &retry_prompt, tutor_output_schema(), cwd)?;
        self.pending_structured_turns.insert(retry_turn_id, pending);
        trim_hash_map(
            &mut self.pending_structured_turns,
            MAX_PENDING_RUNTIME_MAP_ENTRIES,
        );
        self.set_activity(
            "Runtime diagnostics",
            "{\"app.runtime.payload_parse_failure\":1,\"kind\":\"tutor\"}".to_string(),
            ActivityStatus::Idle,
        );
        Ok(())
    }

    fn retry_structured_close_turn(
        &mut self,
        turn_id: &str,
        raw: &str,
        error: &anyhow::Error,
    ) -> Result<()> {
        let Some((_, mut pending)) = self.pending_recap_turn.take() else {
            return Err(anyhow!("missing pending recap turn"));
        };
        if pending.retry_count >= 1 {
            return Err(anyhow!("retry budget exhausted"));
        }

        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow!("runtime unavailable for recap retry"))?;
        let thread_id = self
            .runtime_thread_id
            .clone()
            .ok_or_else(|| anyhow!("runtime thread unavailable for recap retry"))?;
        let cwd = self.paths.root_dir.parent().unwrap_or(&self.paths.root_dir);
        let retry_prompt = format!(
            "Your previous close-session recap did not parse as JSON for the client. Re-emit the immediately previous recap payload as raw JSON only, with no markdown fences or commentary.\nParse error: {error}\nPrevious raw response:\n{raw}"
        );
        pending.retry_count += 1;
        let retry_turn_id = runtime.start_structured_turn(
            &thread_id,
            &retry_prompt,
            tutor_close_output_schema(),
            cwd,
        )?;
        self.pending_recap_turn = Some((retry_turn_id, pending));
        self.set_activity(
            "Runtime diagnostics",
            "{\"app.runtime.payload_parse_failure\":1,\"kind\":\"recap\"}".to_string(),
            ActivityStatus::Idle,
        );
        self.quit_state = QuitState::Preparing(
            self.pending_recap_turn
                .as_ref()
                .map(|(_, pending)| pending.fallback.clone())
                .unwrap_or_else(|| self.fallback_session_recap()),
        );
        let _ = turn_id;
        Ok(())
    }

    fn rebuild_widget_state_from(&mut self, start_index: usize) {
        for index in start_index..self.snapshot.transcript.len() {
            if let Some(ContentBlock::QuestionCard(card)) = self.snapshot.transcript.get(index) {
                self.widget_states
                    .insert(index, widget_state_for_question(card));
                self.question_presented_at.insert(index, Instant::now());
                trim_hash_map(
                    &mut self.question_presented_at,
                    MAX_PENDING_RUNTIME_MAP_ENTRIES,
                );
                self.active_question_index = index;
            }
        }
    }

    fn push_block(&mut self, block: ContentBlock) {
        self.snapshot.transcript.push(block);
    }

    fn rebind_pending_structured_turn_if_needed(&mut self, observed_turn_id: &str) {
        if self.pending_structured_turns.contains_key(observed_turn_id)
            || self.pending_structured_turns.len() != 1
        {
            return;
        }

        let Some(previous_turn_id) = self.pending_structured_turns.keys().next().cloned() else {
            return;
        };
        if previous_turn_id == observed_turn_id {
            return;
        }

        if let Some(pending) = self.pending_structured_turns.remove(&previous_turn_id) {
            self.pending_structured_turns
                .insert(observed_turn_id.to_string(), pending);
            self.set_activity(
                "Tutor turn",
                format!(
                    "Rebound pending structured turn {previous_turn_id} to observed runtime turn {observed_turn_id}."
                ),
                ActivityStatus::Running,
            );
        }
    }

    fn set_activity(&mut self, name: &str, detail: String, status: ActivityStatus) {
        if let Some(item) = self
            .snapshot
            .activity
            .iter_mut()
            .find(|item| item.name == name)
        {
            item.detail = detail;
            item.status = status;
            return;
        }

        self.snapshot.activity.push(ActivityItem {
            name: name.to_string(),
            detail,
            status,
        });
    }

    fn question_indices_from(transcript: &[ContentBlock]) -> Vec<usize> {
        transcript
            .iter()
            .enumerate()
            .filter_map(|(index, block)| {
                matches!(block, ContentBlock::QuestionCard(_)).then_some(index)
            })
            .collect()
    }

    fn advance_question(&mut self, direction: isize) {
        let indices = self.question_indices();
        if indices.is_empty() {
            return;
        }

        let current = indices
            .iter()
            .position(|index| *index == self.active_question_index)
            .unwrap_or(0);

        let next = if direction.is_negative() {
            current.checked_sub(1).unwrap_or(indices.len() - 1)
        } else {
            (current + 1) % indices.len()
        };

        self.active_question_index = indices[next];
    }

    fn handle_transcript_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                self.transcript_scroll = self.transcript_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                self.transcript_scroll = self.transcript_scroll.saturating_add(1);
            }
            KeyCode::Char('g') => {
                self.transcript_scroll = 0;
            }
            KeyCode::Char('G') => {
                self.transcript_scroll = u16::MAX / 4;
            }
            _ => {}
        }
    }

    fn handle_panel_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Right | KeyCode::Down => {
                self.snapshot.panel_tab = next_panel_tab(self.snapshot.panel_tab);
            }
            KeyCode::Left | KeyCode::Up => {
                self.snapshot.panel_tab = previous_panel_tab(self.snapshot.panel_tab);
            }
            _ => {}
        }
    }

    fn handle_scratchpad_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Backspace => {
                self.snapshot.scratchpad.pop();
            }
            KeyCode::Enter => self.snapshot.scratchpad.push('\n'),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.snapshot.scratchpad.push(c);
            }
            _ => {}
        }
    }

    fn handle_widget_key(&mut self, key: KeyEvent) {
        let Some(widget) = self.active_widget_mut() else {
            return;
        };

        match widget {
            ResponseWidget::MatrixGrid(state) => handle_matrix_widget(state, key),
            ResponseWidget::WorkingAnswer(state) => handle_working_widget(state, key),
            ResponseWidget::StepList(state) => handle_step_list_widget(state, key),
            ResponseWidget::RetrievalResponse(state) => handle_retrieval_widget(state, key),
        }
    }

    pub fn developer_instructions(&self) -> String {
        "You are the StudyOS tutor runtime. Prioritize retrieval before explanation, ask for mathematical reasoning rather than spoon-feeding, and stay concise. When the client provides an output schema, obey it strictly. Prefer one active question at a time and choose widget kinds that match the task precisely.".to_string()
    }

    fn resolve_concept_id(&self, concept_tags: &[String]) -> String {
        if let Ok(Some(concept_id)) = self.database.resolve_concept_id(concept_tags) {
            return concept_id;
        }

        if let Some(first) = concept_tags.first() {
            return normalize_identifier(first);
        }

        "general_study_skill".to_string()
    }

    fn apply_resume_state(&mut self, resume: ResumeStateRecord) {
        if resume.active_course != self.snapshot.course {
            self.set_activity(
                "Resume",
                format!(
                    "Skipped draft/UI resume from `{}` while starting `{}`.",
                    resume.active_course, self.snapshot.course
                ),
                ActivityStatus::Healthy,
            );
            return;
        }

        self.runtime_thread_id = resume.runtime_thread_id;
        self.snapshot.mode = SessionMode::from_label(&resume.active_mode);
        self.snapshot.panel_tab = PanelTab::from_label(&resume.focused_panel);

        if !resume.scratchpad_text.trim().is_empty() {
            self.snapshot.scratchpad = resume.scratchpad_text;
        }

        if let Some(active_question_id) = resume.active_question_id {
            if let Ok(index) = active_question_id.parse::<usize>() {
                if self.widget_states.contains_key(&index) {
                    self.active_question_index = index;
                }
            }
        }

        if !resume.draft_payload.trim().is_empty() {
            match parse_resume_draft_payload(&resume.draft_payload) {
                Ok(Some(widget)) => {
                    self.widget_states
                        .insert(self.active_question_index, widget);
                }
                Ok(None) => {}
                Err(error) => {
                    self.push_block(ContentBlock::WarningBox(WarningBox {
                        title: "Draft restore failed".to_string(),
                        body: error.to_string(),
                    }));
                }
            }
        }
    }
}

const RESUME_DRAFT_SCHEMA_VERSION: u32 = 1;
const MAX_PENDING_RUNTIME_MAP_ENTRIES: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResumeDraftPayload {
    schema_version: u32,
    widget: Option<ResponseWidget>,
}

fn build_resume_draft_payload(widget: Option<&ResponseWidget>) -> Result<String> {
    Ok(serde_json::to_string(&ResumeDraftPayload {
        schema_version: RESUME_DRAFT_SCHEMA_VERSION,
        widget: widget.cloned(),
    })?)
}

fn parse_resume_draft_payload(raw: &str) -> Result<Option<ResponseWidget>> {
    let payload = serde_json::from_str::<ResumeDraftPayload>(raw)?;
    if payload.schema_version > RESUME_DRAFT_SCHEMA_VERSION {
        return Err(anyhow!(
            "resume draft schema {} is newer than this client supports ({RESUME_DRAFT_SCHEMA_VERSION})",
            payload.schema_version
        ));
    }

    Ok(payload.widget)
}

fn parse_structured_payload<T: DeserializeOwned>(raw: &str) -> Result<T> {
    let trimmed = strip_json_wrappers(raw);
    Ok(serde_json::from_str::<T>(&trimmed)?)
}

fn strip_json_wrappers(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(fenced) = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
    {
        let unfenced = fenced.trim();
        return unfenced
            .strip_suffix("```")
            .unwrap_or(unfenced)
            .trim()
            .to_string();
    }

    let start = trimmed.find('{').unwrap_or(0);
    let end = trimmed
        .rfind('}')
        .map(|index| index + 1)
        .unwrap_or(trimmed.len());
    trimmed[start..end].trim().to_string()
}

fn trim_hash_map<K, V>(map: &mut HashMap<K, V>, max_len: usize)
where
    K: Clone + Eq + Hash,
{
    while map.len() > max_len {
        let Some(key) = map.keys().next().cloned() else {
            break;
        };
        map.remove(&key);
    }
}

fn widget_state_for_question(card: &QuestionCard) -> ResponseWidget {
    match card.widget_kind {
        ResponseWidgetKind::MatrixGrid => {
            let dimensions = card
                .matrix_dimensions
                .unwrap_or(MatrixDimensions { rows: 2, cols: 2 });
            ResponseWidget::MatrixGrid(MatrixGridState::new(dimensions.rows, dimensions.cols))
        }
        ResponseWidgetKind::WorkingAnswer => {
            ResponseWidget::WorkingAnswer(WorkingAnswerState::default())
        }
        ResponseWidgetKind::StepList => ResponseWidget::StepList(StepListState {
            steps: vec!["".to_string()],
            selected_step: 0,
        }),
        ResponseWidgetKind::RetrievalResponse => {
            ResponseWidget::RetrievalResponse(RetrievalResponseState::default())
        }
    }
}

fn next_panel_tab(current: PanelTab) -> PanelTab {
    match current {
        PanelTab::SessionPlan => PanelTab::DueReviews,
        PanelTab::DueReviews => PanelTab::Deadlines,
        PanelTab::Deadlines => PanelTab::Misconceptions,
        PanelTab::Misconceptions => PanelTab::Scratchpad,
        PanelTab::Scratchpad => PanelTab::Activity,
        PanelTab::Activity => PanelTab::RuntimeLog,
        PanelTab::RuntimeLog => PanelTab::SessionPlan,
    }
}

fn previous_panel_tab(current: PanelTab) -> PanelTab {
    match current {
        PanelTab::SessionPlan => PanelTab::RuntimeLog,
        PanelTab::DueReviews => PanelTab::SessionPlan,
        PanelTab::Deadlines => PanelTab::DueReviews,
        PanelTab::Misconceptions => PanelTab::Deadlines,
        PanelTab::Scratchpad => PanelTab::Misconceptions,
        PanelTab::Activity => PanelTab::Scratchpad,
        PanelTab::RuntimeLog => PanelTab::Activity,
    }
}

fn handle_matrix_widget(state: &mut MatrixGridState, key: KeyEvent) {
    match key.code {
        KeyCode::Left => state.selected_col = state.selected_col.saturating_sub(1),
        KeyCode::Right => {
            state.selected_col =
                (state.selected_col + 1).min(state.dimensions.cols.saturating_sub(1));
        }
        KeyCode::Up => state.selected_row = state.selected_row.saturating_sub(1),
        KeyCode::Down => {
            state.selected_row =
                (state.selected_row + 1).min(state.dimensions.rows.saturating_sub(1));
        }
        KeyCode::Tab => {
            state.selected_col = (state.selected_col + 1) % state.dimensions.cols.max(1);
        }
        KeyCode::Backspace => {
            state.cells[state.selected_row][state.selected_col].pop();
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.cells[state.selected_row][state.selected_col].push(c);
        }
        _ => {}
    }
}

fn handle_working_widget(state: &mut WorkingAnswerState, key: KeyEvent) {
    match key.code {
        KeyCode::Up => state.active_field = WorkingAnswerField::Working,
        KeyCode::Down => state.active_field = WorkingAnswerField::FinalAnswer,
        KeyCode::Backspace => match state.active_field {
            WorkingAnswerField::Working => {
                state.working.pop();
            }
            WorkingAnswerField::FinalAnswer => {
                state.final_answer.pop();
            }
        },
        KeyCode::Enter => {
            if matches!(state.active_field, WorkingAnswerField::Working) {
                state.working.push('\n');
            }
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            match state.active_field {
                WorkingAnswerField::Working => state.working.push(c),
                WorkingAnswerField::FinalAnswer => state.final_answer.push(c),
            }
        }
        _ => {}
    }
}

fn handle_step_list_widget(state: &mut StepListState, key: KeyEvent) {
    match key.code {
        KeyCode::Up => {
            state.selected_step = state.selected_step.saturating_sub(1);
        }
        KeyCode::Down => {
            state.selected_step =
                (state.selected_step + 1).min(state.steps.len().saturating_sub(1));
        }
        KeyCode::Enter => {
            let insert_at = (state.selected_step + 1).min(state.steps.len());
            state.steps.insert(insert_at, String::new());
            state.selected_step = insert_at;
        }
        KeyCode::Backspace => {
            if let Some(current) = state.steps.get_mut(state.selected_step) {
                if !current.is_empty() {
                    current.pop();
                } else if state.steps.len() > 1 {
                    state.steps.remove(state.selected_step);
                    state.selected_step = state.selected_step.saturating_sub(1);
                }
            }
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(current) = state.steps.get_mut(state.selected_step) {
                current.push(c);
            }
        }
        _ => {}
    }
}

fn handle_retrieval_widget(state: &mut RetrievalResponseState, key: KeyEvent) {
    match key.code {
        KeyCode::Backspace => {
            state.response.pop();
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.response.push(c);
        }
        _ => {}
    }
}

pub fn widget_validation_warning(widget: &ResponseWidget) -> Option<WarningBox> {
    match widget {
        ResponseWidget::MatrixGrid(state) => {
            let filled = state
                .cells
                .iter()
                .flat_map(|row| row.iter())
                .any(|cell| !cell.trim().is_empty());
            (!filled).then(|| WarningBox {
                title: "Blank Attempt".to_string(),
                body: "Attempt-first mode expects at least one filled cell before reveal."
                    .to_string(),
            })
        }
        ResponseWidget::WorkingAnswer(state) => (state.working.trim().is_empty()
            && !state.final_answer.trim().is_empty())
        .then(|| WarningBox {
            title: "Method Missing".to_string(),
            body: "This question expects working as well as a final answer.".to_string(),
        }),
        ResponseWidget::StepList(state) => state
            .steps
            .iter()
            .all(|step| step.trim().is_empty())
            .then(|| WarningBox {
                title: "No Reasoning Logged".to_string(),
                body: "Add at least one derivation or reasoning step before submission."
                    .to_string(),
            }),
        ResponseWidget::RetrievalResponse(state) => {
            state.response.trim().is_empty().then(|| WarningBox {
                title: "No Retrieval Attempt".to_string(),
                body: "Write a short answer before asking for help or reveal.".to_string(),
            })
        }
    }
}

fn widget_kind_label(kind: ResponseWidgetKind) -> &'static str {
    match kind {
        ResponseWidgetKind::MatrixGrid => "matrix_grid",
        ResponseWidgetKind::WorkingAnswer => "working_answer",
        ResponseWidgetKind::StepList => "step_list",
        ResponseWidgetKind::RetrievalResponse => "retrieval_response",
    }
}

fn correctness_label(correctness: &TutorCorrectness) -> &'static str {
    match correctness {
        TutorCorrectness::Correct => "correct",
        TutorCorrectness::Partial => "partial",
        TutorCorrectness::Incorrect => "incorrect",
    }
}

fn reasoning_quality_label(reasoning_quality: &TutorReasoningQuality) -> &'static str {
    match reasoning_quality {
        TutorReasoningQuality::Strong => "strong",
        TutorReasoningQuality::Adequate => "adequate",
        TutorReasoningQuality::Weak => "weak",
        TutorReasoningQuality::Missing => "missing",
    }
}

fn error_type_label(error_type: &TutorErrorType) -> &'static str {
    match error_type {
        TutorErrorType::ConceptualMisunderstanding => "conceptual_misunderstanding",
        TutorErrorType::ProceduralSlip => "procedural_slip",
        TutorErrorType::NotationError => "notation_error",
        TutorErrorType::ArithmeticError => "arithmetic_error",
        TutorErrorType::IncompleteJustification => "incomplete_justification",
        TutorErrorType::WeakReasoning => "weak_reasoning",
    }
}

fn stable_hash(text: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn make_id(prefix: &str, seed: &str) -> String {
    format!(
        "{prefix}-{}",
        stable_hash(&format!("{seed}-{:?}", Instant::now()))
    )
}

fn normalize_identifier(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

pub fn tutor_output_schema() -> Value {
    tutor_output_schema_with_evaluation(false)
}

pub fn tutor_submission_output_schema() -> Value {
    tutor_output_schema_with_evaluation(true)
}

fn tutor_output_schema_with_evaluation(require_evaluation: bool) -> Value {
    let evaluation_schema = if require_evaluation {
        json!({
            "type": "object",
            "properties": {
                "correctness": {
                    "type": "string",
                    "enum": ["correct", "partial", "incorrect"]
                },
                "reasoning_quality": {
                    "type": "string",
                    "enum": ["strong", "adequate", "weak", "missing"]
                },
                "feedback_summary": { "type": "string" },
                "misconception": {
                    "type": ["object", "null"],
                    "properties": {
                        "error_type": {
                            "type": "string",
                            "enum": [
                                "conceptual_misunderstanding",
                                "procedural_slip",
                                "notation_error",
                                "arithmetic_error",
                                "incomplete_justification",
                                "weak_reasoning"
                            ]
                        },
                        "description": { "type": "string" }
                    },
                    "required": ["error_type", "description"],
                    "additionalProperties": false
                },
                "outcome_summary": { "type": ["string", "null"] }
            },
            "required": [
                "correctness",
                "reasoning_quality",
                "feedback_summary",
                "misconception",
                "outcome_summary"
            ],
            "additionalProperties": false
        })
    } else {
        json!({
            "type": ["object", "null"],
            "properties": {
                "correctness": {
                    "type": "string",
                    "enum": ["correct", "partial", "incorrect"]
                },
                "reasoning_quality": {
                    "type": "string",
                    "enum": ["strong", "adequate", "weak", "missing"]
                },
                "feedback_summary": { "type": "string" },
                "misconception": {
                    "type": ["object", "null"],
                    "properties": {
                        "error_type": {
                            "type": "string",
                            "enum": [
                                "conceptual_misunderstanding",
                                "procedural_slip",
                                "notation_error",
                                "arithmetic_error",
                                "incomplete_justification",
                                "weak_reasoning"
                            ]
                        },
                        "description": { "type": "string" }
                    },
                    "required": ["error_type", "description"],
                    "additionalProperties": false
                },
                "outcome_summary": { "type": ["string", "null"] }
            },
            "required": [
                "correctness",
                "reasoning_quality",
                "feedback_summary",
                "misconception",
                "outcome_summary"
            ],
            "additionalProperties": false
        })
    };

    json!({
        "type": "object",
        "properties": {
            "session_plan": {
                "type": "object",
                "properties": {
                    "recommended_duration_minutes": { "type": "integer" },
                    "window": {
                        "type": ["object", "null"],
                        "properties": {
                            "start": { "type": "string" },
                            "duration_minutes": { "type": "integer", "minimum": 1 },
                            "source": {
                                "type": "string",
                                "enum": ["timetable_gap", "before_deadline", "evening_block"]
                            }
                        },
                        "required": ["start", "duration_minutes", "source"],
                        "additionalProperties": false
                    },
                    "why_now": { "type": "string" },
                    "warm_up_questions": { "type": "array", "items": { "type": "string" } },
                    "core_targets": { "type": "array", "items": { "type": "string" } },
                    "stretch_target": { "type": ["string", "null"] }
                },
                "required": [
                    "recommended_duration_minutes",
                    "window",
                    "why_now",
                    "warm_up_questions",
                    "core_targets",
                    "stretch_target"
                ],
                "additionalProperties": false
            },
            "teaching_blocks": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "anyOf": [
                        tutor_paragraph_block_schema(),
                        tutor_hint_block_schema(),
                        tutor_warning_block_schema(),
                        tutor_math_block_schema(),
                        tutor_matrix_block_schema(),
                        tutor_bullet_list_block_schema(),
                        tutor_recap_block_schema()
                    ]
                }
            },
            "question": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "prompt": { "type": "string" },
                    "concept_tags": { "type": "array", "items": { "type": "string" } },
                    "widget_kind": {
                        "type": "string",
                        "enum": ["matrix_grid", "working_answer", "step_list", "retrieval_response"]
                    },
                    "matrix_dimensions": {
                        "type": ["object", "null"],
                        "properties": {
                            "rows": { "type": "integer", "minimum": 1, "maximum": 6 },
                            "cols": { "type": "integer", "minimum": 1, "maximum": 6 }
                        },
                        "required": ["rows", "cols"],
                        "additionalProperties": false
                    }
                },
                "required": ["title", "prompt", "concept_tags", "widget_kind", "matrix_dimensions"],
                "additionalProperties": false
            },
            "evaluation": evaluation_schema
        },
        "required": ["session_plan", "teaching_blocks", "question", "evaluation"],
        "additionalProperties": false
    })
}

fn tutor_paragraph_block_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "const": "paragraph" },
            "text": { "type": "string" }
        },
        "required": ["type", "text"],
        "additionalProperties": false
    })
}

fn tutor_hint_block_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "const": "hint" },
            "title": { "type": "string" },
            "body": { "type": "string" }
        },
        "required": ["type", "title", "body"],
        "additionalProperties": false
    })
}

fn tutor_warning_block_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "const": "warning" },
            "title": { "type": "string" },
            "body": { "type": "string" }
        },
        "required": ["type", "title", "body"],
        "additionalProperties": false
    })
}

fn tutor_math_block_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "const": "math" },
            "latex": { "type": "string" },
            "fallback_text": { "type": "string" }
        },
        "required": ["type", "latex", "fallback_text"],
        "additionalProperties": false
    })
}

fn tutor_matrix_block_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "const": "matrix" },
            "title": { "type": "string" },
            "rows": {
                "type": "array",
                "items": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        },
        "required": ["type", "title", "rows"],
        "additionalProperties": false
    })
}

fn tutor_bullet_list_block_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "const": "bullet_list" },
            "items": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["type", "items"],
        "additionalProperties": false
    })
}

fn tutor_recap_block_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "const": "recap" },
            "title": { "type": "string" },
            "highlights": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["type", "title", "highlights"],
        "additionalProperties": false
    })
}

pub fn tutor_close_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "recap": {
                "type": "object",
                "properties": {
                    "outcome_summary": { "type": "string" },
                    "demonstrated_concepts": { "type": "array", "items": { "type": "string" } },
                    "weak_concepts": { "type": "array", "items": { "type": "string" } },
                    "next_review_items": { "type": "array", "items": { "type": "string" } },
                    "unfinished_objectives": { "type": "array", "items": { "type": "string" } }
                },
                "required": [
                    "outcome_summary",
                    "demonstrated_concepts",
                    "weak_concepts",
                    "next_review_items",
                    "unfinished_objectives"
                ],
                "additionalProperties": false
            }
        },
        "required": ["recap"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        sync::Arc,
        sync::{
            Mutex,
            atomic::{AtomicU64, Ordering},
        },
    };

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use serde_json::json;
    use studyos_core::{
        AppConfig, AppDatabase, AppPaths, AppSnapshot, BootstrapStudyContext, ContentBlock,
        LocalContext, MaterialEntry, MatrixDimensions, ResponseWidget, ResponseWidgetKind,
        SessionPlanSummary, TutorBlock, TutorCorrectness, TutorErrorType, TutorEvaluation,
        TutorMisconception, TutorQuestion, TutorReasoningQuality, TutorTurnPayload,
        WorkingAnswerField,
    };

    use crate::{AppServerTransport, RuntimeEvent};

    use super::{
        App, AppAction, AppBootstrap, TutorPendingTurn, build_resume_draft_payload,
        parse_resume_draft_payload,
    };

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_data_root() -> std::path::PathBuf {
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "studyos-app-test-{}-{}-{}",
            std::process::id(),
            counter,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap_or_else(|err| panic!("temp dir create failed: {err}"));
        path
    }

    struct NoopTransport;

    impl AppServerTransport for NoopTransport {
        fn initialize(&self) -> anyhow::Result<()> {
            Ok(())
        }

        fn start_thread(
            &self,
            _cwd: &std::path::Path,
            _developer_instructions: &str,
        ) -> anyhow::Result<String> {
            Ok("thread-test".to_string())
        }

        fn resume_thread(&self, thread_id: &str, _cwd: &std::path::Path) -> anyhow::Result<String> {
            Ok(thread_id.to_string())
        }

        fn start_structured_turn(
            &self,
            _thread_id: &str,
            _prompt: &str,
            _output_schema: serde_json::Value,
            _cwd: &std::path::Path,
        ) -> anyhow::Result<String> {
            Ok("turn-noop".to_string())
        }

        fn poll_events(&self) -> Vec<RuntimeEvent> {
            Vec::new()
        }

        fn runtime_log_lines(&self) -> Vec<String> {
            Vec::new()
        }
    }

    #[derive(Default)]
    struct TransportStats {
        initialize_calls: usize,
        start_thread_calls: usize,
        resume_thread_calls: usize,
    }

    struct CountingTransport {
        stats: Arc<Mutex<TransportStats>>,
    }

    impl AppServerTransport for CountingTransport {
        fn initialize(&self) -> anyhow::Result<()> {
            if let Ok(mut stats) = self.stats.lock() {
                stats.initialize_calls += 1;
            }
            Ok(())
        }

        fn start_thread(
            &self,
            _cwd: &std::path::Path,
            _developer_instructions: &str,
        ) -> anyhow::Result<String> {
            if let Ok(mut stats) = self.stats.lock() {
                stats.start_thread_calls += 1;
            }
            Ok("thread-counting".to_string())
        }

        fn resume_thread(&self, thread_id: &str, _cwd: &std::path::Path) -> anyhow::Result<String> {
            if let Ok(mut stats) = self.stats.lock() {
                stats.resume_thread_calls += 1;
            }
            Ok(thread_id.to_string())
        }

        fn start_structured_turn(
            &self,
            _thread_id: &str,
            _prompt: &str,
            _output_schema: serde_json::Value,
            _cwd: &std::path::Path,
        ) -> anyhow::Result<String> {
            Ok("turn-counting".to_string())
        }

        fn poll_events(&self) -> Vec<RuntimeEvent> {
            Vec::new()
        }

        fn runtime_log_lines(&self) -> Vec<String> {
            Vec::new()
        }
    }

    #[test]
    fn structured_payload_persists_attempt_evidence() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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

        let mut app = App::new(AppBootstrap {
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

        if let Some(ResponseWidget::MatrixGrid(state)) = app.active_widget_mut() {
            state.cells[0][0] = "1".to_string();
        }

        let attempt = app.build_pending_attempt_context();
        app.pending_structured_turns.insert(
            "turn-test".to_string(),
            TutorPendingTurn {
                display_user_text: Some("Submitted answer".to_string()),
                attempt: Some(attempt),
                retry_count: 0,
            },
        );

        let payload = TutorTurnPayload {
            session_plan: Some(SessionPlanSummary {
                recommended_duration_minutes: 10,
                window: None,
                why_now: "Repair matrix product recall.".to_string(),
                warm_up_questions: vec!["When is AB defined?".to_string()],
                core_targets: vec!["Matrix multiplication dimensions".to_string()],
                stretch_target: None,
            }),
            teaching_blocks: vec![TutorBlock::Paragraph {
                text: "You mixed up the dimensions.".to_string(),
            }],
            question: Some(TutorQuestion {
                title: "Dimension Repair".to_string(),
                prompt: "State the inner-dimension rule.".to_string(),
                concept_tags: vec!["matrix_multiplication".to_string()],
                widget_kind: ResponseWidgetKind::RetrievalResponse,
                matrix_dimensions: None,
            }),
            evaluation: Some(TutorEvaluation {
                correctness: TutorCorrectness::Incorrect,
                reasoning_quality: TutorReasoningQuality::Weak,
                feedback_summary: "You entered only one cell and did not complete the product."
                    .to_string(),
                misconception: Some(TutorMisconception {
                    error_type: TutorErrorType::ConceptualMisunderstanding,
                    description: "Confused what product the grid was asking for.".to_string(),
                }),
                outcome_summary: Some("Matrix product recall needs repair.".to_string()),
            }),
        };

        let raw = serde_json::to_string(&payload)
            .unwrap_or_else(|err| panic!("payload serialization failed: {err}"));
        app.apply_structured_tutor_payload("turn-test", &raw);

        let stats = app
            .database
            .stats()
            .unwrap_or_else(|err| panic!("stats query failed: {err}"));
        let repair_signals = app
            .database
            .list_recent_repair_signals(5)
            .unwrap_or_else(|err| panic!("repair signal query failed: {err}"));
        let misconceptions = app
            .database
            .list_recent_misconceptions(5)
            .unwrap_or_else(|err| panic!("misconception query failed: {err}"));

        assert_eq!(stats.total_attempts, 1);
        assert_eq!(repair_signals.len(), 1);
        assert_eq!(repair_signals[0].status, "candidate".to_string());
        assert_eq!(misconceptions.len(), 0);
        assert_eq!(
            repair_signals[0].error_type,
            "conceptual_misunderstanding".to_string()
        );

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn quit_review_opens_before_exit_and_can_be_cancelled() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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

        let mut app = App::new(AppBootstrap {
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

        app.handle_key(KeyEvent::from(KeyCode::Char('q')));

        assert!(!app.should_quit);
        assert_eq!(app.current_mode_label(), "Recap");
        let recap = app
            .quit_recap_preview()
            .unwrap_or_else(|| panic!("quit recap preview should be open"));
        assert!(
            recap
                .outcome_summary
                .contains("Session ended before any graded evidence")
        );
        assert_eq!(recap.unfinished_objectives.len(), 1);

        app.handle_key(KeyEvent::from(KeyCode::Esc));

        assert!(app.quit_recap_preview().is_none());
        assert_eq!(app.current_mode_label(), "Study");
        assert!(!app.should_quit);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn structured_matrix_question_uses_declared_dimensions() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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

        let mut app = App::new(AppBootstrap {
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

        let payload = TutorTurnPayload {
            session_plan: Some(SessionPlanSummary {
                recommended_duration_minutes: 10,
                window: None,
                why_now: "Practice a rectangular matrix product.".to_string(),
                warm_up_questions: vec!["What is the shape of the output?".to_string()],
                core_targets: vec!["Matrix multiplication".to_string()],
                stretch_target: None,
            }),
            teaching_blocks: vec![TutorBlock::Paragraph {
                text: "Fill the product directly in the target grid.".to_string(),
            }],
            question: Some(TutorQuestion {
                title: "Rectangular Product".to_string(),
                prompt: "Enter the 2 by 3 output matrix.".to_string(),
                concept_tags: vec!["matrix_multiplication".to_string()],
                widget_kind: ResponseWidgetKind::MatrixGrid,
                matrix_dimensions: Some(MatrixDimensions { rows: 2, cols: 3 }),
            }),
            evaluation: None,
        };

        let raw = serde_json::to_string(&payload)
            .unwrap_or_else(|err| panic!("payload serialization failed: {err}"));
        app.apply_structured_tutor_payload("turn-open", &raw);

        let widget = app
            .active_widget()
            .unwrap_or_else(|| panic!("matrix widget should be active"));
        match widget {
            ResponseWidget::MatrixGrid(state) => {
                assert_eq!(state.dimensions.rows, 2);
                assert_eq!(state.dimensions.cols, 3);
                assert_eq!(state.cells.len(), 2);
                assert_eq!(state.cells[0].len(), 3);
            }
            other => panic!("expected matrix widget, got {other:?}"),
        }

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn working_answer_widget_switches_between_fields() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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

        let mut app = App::new(AppBootstrap {
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

        app.active_question_index = app
            .snapshot
            .transcript
            .iter()
            .enumerate()
            .find_map(|(index, block)| match block {
                ContentBlock::QuestionCard(card)
                    if matches!(card.widget_kind, ResponseWidgetKind::WorkingAnswer) =>
                {
                    Some(index)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("working-answer question should exist in bootstrap"));

        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        app.handle_key(KeyEvent::from(KeyCode::Down));
        app.handle_key(KeyEvent::from(KeyCode::Char('7')));

        let widget = app
            .active_widget()
            .unwrap_or_else(|| panic!("working-answer widget should be active"));
        match widget {
            ResponseWidget::WorkingAnswer(state) => {
                assert_eq!(state.working, "x");
                assert_eq!(state.final_answer, "7");
                assert_eq!(state.active_field, WorkingAnswerField::FinalAnswer);
            }
            other => panic!("expected working-answer widget, got {other:?}"),
        }

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn opening_prompt_includes_relevant_local_materials() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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

        let app = App::new(AppBootstrap {
            database,
            paths: paths.clone(),
            config,
            stats,
            local_context: LocalContext {
                materials: vec![MaterialEntry {
                    id: "matrix-sheet".to_string(),
                    title: "Matrix Multiplication Worksheet".to_string(),
                    course: "Matrix Algebra & Linear Models".to_string(),
                    topic_tags: vec!["matrix_multiplication".to_string()],
                    material_type: "worksheet".to_string(),
                    path: "materials/linear/matrix.pdf".to_string(),
                    snippet: "Compute products and explain undefined cases.".to_string(),
                    source_hash: String::new(),
                    source_modified_at: String::new(),
                }],
                ..LocalContext::default()
            },
            snapshot,
            runtime: None,
            runtime_factory: None,
            runtime_error: None,
            resume_state: None,
        });

        let prompt = app.build_opening_prompt();
        assert!(prompt.contains("Relevant local materials"));
        assert!(prompt.contains("Matrix Multiplication Worksheet"));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn resume_state_round_trips_per_widget_variant() {
        let widgets = vec![
            ResponseWidget::MatrixGrid(studyos_core::MatrixGridState::new(2, 2)),
            ResponseWidget::WorkingAnswer(studyos_core::WorkingAnswerState {
                working: "AB".to_string(),
                final_answer: "C".to_string(),
                active_field: WorkingAnswerField::FinalAnswer,
            }),
            ResponseWidget::StepList(studyos_core::StepListState {
                steps: vec!["step 1".to_string(), "step 2".to_string()],
                selected_step: 1,
            }),
            ResponseWidget::RetrievalResponse(studyos_core::RetrievalResponseState {
                response: "variance".to_string(),
            }),
        ];

        for widget in widgets {
            let raw = build_resume_draft_payload(Some(&widget))
                .unwrap_or_else(|err| panic!("draft payload build failed: {err}"));
            let parsed = parse_resume_draft_payload(&raw)
                .unwrap_or_else(|err| panic!("draft payload parse failed: {err}"));
            assert_eq!(parsed, Some(widget));
        }
    }

    #[test]
    fn widget_draft_survives_restart() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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

        let mut app = App::new(AppBootstrap {
            database,
            paths: paths.clone(),
            config: config.clone(),
            stats: stats.clone(),
            local_context: LocalContext::default(),
            snapshot,
            runtime: None,
            runtime_factory: None,
            runtime_error: None,
            resume_state: None,
        });
        if let Some(ResponseWidget::MatrixGrid(state)) = app.active_widget_mut() {
            state.cells[0][0] = "5".to_string();
        }
        app.persist_resume_state()
            .unwrap_or_else(|err| panic!("resume persist failed: {err}"));

        let database = AppDatabase::open(&paths.database_path)
            .unwrap_or_else(|err| panic!("database reopen failed: {err}"));
        let resume_state = database
            .load_resume_state()
            .unwrap_or_else(|err| panic!("resume load failed: {err}"));
        let stats = database
            .stats()
            .unwrap_or_else(|err| panic!("stats query failed: {err}"));
        let snapshot = AppSnapshot::bootstrap(&config, &stats, &BootstrapStudyContext::default());
        let restarted = App::new(AppBootstrap {
            database,
            paths,
            config,
            stats,
            local_context: LocalContext::default(),
            snapshot,
            runtime: None,
            runtime_factory: None,
            runtime_error: None,
            resume_state,
        });

        match restarted.active_widget() {
            Some(ResponseWidget::MatrixGrid(state)) => {
                assert_eq!(state.cells[0][0], "5");
            }
            other => panic!("expected restored matrix widget, got {other:?}"),
        }

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn recap_ready_event_does_not_block_key_handler() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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
        let mut app = App::new(AppBootstrap {
            database,
            paths,
            config,
            stats,
            local_context: LocalContext::default(),
            snapshot,
            runtime: Some(Arc::new(NoopTransport)),
            runtime_factory: Some(Arc::new(|| Ok(Arc::new(NoopTransport)))),
            runtime_error: None,
            resume_state: None,
        });
        app.runtime_thread_id = Some("thread-test".to_string());

        let started = std::time::Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Char('q')));
        assert!(
            started.elapsed().as_millis() < 50,
            "quit key handler should not block on recap generation"
        );
        assert!(app.quit_recap_is_preparing());
    }

    #[test]
    fn runtime_bootstrap_hides_placeholder_widget_until_live_payload_arrives() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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
        let mut app = App::new(AppBootstrap {
            database,
            paths: paths.clone(),
            config,
            stats,
            local_context: LocalContext::default(),
            snapshot,
            runtime: Some(Arc::new(NoopTransport)),
            runtime_factory: Some(Arc::new(|| Ok(Arc::new(NoopTransport)))),
            runtime_error: None,
            resume_state: None,
        });

        assert!(app.active_widget().is_none());
        assert!(app.question_indices().is_empty());
        assert!(
            app.status_line()
                .contains("Waiting for live tutor question")
        );

        let payload = TutorTurnPayload {
            session_plan: Some(SessionPlanSummary {
                recommended_duration_minutes: 10,
                window: None,
                why_now: "Runtime bootstrapped.".to_string(),
                warm_up_questions: vec!["When is AB defined?".to_string()],
                core_targets: vec!["Matrix multiplication".to_string()],
                stretch_target: None,
            }),
            teaching_blocks: vec![TutorBlock::Paragraph {
                text: "Live tutor question arrived.".to_string(),
            }],
            question: Some(TutorQuestion {
                title: "Live Question".to_string(),
                prompt: "Fill the 2 by 2 product.".to_string(),
                concept_tags: vec!["matrix_multiplication".to_string()],
                widget_kind: ResponseWidgetKind::MatrixGrid,
                matrix_dimensions: Some(MatrixDimensions { rows: 2, cols: 2 }),
            }),
            evaluation: None,
        };

        let raw = serde_json::to_string(&payload)
            .unwrap_or_else(|err| panic!("payload serialization failed: {err}"));
        app.apply_structured_tutor_payload("turn-live", &raw);

        assert!(app.active_widget().is_some());
        assert!(!app.question_indices().is_empty());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn disconnect_persists_resume_and_blocks_submission() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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
        let mut app = App::new(AppBootstrap {
            database,
            paths: paths.clone(),
            config: config.clone(),
            stats,
            local_context: LocalContext::default(),
            snapshot,
            runtime: Some(Arc::new(NoopTransport)),
            runtime_factory: Some(Arc::new(|| Ok(Arc::new(NoopTransport)))),
            runtime_error: None,
            resume_state: None,
        });
        app.runtime_ready = true;
        app.runtime_bootstrap_applied = true;
        app.runtime_thread_id = Some("thread-test".to_string());
        if let Some(ResponseWidget::MatrixGrid(state)) = app.active_widget_mut() {
            state.cells[0][0] = "9".to_string();
        }

        app.handle_runtime_event(RuntimeEvent::Disconnected {
            message: "simulated disconnect".to_string(),
        });

        let resume = app
            .database
            .load_resume_state()
            .unwrap_or_else(|err| panic!("resume state load failed: {err}"))
            .unwrap_or_else(|| panic!("resume state should be persisted on disconnect"));
        assert!(resume.draft_payload.contains("\"schema_version\":1"));

        app.execute_action(AppAction::SubmitCurrentAnswer);
        let warning = app
            .snapshot
            .transcript
            .iter()
            .rev()
            .find_map(|block| match block {
                ContentBlock::WarningBox(warning) => Some(warning),
                _ => None,
            })
            .unwrap_or_else(|| panic!("disconnect warning should be visible"));
        assert!(warning.body.contains("Ctrl+R"));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn ctrl_r_reconnects_runtime_after_disconnect() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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
        let transport_stats = Arc::new(Mutex::new(TransportStats::default()));
        let factory_stats = Arc::clone(&transport_stats);
        let runtime_factory = Arc::new(move || {
            Ok(Arc::new(CountingTransport {
                stats: Arc::clone(&factory_stats),
            }) as Arc<dyn AppServerTransport>)
        });
        let runtime =
            runtime_factory().unwrap_or_else(|err| panic!("runtime factory should succeed: {err}"));

        let mut app = App::new(AppBootstrap {
            database,
            paths: paths.clone(),
            config,
            stats,
            local_context: LocalContext::default(),
            snapshot,
            runtime: Some(runtime),
            runtime_factory: Some(runtime_factory),
            runtime_error: None,
            resume_state: None,
        });
        app.runtime_thread_id = Some("thread-test".to_string());
        app.handle_runtime_event(RuntimeEvent::Disconnected {
            message: "simulated disconnect".to_string(),
        });

        let action = app
            .handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL))
            .unwrap_or_else(|| panic!("Ctrl+R should return reconnect action"));
        app.execute_action(action);

        let stats = transport_stats
            .lock()
            .unwrap_or_else(|_| panic!("transport stats lock should not be poisoned"));
        assert_eq!(stats.initialize_calls, 1);
        assert_eq!(stats.resume_thread_calls, 1);
        assert_eq!(stats.start_thread_calls, 0);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn mismatched_runtime_turn_id_still_persists_structured_payload() {
        let base = temp_data_root();
        let paths = AppPaths::discover(&base);
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
        let mut app = App::new(AppBootstrap {
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
        if let Some(ResponseWidget::MatrixGrid(state)) = app.active_widget_mut() {
            state.cells[0][0] = "2".to_string();
        }

        let attempt = app.build_pending_attempt_context();
        app.pending_structured_turns.insert(
            "turn-requested".to_string(),
            TutorPendingTurn {
                display_user_text: Some("Submitted answer".to_string()),
                attempt: Some(attempt),
                retry_count: 0,
            },
        );

        let payload = TutorTurnPayload {
            session_plan: Some(SessionPlanSummary {
                recommended_duration_minutes: 10,
                window: None,
                why_now: "Repair the row-by-column method.".to_string(),
                warm_up_questions: vec!["Which row and column define one entry?".to_string()],
                core_targets: vec!["Matrix multiplication".to_string()],
                stretch_target: None,
            }),
            teaching_blocks: vec![TutorBlock::Paragraph {
                text: "Use one row and one column per entry.".to_string(),
            }],
            question: Some(TutorQuestion {
                title: "Repair".to_string(),
                prompt: "Compute the first entry only.".to_string(),
                concept_tags: vec!["matrix_multiplication".to_string()],
                widget_kind: ResponseWidgetKind::WorkingAnswer,
                matrix_dimensions: None,
            }),
            evaluation: Some(TutorEvaluation {
                correctness: TutorCorrectness::Incorrect,
                reasoning_quality: TutorReasoningQuality::Weak,
                feedback_summary: "The method needs repair.".to_string(),
                misconception: Some(TutorMisconception {
                    error_type: TutorErrorType::ConceptualMisunderstanding,
                    description: "Mixed up entrywise and row-by-column multiplication.".to_string(),
                }),
                outcome_summary: Some("Repair the multiplication rule.".to_string()),
            }),
        };

        let payload_raw = serde_json::to_string(&payload)
            .unwrap_or_else(|err| panic!("payload serialization failed: {err}"));
        app.handle_runtime_event(RuntimeEvent::AgentMessageDelta {
            turn_id: "turn-observed".to_string(),
            item_id: "item-1".to_string(),
            delta: payload_raw.clone(),
        });
        app.handle_runtime_event(RuntimeEvent::ItemCompleted {
            turn_id: "turn-observed".to_string(),
            item: json!({
                "type": "agentMessage",
                "id": "item-1",
                "text": payload_raw,
            }),
        });

        let stats = app
            .database
            .stats()
            .unwrap_or_else(|err| panic!("stats query failed: {err}"));
        assert_eq!(stats.total_attempts, 1);
        assert!(app.pending_structured_turns.is_empty());

        let _ = fs::remove_dir_all(base);
    }
}
