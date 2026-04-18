use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseWidgetKind {
    MatrixGrid,
    WorkingAnswer,
    StepList,
    RetrievalResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixDimensions {
    pub rows: usize,
    pub cols: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixGridState {
    pub dimensions: MatrixDimensions,
    pub cells: Vec<Vec<String>>,
    pub selected_row: usize,
    pub selected_col: usize,
}

impl MatrixGridState {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            dimensions: MatrixDimensions { rows, cols },
            cells: vec![vec![String::new(); cols]; rows],
            selected_row: 0,
            selected_col: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkingAnswerState {
    pub working: String,
    pub final_answer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StepListState {
    pub steps: Vec<String>,
    pub selected_step: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RetrievalResponseState {
    pub response: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseWidget {
    MatrixGrid(MatrixGridState),
    WorkingAnswer(WorkingAnswerState),
    StepList(StepListState),
    RetrievalResponse(RetrievalResponseState),
}

impl ResponseWidget {
    pub fn kind(&self) -> ResponseWidgetKind {
        match self {
            Self::MatrixGrid(_) => ResponseWidgetKind::MatrixGrid,
            Self::WorkingAnswer(_) => ResponseWidgetKind::WorkingAnswer,
            Self::StepList(_) => ResponseWidgetKind::StepList,
            Self::RetrievalResponse(_) => ResponseWidgetKind::RetrievalResponse,
        }
    }
}
