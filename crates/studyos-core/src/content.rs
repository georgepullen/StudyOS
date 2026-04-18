use serde::{Deserialize, Serialize};

use crate::widgets::{MatrixDimensions, ResponseWidgetKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentBlock {
    Heading(HeadingBlock),
    Paragraph(ParagraphBlock),
    BulletList(Vec<String>),
    MathBlock(MathBlock),
    MatrixBlock(MatrixBlock),
    QuestionCard(QuestionCard),
    HintCard(HintCard),
    WarningBox(WarningBox),
    RecapBox(RecapBox),
    Divider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingBlock {
    pub level: u8,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParagraphBlock {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MathBlock {
    pub latex: String,
    pub fallback_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixBlock {
    pub title: String,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionCard {
    pub title: String,
    pub prompt: String,
    pub concept_tags: Vec<String>,
    pub widget_kind: ResponseWidgetKind,
    pub matrix_dimensions: Option<MatrixDimensions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintCard {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarningBox {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecapBox {
    pub title: String,
    pub highlights: Vec<String>,
}
