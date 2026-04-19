use serde::{Deserialize, Serialize};

use crate::{
    AppConfig, AppStats,
    content::{
        ContentBlock, HeadingBlock, HintCard, MathBlock, MatrixBlock, ParagraphBlock, QuestionCard,
        RecapBox, WarningBox,
    },
    widgets::{MatrixGridState, ResponseWidget, ResponseWidgetKind, WorkingAnswerState},
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionRecapSummary {
    pub outcome_summary: String,
    pub demonstrated_concepts: Vec<String>,
    pub weak_concepts: Vec<String>,
    pub next_review_items: Vec<String>,
    pub unfinished_objectives: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StartupReviewItem {
    pub concept_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StartupMisconceptionItem {
    pub concept_name: String,
    pub error_type: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BootstrapStudyContext {
    pub due_reviews: Vec<StartupReviewItem>,
    pub recent_misconceptions: Vec<StartupMisconceptionItem>,
    pub last_session_recap: Option<SessionRecapSummary>,
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
    pub fn bootstrap(
        config: &AppConfig,
        stats: &AppStats,
        study_context: &BootstrapStudyContext,
    ) -> Self {
        let urgency = if stats.upcoming_deadlines >= 2 {
            DeadlineUrgency::Urgent
        } else if stats.upcoming_deadlines > 0 {
            DeadlineUrgency::Upcoming
        } else {
            DeadlineUrgency::Calm
        };
        let mode = choose_start_mode(stats, study_context, urgency);
        let plan = build_start_plan(config, stats, study_context, mode, urgency);
        let panel_tab = match mode {
            SessionMode::Review => PanelTab::DueReviews,
            SessionMode::Drill => PanelTab::Deadlines,
            _ => PanelTab::SessionPlan,
        };

        Self {
            mode,
            course: config.default_course.clone(),
            time_remaining_minutes: config.default_session_minutes,
            panel_tab,
            deadline_urgency: urgency,
            metrics: SessionMetrics {
                due_reviews: stats.due_reviews,
                upcoming_deadlines: stats.upcoming_deadlines,
                attempts_logged: stats.total_attempts,
                sessions_logged: stats.total_sessions,
            },
            plan,
            transcript: bootstrap_transcript(&config.default_course),
            widget: bootstrap_widget(&config.default_course),
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

fn bootstrap_transcript(course: &str) -> Vec<ContentBlock> {
    if is_probability_course(course) {
        return probability_bootstrap_transcript();
    }

    linear_algebra_bootstrap_transcript()
}

fn bootstrap_widget(course: &str) -> ResponseWidget {
    if is_probability_course(course) {
        return ResponseWidget::WorkingAnswer(WorkingAnswerState::default());
    }

    ResponseWidget::MatrixGrid(MatrixGridState::new(2, 2))
}

fn is_probability_course(course: &str) -> bool {
    let normalized = course.to_lowercase();
    normalized.contains("probability") || normalized.contains("statistics")
}

fn linear_algebra_bootstrap_transcript() -> Vec<ContentBlock> {
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
            matrix_dimensions: Some(crate::MatrixDimensions { rows: 2, cols: 2 }),
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
            matrix_dimensions: None,
        }),
        ContentBlock::QuestionCard(QuestionCard {
            title: "Method-mark Prompt".to_string(),
            prompt: "Outline your working for solving a 2x2 linear system, then give the final solution vector.".to_string(),
            concept_tags: vec!["linear_systems".to_string()],
            widget_kind: ResponseWidgetKind::WorkingAnswer,
            matrix_dimensions: None,
        }),
        ContentBlock::Paragraph(ParagraphBlock {
            text: "Later iterations will swap these bootstrap cards for app-server generated session plans, question cards, grading feedback, and recaps.".to_string(),
        }),
    ]
}

fn probability_bootstrap_transcript() -> Vec<ContentBlock> {
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
            latex: "\\mathbb{E}[X] = \\sum_x x p(x), \\qquad \\mathrm{Var}(X)=\\mathbb{E}[X^2]-\\mathbb{E}[X]^2".to_string(),
            fallback_text: "E[X] = sum x p(x), Var(X) = E[X^2] - (E[X])^2".to_string(),
        }),
        ContentBlock::QuestionCard(QuestionCard {
            title: "Warm-up Expectation Check".to_string(),
            prompt: "A discrete random variable X takes values 0, 1, 2 with probabilities 0.2, 0.5, 0.3. Show your working for E[X] and Var(X), then give the final pair of values.".to_string(),
            concept_tags: vec![
                "expectation".to_string(),
                "variance".to_string(),
                "structured_input".to_string(),
            ],
            widget_kind: ResponseWidgetKind::WorkingAnswer,
            matrix_dimensions: None,
        }),
        ContentBlock::HintCard(HintCard {
            title: "Hint".to_string(),
            body: "Compute E[X] first, then E[X^2], and only then subtract (E[X])^2."
                .to_string(),
        }),
        ContentBlock::RecapBox(RecapBox {
            title: "End-of-session target".to_string(),
            highlights: vec![
                "log at least one structured stats attempt".to_string(),
                "persist the session to SQLite".to_string(),
                "prepare the next retrieval review".to_string(),
            ],
        }),
        ContentBlock::Paragraph(ParagraphBlock {
            text: "Use the panel tabs to inspect the session plan, deadlines, or scratchpad while the shell is still local-only.".to_string(),
        }),
        ContentBlock::QuestionCard(QuestionCard {
            title: "Interpretation Check".to_string(),
            prompt: "In one sentence, explain what it means if two variables have covariance 0.".to_string(),
            concept_tags: vec!["covariance".to_string(), "interpretation".to_string()],
            widget_kind: ResponseWidgetKind::RetrievalResponse,
            matrix_dimensions: None,
        }),
        ContentBlock::QuestionCard(QuestionCard {
            title: "Method-mark Prompt".to_string(),
            prompt: "List the steps for standardising a normal random variable before reading a z-table.".to_string(),
            concept_tags: vec!["normal_distribution".to_string(), "standardisation".to_string()],
            widget_kind: ResponseWidgetKind::StepList,
            matrix_dimensions: None,
        }),
        ContentBlock::Paragraph(ParagraphBlock {
            text: "Later iterations will swap these bootstrap cards for app-server generated session plans, question cards, grading feedback, and recaps.".to_string(),
        }),
    ]
}

fn choose_start_mode(
    stats: &AppStats,
    study_context: &BootstrapStudyContext,
    urgency: DeadlineUrgency,
) -> SessionMode {
    let repeated_repairs = study_context
        .recent_misconceptions
        .iter()
        .any(|item| item.error_type == "conceptual_misunderstanding");

    if stats.due_reviews >= 2 || repeated_repairs {
        SessionMode::Review
    } else if matches!(urgency, DeadlineUrgency::Urgent) && stats.total_attempts > 0 {
        SessionMode::Drill
    } else {
        SessionMode::Study
    }
}

fn build_start_plan(
    config: &AppConfig,
    stats: &AppStats,
    study_context: &BootstrapStudyContext,
    mode: SessionMode,
    urgency: DeadlineUrgency,
) -> SessionPlanSummary {
    match mode {
        SessionMode::Review => SessionPlanSummary {
            recommended_duration_minutes: config.default_session_minutes.min(35),
            why_now: if !study_context.recent_misconceptions.is_empty() {
                format!(
                    "You have active repair work to revisit, starting with {}.",
                    study_context.recent_misconceptions[0].concept_name
                )
            } else {
                "You have due retrieval items queued, so the session should repair memory before novelty."
                    .to_string()
            },
            warm_up_questions: study_context
                .due_reviews
                .iter()
                .take(2)
                .map(|item| format!("Retrieve the key rule for {}.", item.concept_name))
                .chain(
                    study_context
                        .recent_misconceptions
                        .iter()
                        .take(1)
                        .map(|item| format!("Explain the mistake behind: {}", item.description)),
                )
                .collect(),
            core_targets: study_context
                .due_reviews
                .iter()
                .take(3)
                .map(|item| item.concept_name.clone())
                .collect(),
            stretch_target: Some(
                "Only move on if the repair question is genuinely secure.".to_string(),
            ),
        },
        SessionMode::Drill => SessionPlanSummary {
            recommended_duration_minutes: config.default_session_minutes.min(30),
            why_now:
                "A deadline is close enough that this opener should feel exam-like and time-aware."
                    .to_string(),
            warm_up_questions: vec![
                "Predict the output dimensions before you compute anything.".to_string(),
                "State the most common exam-time failure mode for this topic.".to_string(),
            ],
            core_targets: vec![
                "Fast dimension checks under pressure".to_string(),
                "Accurate matrix or stats computation without wandering".to_string(),
            ],
            stretch_target: Some(match urgency {
                DeadlineUrgency::Urgent => {
                    "Finish with one short transfer or interpretation prompt under time pressure."
                        .to_string()
                }
                _ => "Finish with one short transfer prompt.".to_string(),
            }),
        },
        _ => SessionPlanSummary {
            recommended_duration_minutes: config.default_session_minutes,
            why_now: if let Some(recap) = &study_context.last_session_recap {
                if let Some(objective) = recap.unfinished_objectives.first() {
                    format!(
                        "Last session ended with unfinished work, so restart by stabilizing: {objective}."
                    )
                } else if stats.due_reviews > 0 {
                    "You have some memory pressure, but not enough to force a full review block, so start with retrieval and then progress."
                        .to_string()
                } else {
                    "No urgent repair queue yet, so this session should establish or extend fluent understanding with retrieval-first warmups."
                        .to_string()
                }
            } else if stats.due_reviews > 0 {
                "You have some memory pressure, but not enough to force a full review block, so start with retrieval and then progress."
                    .to_string()
            } else {
                "No urgent repair queue yet, so this session should establish or extend fluent understanding with retrieval-first warmups."
                    .to_string()
            },
            warm_up_questions: vec![
                study_context
                    .last_session_recap
                    .as_ref()
                    .and_then(|recap| recap.unfinished_objectives.first().cloned())
                    .unwrap_or_else(|| {
                        "State the condition for matrix multiplication dimensions.".to_string()
                    }),
                "Explain what a singular matrix tells you about invertibility.".to_string(),
            ],
            core_targets: vec![
                "Matrix multiplication fluency".to_string(),
                "Reasoning about invertibility".to_string(),
            ],
            stretch_target: Some(
                study_context
                    .last_session_recap
                    .as_ref()
                    .and_then(|recap| recap.unfinished_objectives.first().cloned())
                    .unwrap_or_else(|| {
                        "Connect determinant zero to linear dependence.".to_string()
                    }),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppConfig;

    fn stats(due_reviews: usize, upcoming_deadlines: usize, total_attempts: usize) -> AppStats {
        AppStats {
            due_reviews,
            upcoming_deadlines,
            total_attempts,
            total_sessions: 0,
        }
    }

    #[test]
    fn bootstrap_routes_to_review_when_due_reviews_exist() {
        let config = AppConfig::default();
        let snapshot = AppSnapshot::bootstrap(
            &config,
            &stats(3, 0, 2),
            &BootstrapStudyContext {
                due_reviews: vec![StartupReviewItem {
                    concept_name: "Matrix multiplication dimensions".to_string(),
                }],
                recent_misconceptions: Vec::new(),
                last_session_recap: None,
            },
        );

        assert_eq!(snapshot.mode, SessionMode::Review);
        assert_eq!(snapshot.panel_tab, PanelTab::DueReviews);
    }

    #[test]
    fn bootstrap_routes_to_drill_when_deadline_is_urgent() {
        let config = AppConfig::default();
        let snapshot =
            AppSnapshot::bootstrap(&config, &stats(0, 2, 4), &BootstrapStudyContext::default());

        assert_eq!(snapshot.mode, SessionMode::Drill);
        assert_eq!(snapshot.panel_tab, PanelTab::Deadlines);
    }

    #[test]
    fn bootstrap_uses_unfinished_objectives_from_last_session() {
        let config = AppConfig::default();
        let snapshot = AppSnapshot::bootstrap(
            &config,
            &stats(0, 0, 1),
            &BootstrapStudyContext {
                due_reviews: Vec::new(),
                recent_misconceptions: Vec::new(),
                last_session_recap: Some(SessionRecapSummary {
                    outcome_summary: "Stopped mid repair.".to_string(),
                    demonstrated_concepts: Vec::new(),
                    weak_concepts: vec!["Matrix multiplication".to_string()],
                    next_review_items: Vec::new(),
                    unfinished_objectives: vec![
                        "Rebuild the inner-dimension rule for matrix multiplication.".to_string(),
                    ],
                }),
            },
        );

        assert_eq!(snapshot.mode, SessionMode::Study);
        assert!(
            snapshot.plan.why_now.contains("unfinished work"),
            "expected why_now to mention unfinished work"
        );
        assert_eq!(
            snapshot.plan.warm_up_questions[0],
            "Rebuild the inner-dimension rule for matrix multiplication."
        );
    }

    #[test]
    fn bootstrap_transcript_matches_probability_course() {
        let config = AppConfig {
            default_course: "Probability & Statistics for Scientists".to_string(),
            ..AppConfig::default()
        };
        let snapshot =
            AppSnapshot::bootstrap(&config, &stats(0, 0, 0), &BootstrapStudyContext::default());

        assert!(matches!(
            snapshot.widget,
            ResponseWidget::WorkingAnswer(WorkingAnswerState { .. })
        ));
        let first_question = snapshot
            .transcript
            .iter()
            .find_map(|block| match block {
                ContentBlock::QuestionCard(card) => Some(card),
                _ => None,
            })
            .unwrap_or_else(|| panic!("probability bootstrap should include a question card"));

        assert!(first_question.title.contains("Expectation"));
        assert!(
            first_question
                .concept_tags
                .iter()
                .any(|tag| tag == "expectation")
        );
    }
}
