use serde::{Deserialize, Serialize};

use crate::{
    AppConfig, AppStats,
    content::{
        ContentBlock, HeadingBlock, HintCard, MathBlock, MatrixBlock, ParagraphBlock, QuestionCard,
        RecapBox, WarningBox,
    },
    widgets::{MatrixGridState, ResponseWidget, ResponseWidgetKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Study,
    Review,
    Drill,
    Recap,
}

impl SessionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Study => "Study",
            Self::Review => "Review",
            Self::Drill => "Drill",
            Self::Recap => "Recap",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "Review" => Self::Review,
            "Drill" => Self::Drill,
            "Recap" => Self::Recap,
            _ => Self::Study,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTab {
    SessionPlan,
    DueReviews,
    Deadlines,
    Misconceptions,
    Scratchpad,
    Activity,
}

impl PanelTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::SessionPlan => "Plan",
            Self::DueReviews => "Reviews",
            Self::Deadlines => "Deadlines",
            Self::Misconceptions => "Misconceptions",
            Self::Scratchpad => "Scratchpad",
            Self::Activity => "Activity",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "Reviews" => Self::DueReviews,
            "Deadlines" => Self::Deadlines,
            "Misconceptions" => Self::Misconceptions,
            "Scratchpad" => Self::Scratchpad,
            "Activity" => Self::Activity,
            _ => Self::SessionPlan,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineUrgency {
    Calm,
    Upcoming,
    Urgent,
}

impl DeadlineUrgency {
    pub fn label(self) -> &'static str {
        match self {
            Self::Calm => "Calm",
            Self::Upcoming => "Upcoming",
            Self::Urgent => "Urgent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionPlanSummary {
    pub recommended_duration_minutes: u16,
    pub why_now: String,
    pub warm_up_questions: Vec<String>,
    pub core_targets: Vec<String>,
    pub stretch_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityStatus {
    Idle,
    Running,
    Healthy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityItem {
    pub name: String,
    pub detail: String,
    pub status: ActivityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMetrics {
    pub due_reviews: usize,
    pub upcoming_deadlines: usize,
    pub attempts_logged: usize,
    pub sessions_logged: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingHint {
    pub key: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSnapshot {
    pub mode: SessionMode,
    pub course: String,
    pub time_remaining_minutes: u16,
    pub panel_tab: PanelTab,
    pub deadline_urgency: DeadlineUrgency,
    pub metrics: SessionMetrics,
    pub plan: SessionPlanSummary,
    pub transcript: Vec<ContentBlock>,
    pub widget: ResponseWidget,
    pub scratchpad: String,
    pub activity: Vec<ActivityItem>,
    pub keybindings: Vec<KeybindingHint>,
}

impl AppSnapshot {
    pub fn bootstrap(config: &AppConfig, stats: &AppStats) -> Self {
        let urgency = if stats.upcoming_deadlines > 0 {
            DeadlineUrgency::Upcoming
        } else {
            DeadlineUrgency::Calm
        };

        Self {
            mode: SessionMode::Study,
            course: config.default_course.clone(),
            time_remaining_minutes: config.default_session_minutes,
            panel_tab: PanelTab::SessionPlan,
            deadline_urgency: urgency,
            metrics: SessionMetrics {
                due_reviews: stats.due_reviews,
                upcoming_deadlines: stats.upcoming_deadlines,
                attempts_logged: stats.total_attempts,
                sessions_logged: stats.total_sessions,
            },
            plan: SessionPlanSummary {
                recommended_duration_minutes: config.default_session_minutes,
                why_now: if stats.due_reviews > 0 {
                    "You have due retrieval items queued, so the session should start by repairing memory before new material.".to_string()
                } else {
                    "No due queue yet, so this session should establish a baseline with retrieval-first warmups and one structured matrix task.".to_string()
                },
                warm_up_questions: vec![
                    "State the condition for matrix multiplication dimensions.".to_string(),
                    "Explain what a singular matrix tells you about invertibility.".to_string(),
                ],
                core_targets: vec![
                    "Matrix multiplication fluency".to_string(),
                    "Reasoning about invertibility".to_string(),
                ],
                stretch_target: Some("Connect determinant zero to linear dependence.".to_string()),
            },
            transcript: bootstrap_transcript(),
            widget: ResponseWidget::MatrixGrid(MatrixGridState::new(2, 2)),
            scratchpad: "Use this scratchpad for rough working that should not be submitted.\n- jot down row operations\n- note shortcuts\n- park reminders".to_string(),
            activity: vec![
                ActivityItem {
                    name: "SQLite".to_string(),
                    detail: "Local study memory opened and schema verified.".to_string(),
                    status: ActivityStatus::Healthy,
                },
                ActivityItem {
                    name: "App-server".to_string(),
                    detail: "Codex app-server bootstrap is pending; live tutor content will replace this shell once the first turn completes.".to_string(),
                    status: ActivityStatus::Running,
                },
                ActivityItem {
                    name: "Renderer".to_string(),
                    detail: "Structured transcript blocks ready for TUI rendering.".to_string(),
                    status: ActivityStatus::Healthy,
                },
            ],
            keybindings: vec![
                KeybindingHint {
                    key: "q",
                    description: "quit safely",
                },
                KeybindingHint {
                    key: "tab",
                    description: "cycle focus",
                },
                KeybindingHint {
                    key: "1-6",
                    description: "switch panel tab",
                },
                KeybindingHint {
                    key: "?",
                    description: "toggle help",
                },
            ],
        }
    }
}

fn bootstrap_transcript() -> Vec<ContentBlock> {
    vec![
        ContentBlock::Heading(HeadingBlock {
            level: 1,
            text: "StudyOS V1 Shell".to_string(),
        }),
        ContentBlock::Paragraph(ParagraphBlock {
            text: "This bootstrap view appears while the local tutor runtime connects. Once the first structured Codex turn returns, it replaces this placeholder with live study content.".to_string(),
        }),
        ContentBlock::WarningBox(WarningBox {
            title: "Attempt-First Default".to_string(),
            body: "Full worked solutions should not appear before the student makes a genuine attempt.".to_string(),
        }),
        ContentBlock::MathBlock(MathBlock {
            latex: "AB = \\begin{bmatrix}1 & 2 \\\\ 3 & 4\\end{bmatrix}\\begin{bmatrix}x \\\\ y\\end{bmatrix}".to_string(),
            fallback_text: "AB = [[1, 2], [3, 4]] [x, y]^T".to_string(),
        }),
        ContentBlock::QuestionCard(QuestionCard {
            title: "Warm-up Matrix Retrieval".to_string(),
            prompt: "Fill the 2x2 result for multiplying A = [[1, 2], [3, 4]] by B = [[2, 0], [1, 2]]. Use the matrix grid below.".to_string(),
            concept_tags: vec![
                "matrix_multiplication".to_string(),
                "structured_input".to_string(),
            ],
            widget_kind: ResponseWidgetKind::MatrixGrid,
        }),
        ContentBlock::HintCard(HintCard {
            title: "Hint".to_string(),
            body: "Work cell by cell. Each output entry is a row-by-column dot product.".to_string(),
        }),
        ContentBlock::MatrixBlock(MatrixBlock {
            title: "Reference Matrix A".to_string(),
            rows: vec![
                vec!["1".to_string(), "2".to_string()],
                vec!["3".to_string(), "4".to_string()],
            ],
        }),
        ContentBlock::RecapBox(RecapBox {
            title: "End-of-session target".to_string(),
            highlights: vec![
                "log at least one structured matrix attempt".to_string(),
                "persist the session to SQLite".to_string(),
                "prepare for due-review scheduling".to_string(),
            ],
        }),
        ContentBlock::Paragraph(ParagraphBlock {
            text: "Use the panel tabs to inspect the session plan, deadlines, or scratchpad while the shell is still local-only.".to_string(),
        }),
        ContentBlock::QuestionCard(QuestionCard {
            title: "Interpretation Check".to_string(),
            prompt: "In one sentence, explain what it means if det(A) = 0 for a square matrix A.".to_string(),
            concept_tags: vec!["determinant".to_string(), "invertibility".to_string()],
            widget_kind: ResponseWidgetKind::RetrievalResponse,
        }),
        ContentBlock::QuestionCard(QuestionCard {
            title: "Method-mark Prompt".to_string(),
            prompt: "Outline your working for solving a 2x2 linear system, then give the final solution vector.".to_string(),
            concept_tags: vec!["linear_systems".to_string()],
            widget_kind: ResponseWidgetKind::WorkingAnswer,
        }),
        ContentBlock::Paragraph(ParagraphBlock {
            text: "Later iterations will swap these bootstrap cards for app-server generated session plans, question cards, grading feedback, and recaps.".to_string(),
        }),
    ]
}
