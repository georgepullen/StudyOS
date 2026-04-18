use std::{fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopicDefinition {
    pub id: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConceptDefinition {
    pub id: String,
    pub topic_id: String,
    pub title: String,
    pub summary: String,
    pub prerequisite_ids: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseDefinition {
    pub course_id: String,
    pub title: String,
    pub topics: Vec<TopicDefinition>,
    pub concepts: Vec<ConceptDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CourseCatalog {
    pub courses: Vec<CourseDefinition>,
}

impl CourseCatalog {
    pub fn load(courses_dir: &Path) -> Result<Self> {
        if !courses_dir.exists() {
            return Ok(Self::default());
        }

        let mut courses = Vec::new();

        for entry in fs::read_dir(courses_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }

            let raw = fs::read_to_string(&path)?;
            let course: CourseDefinition = toml::from_str(&raw)?;
            courses.push(course);
        }

        courses.sort_by(|left, right| left.title.cmp(&right.title));
        Ok(Self { courses })
    }
}
