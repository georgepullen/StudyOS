use serde::{Deserialize, Serialize};

use crate::{
    ContentBlock, HintCard, MathBlock, MatrixBlock, ParagraphBlock, QuestionCard, RecapBox,
    ResponseWidgetKind, SessionPlanSummary, WarningBox,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TutorBlock {
    Paragraph {
        text: String,
    },
    Hint {
        title: String,
        body: String,
    },
    Warning {
        title: String,
        body: String,
    },
    Math {
        latex: String,
        fallback_text: String,
    },
    Matrix {
        title: String,
        rows: Vec<Vec<String>>,
    },
    BulletList {
        items: Vec<String>,
    },
    Recap {
        title: String,
        highlights: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TutorQuestion {
    pub title: String,
    pub prompt: String,
    pub concept_tags: Vec<String>,
    pub widget_kind: ResponseWidgetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TutorTurnPayload {
    pub session_plan: Option<SessionPlanSummary>,
    pub teaching_blocks: Vec<TutorBlock>,
    pub question: Option<TutorQuestion>,
}

impl TutorTurnPayload {
    pub fn into_content_blocks(self) -> Vec<ContentBlock> {
        let mut blocks = Vec::new();

        for block in self.teaching_blocks {
            let mapped = match block {
                TutorBlock::Paragraph { text } => ContentBlock::Paragraph(ParagraphBlock { text }),
                TutorBlock::Hint { title, body } => {
                    ContentBlock::HintCard(HintCard { title, body })
                }
                TutorBlock::Warning { title, body } => {
                    ContentBlock::WarningBox(WarningBox { title, body })
                }
                TutorBlock::Math {
                    latex,
                    fallback_text,
                } => ContentBlock::MathBlock(MathBlock {
                    latex,
                    fallback_text,
                }),
                TutorBlock::Matrix { title, rows } => {
                    ContentBlock::MatrixBlock(MatrixBlock { title, rows })
                }
                TutorBlock::BulletList { items } => ContentBlock::BulletList(items),
                TutorBlock::Recap { title, highlights } => {
                    ContentBlock::RecapBox(RecapBox { title, highlights })
                }
            };

            blocks.push(mapped);
        }

        if let Some(question) = self.question {
            blocks.push(ContentBlock::QuestionCard(QuestionCard {
                title: question.title,
                prompt: question.prompt,
                concept_tags: question.concept_tags,
                widget_kind: question.widget_kind,
            }));
        }

        blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutor_payload_maps_into_content_blocks() {
        let payload = TutorTurnPayload {
            session_plan: None,
            teaching_blocks: vec![
                TutorBlock::Paragraph {
                    text: "Recall the inner dimensions first.".to_string(),
                },
                TutorBlock::Math {
                    latex: "A_{m\\times n}B_{n\\times p}".to_string(),
                    fallback_text: "A m by n times B n by p".to_string(),
                },
            ],
            question: Some(TutorQuestion {
                title: "Dimension Check".to_string(),
                prompt: "What dimensions must match before multiplying AB?".to_string(),
                concept_tags: vec!["matrix multiplication".to_string()],
                widget_kind: ResponseWidgetKind::RetrievalResponse,
            }),
        };

        let blocks = payload.into_content_blocks();
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0], ContentBlock::Paragraph(_)));
        assert!(matches!(blocks[1], ContentBlock::MathBlock(_)));
        assert!(matches!(blocks[2], ContentBlock::QuestionCard(_)));
    }
}
