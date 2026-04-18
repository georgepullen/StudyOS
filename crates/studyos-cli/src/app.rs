use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};
use studyos_core::{
    ActivityItem, ActivityStatus, AppConfig, AppDatabase, AppPaths, AppSnapshot, AppStats,
    ContentBlock, LocalContext, MatrixGridState, PanelTab, ResponseWidget, ResponseWidgetKind,
    ResumeStateRecord, RetrievalResponseState, SessionMode, StepListState, TutorTurnPayload,
    WarningBox, WorkingAnswerState,
};

use crate::runtime::{AppServerClient, RuntimeEvent};

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
}

pub struct AppBootstrap {
    pub database: AppDatabase,
    pub paths: AppPaths,
    pub config: AppConfig,
    pub stats: AppStats,
    pub local_context: LocalContext,
    pub snapshot: AppSnapshot,
    pub runtime: Option<AppServerClient>,
    pub runtime_error: Option<String>,
    pub resume_state: Option<ResumeStateRecord>,
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
    runtime: Option<AppServerClient>,
    runtime_thread_id: Option<String>,
    runtime_ready: bool,
    runtime_bootstrap_applied: bool,
    pending_structured_turns: HashSet<String>,
    live_message_indices: HashMap<String, usize>,
    structured_buffers: HashMap<String, String>,
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
            runtime_error,
            resume_state,
        } = bootstrap;

        let question_indices = Self::question_indices_from(&snapshot.transcript);
        let active_question_index = *question_indices.first().unwrap_or(&0);

        let widget_states = question_indices
            .iter()
            .filter_map(|index| {
                let ContentBlock::QuestionCard(card) = &snapshot.transcript[*index] else {
                    return None;
                };

                Some((*index, default_widget_state(card.widget_kind)))
            })
            .collect();

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
            runtime_thread_id: None,
            runtime_ready: false,
            runtime_bootstrap_applied: false,
            pending_structured_turns: HashSet::new(),
            live_message_indices: HashMap::new(),
            structured_buffers: HashMap::new(),
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
            self.pending_structured_turns.insert(turn_id);
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

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
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
            KeyCode::Char('1') => {
                self.snapshot.panel_tab = PanelTab::SessionPlan;
                return None;
            }
            KeyCode::Char('2') => {
                self.snapshot.panel_tab = PanelTab::DueReviews;
                return None;
            }
            KeyCode::Char('3') => {
                self.snapshot.panel_tab = PanelTab::Deadlines;
                return None;
            }
            KeyCode::Char('4') => {
                self.snapshot.panel_tab = PanelTab::Misconceptions;
                return None;
            }
            KeyCode::Char('5') => {
                self.snapshot.panel_tab = PanelTab::Scratchpad;
                return None;
            }
            KeyCode::Char('6') => {
                self.snapshot.panel_tab = PanelTab::Activity;
                return None;
            }
            KeyCode::Char(']') => {
                self.advance_question(1);
                return None;
            }
            KeyCode::Char('[') => {
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

    pub fn active_widget(&self) -> Option<&ResponseWidget> {
        self.widget_states.get(&self.active_question_index)
    }

    pub fn active_widget_mut(&mut self) -> Option<&mut ResponseWidget> {
        self.widget_states.get_mut(&self.active_question_index)
    }

    pub fn persist_resume_state(&self) -> Result<()> {
        let draft_payload = self
            .active_widget()
            .map(toml::to_string)
            .transpose()?
            .unwrap_or_default();

        let record = ResumeStateRecord {
            session_id: "study-session".to_string(),
            runtime_thread_id: self.runtime_thread_id.clone(),
            active_mode: self.snapshot.mode.label().to_string(),
            active_question_id: Some(self.active_question_index.to_string()),
            focused_panel: self.snapshot.panel_tab.label().to_string(),
            draft_payload,
            scratchpad_text: self.snapshot.scratchpad.clone(),
        };

        self.database.save_resume_state(&record)
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
        Self::question_indices_from(&self.snapshot.transcript)
    }

    pub fn status_line(&self) -> String {
        let runtime_label = if self.runtime_ready {
            "App-server connected"
        } else if self.runtime.is_some() {
            "App-server starting"
        } else {
            "Local fallback"
        };

        format!(
            "Focus: {} | Panel: {} | Strictness: {:?} | Sessions: {} | Attempts: {} | Runtime thread: {} | {}",
            self.focus.label(),
            self.snapshot.panel_tab.label(),
            self.config.strictness,
            self.stats.total_sessions,
            self.stats.total_attempts,
            self.runtime_thread_id.as_deref().unwrap_or("not-started"),
            runtime_label,
        )
    }

    pub fn misconceptions_summary(&self) -> Vec<String> {
        let mut lines = vec![
            "No misconception history yet; future sessions will persist repeated errors here."
                .to_string(),
            "Determinant-zero confusion should escalate into prerequisite repair mode.".to_string(),
        ];

        for course in self.local_context.courses.courses.iter().take(2) {
            lines.push(format!("Loaded course graph: {}", course.title));
        }

        lines
    }

    pub fn review_summary(&self) -> Vec<String> {
        vec![
            format!("Due review count: {}", self.snapshot.metrics.due_reviews),
            "Warm-up queue starts with matrix dimension rules.".to_string(),
            "Transfer prompts should follow quick correct answers.".to_string(),
        ]
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
        }

        lines
    }

    fn execute_action_inner(&mut self, action: AppAction) -> Result<()> {
        match action {
            AppAction::SubmitCurrentAnswer => self.submit_current_answer(),
        }
    }

    fn submit_current_answer(&mut self) -> Result<()> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow!("app-server runtime is unavailable"))?;
        let thread_id = self
            .runtime_thread_id
            .clone()
            .ok_or_else(|| anyhow!("no runtime thread is active"))?;
        let prompt = self.build_submission_prompt();
        let cwd = self.paths.root_dir.parent().unwrap_or(&self.paths.root_dir);
        let turn_id =
            runtime.start_structured_turn(&thread_id, &prompt, tutor_output_schema(), cwd)?;
        self.pending_structured_turns.insert(turn_id);
        self.set_activity(
            "App-server",
            "Submitted structured student answer for grading and next-step planning.".to_string(),
            ActivityStatus::Running,
        );
        Ok(())
    }

    fn build_opening_prompt(&self) -> String {
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

        format!(
            "You are the StudyOS tutor runtime. Return JSON matching the provided schema only.\n\
            Build a concise opening study step for a real student session.\n\
            Course focus: {course}\n\
            Available session minutes: {minutes}\n\
            Due review count: {due_reviews}\n\
            Upcoming deadlines: {deadlines}\n\
            Local courses loaded: {course_names}\n\
            Strictness: {:?}\n\
            Requirements:\n\
            - retrieval first, not explanation first\n\
            - one short session plan\n\
            - 1 to 3 teaching blocks max before the question\n\
            - exactly one active question using one of the supported widgets\n\
            - prefer matrix_grid for matrix algebra warmups when appropriate\n\
            - keep the tone direct and anti-passive",
            self.config.strictness,
            course = self.snapshot.course,
            minutes = self.snapshot.time_remaining_minutes,
            due_reviews = self.snapshot.metrics.due_reviews,
            deadlines = deadlines,
            course_names = course_names,
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
            - provide exactly one next active question",
            self.snapshot.mode.label(),
            title,
            prompt,
            answer,
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

    fn handle_runtime_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::ThreadReady { thread_id } => {
                self.runtime_thread_id = Some(thread_id.clone());
                self.set_activity(
                    "App-server",
                    format!("Thread ready: {}", thread_id),
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
                self.pending_structured_turns.remove(&turn_id);
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
                if self.pending_structured_turns.contains(&turn_id) {
                    self.structured_buffers
                        .entry(item_id)
                        .or_default()
                        .push_str(&delta);
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
            RuntimeEvent::Disconnected => {
                self.set_activity(
                    "App-server",
                    "Codex app-server disconnected; the shell has fallen back to local state."
                        .to_string(),
                    ActivityStatus::Idle,
                );
            }
        }
    }

    fn handle_runtime_item_started(&mut self, turn_id: &str, item: Value) {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        let item_id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if item_type == "agentMessage" && !self.pending_structured_turns.contains(turn_id) {
            let index = self.snapshot.transcript.len();
            self.snapshot
                .transcript
                .push(ContentBlock::Paragraph(studyos_core::ParagraphBlock {
                    text: "Tutor: ".to_string(),
                }));
            self.live_message_indices.insert(item_id, index);
            return;
        }

        if item_type == "agentMessage" && self.pending_structured_turns.contains(turn_id) {
            self.set_activity(
                "Tutor turn",
                "Streaming structured tutor payload...".to_string(),
                ActivityStatus::Running,
            );
        }
    }

    fn handle_runtime_item_completed(&mut self, turn_id: &str, item: Value) {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        match item_type {
            "userMessage" => {
                if let Some(text) = item
                    .get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| content.first())
                    .and_then(|entry| entry.get("text"))
                    .and_then(Value::as_str)
                {
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

                if self.pending_structured_turns.contains(turn_id) {
                    let structured_text = self.structured_buffers.remove(&item_id).unwrap_or(text);
                    self.apply_structured_tutor_payload(&structured_text);
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

    fn apply_structured_tutor_payload(&mut self, raw: &str) {
        match serde_json::from_str::<TutorTurnPayload>(raw) {
            Ok(payload) => {
                if let Some(plan) = payload.session_plan.clone() {
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
                self.set_activity(
                    "Tutor turn",
                    "Structured tutor payload rendered successfully.".to_string(),
                    ActivityStatus::Healthy,
                );
            }
            Err(error) => {
                self.push_block(ContentBlock::WarningBox(WarningBox {
                    title: "Structured payload parse failed".to_string(),
                    body: format!("{} | Raw response: {}", error, raw),
                }));
            }
        }
    }

    fn rebuild_widget_state_from(&mut self, start_index: usize) {
        for index in start_index..self.snapshot.transcript.len() {
            if let Some(ContentBlock::QuestionCard(card)) = self.snapshot.transcript.get(index) {
                self.widget_states
                    .insert(index, default_widget_state(card.widget_kind));
                self.active_question_index = index;
            }
        }
    }

    fn push_block(&mut self, block: ContentBlock) {
        self.snapshot.transcript.push(block);
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

    fn developer_instructions(&self) -> String {
        "You are the StudyOS tutor runtime. Prioritize retrieval before explanation, ask for mathematical reasoning rather than spoon-feeding, and stay concise. When the client provides an output schema, obey it strictly. Prefer one active question at a time and choose widget kinds that match the task precisely.".to_string()
    }

    fn apply_resume_state(&mut self, resume: ResumeStateRecord) {
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
            if let Ok(widget) = toml::from_str::<ResponseWidget>(&resume.draft_payload) {
                self.widget_states
                    .insert(self.active_question_index, widget);
            }
        }
    }
}

fn default_widget_state(kind: ResponseWidgetKind) -> ResponseWidget {
    match kind {
        ResponseWidgetKind::MatrixGrid => ResponseWidget::MatrixGrid(MatrixGridState::new(2, 2)),
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
        PanelTab::Activity => PanelTab::SessionPlan,
    }
}

fn previous_panel_tab(current: PanelTab) -> PanelTab {
    match current {
        PanelTab::SessionPlan => PanelTab::Activity,
        PanelTab::DueReviews => PanelTab::SessionPlan,
        PanelTab::Deadlines => PanelTab::DueReviews,
        PanelTab::Misconceptions => PanelTab::Deadlines,
        PanelTab::Scratchpad => PanelTab::Misconceptions,
        PanelTab::Activity => PanelTab::Scratchpad,
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
        KeyCode::Backspace => {
            if !state.final_answer.is_empty() {
                state.final_answer.pop();
            } else {
                state.working.pop();
            }
        }
        KeyCode::Enter => state.working.push('\n'),
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            state.final_answer.push(c);
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.working.push(c);
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

fn tutor_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "session_plan": {
                "type": "object",
                "properties": {
                    "recommended_duration_minutes": { "type": "integer" },
                    "why_now": { "type": "string" },
                    "warm_up_questions": { "type": "array", "items": { "type": "string" } },
                    "core_targets": { "type": "array", "items": { "type": "string" } },
                    "stretch_target": { "type": ["string", "null"] }
                },
                "required": [
                    "recommended_duration_minutes",
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
                    }
                },
                "required": ["title", "prompt", "concept_tags", "widget_kind"],
                "additionalProperties": false
            }
        },
        "required": ["session_plan", "teaching_blocks", "question"],
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
