use std::collections::HashMap;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use studyos_core::{
    ActivityStatus, AppConfig, AppDatabase, AppPaths, AppSnapshot, AppStats, ContentBlock,
    LocalContext, MatrixGridState, PanelTab, ResponseWidget, ResponseWidgetKind, ResumeStateRecord,
    RetrievalResponseState, SessionMode, StepListState, WarningBox, WorkingAnswerState,
};

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
}

impl App {
    pub fn new(
        paths: AppPaths,
        config: AppConfig,
        stats: AppStats,
        snapshot: AppSnapshot,
        database: AppDatabase,
        local_context: LocalContext,
        resume_state: Option<ResumeStateRecord>,
    ) -> Self {
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
        };

        if let Some(resume) = resume_state {
            app.apply_resume_state(resume);
        }

        app.snapshot.activity.push(studyos_core::ActivityItem {
            name: "Resume".to_string(),
            detail: "Resume state is now loaded from local SQLite when available.".to_string(),
            status: ActivityStatus::Healthy,
        });
        app.snapshot.activity.push(studyos_core::ActivityItem {
            name: "Local context".to_string(),
            detail: format!(
                "{} deadlines, {} materials, {} course files discovered.",
                app.local_context.deadlines.len(),
                app.local_context.materials.len(),
                app.local_context.courses.courses.len()
            ),
            status: ActivityStatus::Healthy,
        });

        app
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                return;
            }
            KeyCode::Tab => {
                self.focus = self.focus.next();
                return;
            }
            KeyCode::Char('1') => {
                self.snapshot.panel_tab = PanelTab::SessionPlan;
                return;
            }
            KeyCode::Char('2') => {
                self.snapshot.panel_tab = PanelTab::DueReviews;
                return;
            }
            KeyCode::Char('3') => {
                self.snapshot.panel_tab = PanelTab::Deadlines;
                return;
            }
            KeyCode::Char('4') => {
                self.snapshot.panel_tab = PanelTab::Misconceptions;
                return;
            }
            KeyCode::Char('5') => {
                self.snapshot.panel_tab = PanelTab::Scratchpad;
                return;
            }
            KeyCode::Char('6') => {
                self.snapshot.panel_tab = PanelTab::Activity;
                return;
            }
            KeyCode::Char(']') => {
                self.advance_question(1);
                return;
            }
            KeyCode::Char('[') => {
                self.advance_question(-1);
                return;
            }
            _ => {}
        }

        match self.focus {
            FocusRegion::Transcript => self.handle_transcript_key(key),
            FocusRegion::Panel => self.handle_panel_key(key),
            FocusRegion::Widget => self.handle_widget_key(key),
            FocusRegion::Scratchpad => self.handle_scratchpad_key(key),
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
            session_id: "bootstrap-shell".to_string(),
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

    pub fn question_indices(&self) -> Vec<usize> {
        Self::question_indices_from(&self.snapshot.transcript)
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

    pub fn status_line(&self) -> String {
        let runtime = self
            .snapshot
            .activity
            .iter()
            .find(|item| item.name == "App-server")
            .map(|item| match item.status {
                ActivityStatus::Idle => "Local bootstrap mode",
                ActivityStatus::Running => "App-server active",
                ActivityStatus::Healthy => "App-server healthy",
            })
            .unwrap_or("Runtime unknown");

        format!(
            "Focus: {} | Panel: {} | Strictness: {:?} | Sessions: {} | Attempts: {} | Data: {} | Runtime: {}",
            self.focus.label(),
            self.snapshot.panel_tab.label(),
            self.config.strictness,
            self.stats.total_sessions,
            self.stats.total_attempts,
            self.paths.root_dir.display(),
            runtime
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

    fn apply_resume_state(&mut self, resume: ResumeStateRecord) {
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
