pub mod config;
pub mod content;
pub mod course;
pub mod local_data;
pub mod session;
pub mod store;
pub mod tutor;
pub mod widgets;

pub use config::{AppConfig, AppPaths, FocusSettings, RendererMode, StrictnessMode, ThemeMode};
pub use content::{
    ContentBlock, HeadingBlock, HintCard, MathBlock, MatrixBlock, ParagraphBlock, QuestionCard,
    RecapBox, WarningBox,
};
pub use course::{ConceptDefinition, CourseCatalog, CourseDefinition, TopicDefinition};
pub use local_data::{
    DeadlineEntry, LocalContext, MaterialEntry, TimetableData, TimetableSlot,
    append_timetable_slot, load_deadlines, load_materials, load_timetable, save_deadlines,
    save_timetable, upsert_deadline,
};
pub use session::{
    ActivityItem, ActivityStatus, AppSnapshot, BootstrapStudyContext, DeadlineUrgency,
    KeybindingHint, PanelTab, SessionMetrics, SessionMode, SessionPlanSummary, SessionRecapSummary,
    StartupMisconceptionItem, StartupReviewItem,
};
pub use store::{
    AppDatabase, AppStats, AttemptRecord, DueReviewSummary, MisconceptionInput,
    MisconceptionSummary, ResumeStateRecord, SessionRecapRecord, SessionRecord,
};
pub use tutor::{
    TutorBlock, TutorCorrectness, TutorErrorType, TutorEvaluation, TutorMisconception,
    TutorQuestion, TutorReasoningQuality, TutorSessionClosePayload, TutorTurnPayload,
};
pub use widgets::{
    MatrixDimensions, MatrixGridState, ResponseWidget, ResponseWidgetKind, RetrievalResponseState,
    StepListState, WorkingAnswerField, WorkingAnswerState,
};

pub fn bootstrap_message() -> &'static str {
    "StudyOS bootstrap: terminal-native adaptive tutor runtime"
}

#[cfg(test)]
mod tests {
    use super::bootstrap_message;

    #[test]
    fn bootstrap_message_mentions_studyos() {
        assert!(bootstrap_message().contains("StudyOS"));
    }
}
