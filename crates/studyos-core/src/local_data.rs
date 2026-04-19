use std::{
    collections::HashSet,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use time::{Duration, OffsetDateTime, Weekday, format_description::well_known::Rfc3339};

use crate::{AppPaths, CourseCatalog, StudyWindow, WindowSource};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeadlineEntry {
    pub id: String,
    pub source: String,
    pub title: String,
    pub due_at: String,
    pub course: String,
    pub weight: f32,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimetableSlot {
    pub day: String,
    pub start: String,
    pub end: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimetableData {
    pub timezone: String,
    pub slots: Vec<TimetableSlot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialEntry {
    pub id: String,
    pub title: String,
    pub course: String,
    pub topic_tags: Vec<String>,
    pub material_type: String,
    pub path: String,
    pub snippet: String,
    #[serde(default)]
    pub source_hash: String,
    #[serde(default)]
    pub source_modified_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MaterialConceptIndexEntry {
    pub path: String,
    pub topic_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MaterialManifest {
    pub generated_at: String,
    #[serde(default)]
    pub entries: Vec<MaterialEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MaterialIngestionStatus {
    pub files_indexed: usize,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocalContext {
    pub deadlines: Vec<DeadlineEntry>,
    pub timetable: Option<TimetableData>,
    pub materials: Vec<MaterialEntry>,
    pub courses: CourseCatalog,
}

impl LocalContext {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        let deadlines = load_deadlines(&paths.deadlines_path)?;
        let timetable = load_timetable(&paths.timetable_path)?;
        let materials = load_materials(&paths.materials_manifest_path)?;
        let courses = CourseCatalog::load(&paths.courses_dir)?;

        Ok(Self {
            deadlines,
            timetable,
            materials,
            courses,
        })
    }

    pub fn upcoming_deadline_count(&self) -> usize {
        let horizon = OffsetDateTime::now_utc() + Duration::days(14);
        self.deadlines
            .iter()
            .filter(|deadline| {
                OffsetDateTime::parse(&deadline.due_at, &Rfc3339)
                    .map(|due_at| due_at <= horizon)
                    .unwrap_or(true)
            })
            .count()
    }

    pub fn upcoming_deadline_count_for_course(&self, course: &str) -> usize {
        let horizon = OffsetDateTime::now_utc() + Duration::days(14);
        self.deadlines
            .iter()
            .filter(|deadline| deadline.course.eq_ignore_ascii_case(course))
            .filter(|deadline| {
                OffsetDateTime::parse(&deadline.due_at, &Rfc3339)
                    .map(|due_at| due_at <= horizon)
                    .unwrap_or(true)
            })
            .count()
    }

    pub fn search_materials(
        &self,
        course_filter: Option<&str>,
        terms: &[String],
        limit: usize,
    ) -> Vec<MaterialEntry> {
        let normalized_terms = terms
            .iter()
            .map(|term| term.trim().to_lowercase())
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>();

        let mut scored = self
            .materials
            .iter()
            .filter_map(|entry| {
                let course_matches = course_filter
                    .map(|course| entry.course.eq_ignore_ascii_case(course))
                    .unwrap_or(true);
                if !course_matches {
                    return None;
                }

                let haystack = format!(
                    "{} {} {} {}",
                    entry.title,
                    entry.snippet,
                    entry.material_type,
                    entry.topic_tags.join(" ")
                )
                .to_lowercase();

                let mut score = if course_filter.is_some() { 2 } else { 0 };
                if normalized_terms.is_empty() {
                    score += 1;
                } else {
                    for term in &normalized_terms {
                        if haystack.contains(term) {
                            score += 2;
                        }
                    }
                }

                (score > 0).then(|| (score, entry.clone()))
            })
            .collect::<Vec<_>>();

        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.title.cmp(&right.1.title))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry)
            .collect()
    }

    pub fn next_timetable_slots(&self, limit: usize) -> Vec<TimetableSlot> {
        let Some(timetable) = &self.timetable else {
            return Vec::new();
        };

        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let today_index = weekday_index(now.weekday());

        let mut slots = timetable
            .slots
            .iter()
            .filter_map(|slot| {
                let slot_index = weekday_index(parse_weekday(&slot.day)?);
                let day_distance = (slot_index + 7 - today_index) % 7;
                Some((day_distance, slot.start.clone(), slot.clone()))
            })
            .collect::<Vec<_>>();

        slots.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        slots
            .into_iter()
            .take(limit)
            .map(|(_, _, slot)| slot)
            .collect()
    }

    pub fn today_timetable_slots(&self) -> Vec<TimetableSlot> {
        let Some(timetable) = &self.timetable else {
            return Vec::new();
        };

        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        timetable.slots_for_weekday(now.weekday())
    }

    pub fn best_study_window(&self) -> Option<StudyWindow> {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        self.best_study_window_at(now)
    }

    pub fn best_study_window_for_course(&self, course: &str) -> Option<StudyWindow> {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        self.best_study_window_for_course_at(course, now)
    }

    pub fn best_study_window_at(&self, now: OffsetDateTime) -> Option<StudyWindow> {
        if let Some(window) = self.timetable_gap_window(now) {
            return Some(window);
        }

        let local_hour = now.hour();
        let duration_minutes = if local_hour >= 20 { 60 } else { 90 };
        let source = if self.has_deadline_within_hours(now, 48) {
            WindowSource::BeforeDeadline
        } else {
            WindowSource::EveningBlock
        };
        Some(StudyWindow {
            start: now.format(&Rfc3339).unwrap_or_else(|_| now.to_string()),
            duration_minutes,
            source,
        })
    }

    pub fn best_study_window_for_course_at(
        &self,
        course: &str,
        now: OffsetDateTime,
    ) -> Option<StudyWindow> {
        if let Some(window) = self.timetable_gap_window_for_course(course, now) {
            return Some(window);
        }

        let local_hour = now.hour();
        let duration_minutes = if local_hour >= 20 { 60 } else { 90 };
        let source = if self.has_deadline_within_hours_for_course(course, now, 48) {
            WindowSource::BeforeDeadline
        } else {
            WindowSource::EveningBlock
        };
        Some(StudyWindow {
            start: now.format(&Rfc3339).unwrap_or_else(|_| now.to_string()),
            duration_minutes,
            source,
        })
    }

    fn timetable_gap_window(&self, now: OffsetDateTime) -> Option<StudyWindow> {
        let timetable = self.timetable.as_ref()?;
        for slot in timetable.slots_for_weekday(now.weekday()) {
            let start_minutes = parse_clock_minutes(&slot.start)?;
            let now_minutes = (now.hour() as u16) * 60 + now.minute() as u16;
            if start_minutes <= now_minutes {
                continue;
            }

            let gap_minutes = start_minutes.saturating_sub(now_minutes);
            if gap_minutes < 10 {
                continue;
            }

            let source = if self.has_deadline_within_hours(now, 48) {
                WindowSource::BeforeDeadline
            } else {
                WindowSource::TimetableGap
            };
            return Some(StudyWindow {
                start: now.format(&Rfc3339).unwrap_or_else(|_| now.to_string()),
                duration_minutes: gap_minutes.min(120),
                source,
            });
        }

        None
    }

    fn timetable_gap_window_for_course(
        &self,
        course: &str,
        now: OffsetDateTime,
    ) -> Option<StudyWindow> {
        let timetable = self.timetable.as_ref()?;
        for slot in timetable.slots_for_weekday(now.weekday()) {
            let start_minutes = parse_clock_minutes(&slot.start)?;
            let now_minutes = (now.hour() as u16) * 60 + now.minute() as u16;
            if start_minutes <= now_minutes {
                continue;
            }

            let gap_minutes = start_minutes.saturating_sub(now_minutes);
            if gap_minutes < 10 {
                continue;
            }

            let source = if self.has_deadline_within_hours_for_course(course, now, 48) {
                WindowSource::BeforeDeadline
            } else {
                WindowSource::TimetableGap
            };
            return Some(StudyWindow {
                start: now.format(&Rfc3339).unwrap_or_else(|_| now.to_string()),
                duration_minutes: gap_minutes.min(120),
                source,
            });
        }

        None
    }

    fn has_deadline_within_hours(&self, now: OffsetDateTime, hours: i64) -> bool {
        let horizon = now + Duration::hours(hours);
        self.deadlines.iter().any(|deadline| {
            OffsetDateTime::parse(&deadline.due_at, &Rfc3339)
                .map(|due_at| due_at >= now && due_at <= horizon)
                .unwrap_or(false)
        })
    }

    fn has_deadline_within_hours_for_course(
        &self,
        course: &str,
        now: OffsetDateTime,
        hours: i64,
    ) -> bool {
        let horizon = now + Duration::hours(hours);
        self.deadlines
            .iter()
            .filter(|deadline| deadline.course.eq_ignore_ascii_case(course))
            .any(|deadline| {
                OffsetDateTime::parse(&deadline.due_at, &Rfc3339)
                    .map(|due_at| due_at >= now && due_at <= horizon)
                    .unwrap_or(false)
            })
    }
}

impl TimetableData {
    pub fn slots_for_weekday(&self, weekday: Weekday) -> Vec<TimetableSlot> {
        let target = weekday_index(weekday);
        let mut slots = self
            .slots
            .iter()
            .filter(|slot| {
                parse_weekday(&slot.day)
                    .map(|day| weekday_index(day) == target)
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();
        slots.sort_by(|left, right| left.start.cmp(&right.start));
        slots
    }
}

pub fn load_deadlines(path: &Path) -> Result<Vec<DeadlineEntry>> {
    let mut deadlines = load_json_file::<Vec<DeadlineEntry>>(path)?.unwrap_or_default();
    deadlines.sort_by(|left, right| left.due_at.cmp(&right.due_at));
    Ok(deadlines)
}

pub fn save_deadlines(path: &Path, deadlines: &[DeadlineEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut entries = deadlines.to_vec();
    entries.sort_by(|left, right| left.due_at.cmp(&right.due_at));
    fs::write(path, serde_json::to_string_pretty(&entries)?)?;
    Ok(())
}

pub fn upsert_deadline(path: &Path, entry: DeadlineEntry) -> Result<Vec<DeadlineEntry>> {
    let mut deadlines = load_deadlines(path)?;
    if let Some(existing) = deadlines
        .iter_mut()
        .find(|deadline| deadline.id == entry.id)
    {
        *existing = entry;
    } else {
        deadlines.push(entry);
    }
    save_deadlines(path, &deadlines)?;
    Ok(deadlines)
}

pub fn load_materials(path: &Path) -> Result<Vec<MaterialEntry>> {
    let mut materials = if !path.exists() {
        Vec::new()
    } else {
        let raw = fs::read_to_string(path)?;
        if let Ok(manifest) = serde_json::from_str::<MaterialManifest>(&raw) {
            manifest.entries
        } else {
            serde_json::from_str::<Vec<MaterialEntry>>(&raw)?
        }
    };
    materials.sort_by(|left, right| left.title.cmp(&right.title));
    Ok(materials)
}

pub fn load_material_ingestion_status(paths: &AppPaths) -> Result<MaterialIngestionStatus> {
    if !paths.materials_manifest_path.exists() {
        return Ok(MaterialIngestionStatus::default());
    }

    let raw = fs::read_to_string(&paths.materials_manifest_path)?;
    let manifest = serde_json::from_str::<MaterialManifest>(&raw)?;
    Ok(MaterialIngestionStatus {
        files_indexed: manifest.entries.len(),
        last_run_at: (!manifest.generated_at.trim().is_empty()).then_some(manifest.generated_at),
    })
}

pub fn ingest_materials(paths: &AppPaths, courses: &CourseCatalog) -> Result<MaterialManifest> {
    fs::create_dir_all(&paths.materials_raw_dir)?;
    fs::create_dir_all(&paths.materials_index_dir)?;

    let previous_manifest = if paths.materials_manifest_path.exists() {
        let raw = fs::read_to_string(&paths.materials_manifest_path)?;
        serde_json::from_str::<MaterialManifest>(&raw).unwrap_or_default()
    } else {
        MaterialManifest::default()
    };

    let mut previous_by_path = previous_manifest
        .entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<std::collections::HashMap<_, _>>();

    let mut entries = Vec::new();
    let mut concepts = Vec::new();

    for path in walk_material_files(&paths.materials_raw_dir)? {
        let relative_path = path
            .strip_prefix(&paths.materials_raw_dir)
            .unwrap_or(&path)
            .display()
            .to_string();
        let metadata = fs::metadata(&path)?;
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(system_time_to_rfc3339)
            .unwrap_or_default();
        let source_hash = hash_file(&path)?;

        if let Some(previous) = previous_by_path.remove(&relative_path)
            && previous.source_hash == source_hash
            && previous.source_modified_at == modified_at
        {
            concepts.push(MaterialConceptIndexEntry {
                path: previous.path.clone(),
                topic_tags: previous.topic_tags.clone(),
            });
            entries.push(previous);
            continue;
        }

        let extracted_text = match extract_material_text(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let snippet = snippet_from_text(&extracted_text);
        let title = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| relative_path.clone());
        let topic_tags = derive_topic_tags(&title, &extracted_text, courses);
        let course = infer_course(
            &relative_path,
            &title,
            &extracted_text,
            courses,
            &topic_tags,
        );
        let material_type = infer_material_type(&path);

        let entry = MaterialEntry {
            id: make_material_id(&relative_path),
            title,
            course,
            topic_tags: topic_tags.clone(),
            material_type,
            path: relative_path.clone(),
            snippet,
            source_hash,
            source_modified_at: modified_at,
        };
        concepts.push(MaterialConceptIndexEntry {
            path: relative_path,
            topic_tags,
        });
        entries.push(entry);
    }

    entries.sort_by(|left, right| left.title.cmp(&right.title));
    concepts.sort_by(|left, right| left.path.cmp(&right.path));

    let manifest = MaterialManifest {
        generated_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string()),
        entries,
    };
    fs::write(
        &paths.materials_manifest_path,
        serde_json::to_string_pretty(&manifest)?,
    )?;
    fs::write(
        &paths.materials_concepts_path,
        serde_json::to_string_pretty(&concepts)?,
    )?;
    Ok(manifest)
}

pub fn load_timetable(path: &Path) -> Result<Option<TimetableData>> {
    let mut timetable = load_json_file::<TimetableData>(path)?;
    if let Some(data) = &mut timetable {
        sort_timetable_slots(&mut data.slots);
    }
    Ok(timetable)
}

pub fn save_timetable(path: &Path, timetable: &TimetableData) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut data = timetable.clone();
    sort_timetable_slots(&mut data.slots);
    fs::write(path, serde_json::to_string_pretty(&data)?)?;
    Ok(())
}

pub fn append_timetable_slot(
    path: &Path,
    timezone: String,
    slot: TimetableSlot,
) -> Result<TimetableData> {
    let mut timetable = load_timetable(path)?.unwrap_or(TimetableData {
        timezone: timezone.clone(),
        slots: Vec::new(),
    });
    if timetable.timezone.trim().is_empty() {
        timetable.timezone = timezone;
    }
    timetable.slots.push(slot);
    save_timetable(path, &timetable)?;
    Ok(timetable)
}

fn parse_weekday(day: &str) -> Option<Weekday> {
    match day.trim().to_ascii_lowercase().as_str() {
        "monday" => Some(Weekday::Monday),
        "tuesday" => Some(Weekday::Tuesday),
        "wednesday" => Some(Weekday::Wednesday),
        "thursday" => Some(Weekday::Thursday),
        "friday" => Some(Weekday::Friday),
        "saturday" => Some(Weekday::Saturday),
        "sunday" => Some(Weekday::Sunday),
        _ => None,
    }
}

fn parse_clock_minutes(clock: &str) -> Option<u16> {
    let mut parts = clock.split(':');
    let hour = parts.next()?.parse::<u16>().ok()?;
    let minute = parts.next()?.parse::<u16>().ok()?;
    Some(hour * 60 + minute)
}

fn weekday_index(day: Weekday) -> u8 {
    match day {
        Weekday::Monday => 0,
        Weekday::Tuesday => 1,
        Weekday::Wednesday => 2,
        Weekday::Thursday => 3,
        Weekday::Friday => 4,
        Weekday::Saturday => 5,
        Weekday::Sunday => 6,
    }
}

fn sort_timetable_slots(slots: &mut [TimetableSlot]) {
    slots.sort_by(|left, right| {
        let left_day = parse_weekday(&left.day)
            .map(weekday_index)
            .unwrap_or(u8::MAX);
        let right_day = parse_weekday(&right.day)
            .map(weekday_index)
            .unwrap_or(u8::MAX);
        left_day
            .cmp(&right_day)
            .then_with(|| left.start.cmp(&right.start))
            .then_with(|| left.title.cmp(&right.title))
    });
}

fn walk_material_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else {
                files.push(entry_path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn extract_material_text(path: &Path) -> Result<String> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "txt" | "tex" => Ok(fs::read_to_string(path)?),
        "pdf" => pdf_extract::extract_text(path)
            .map_err(|error| anyhow!("pdf extraction failed for {}: {error}", path.display())),
        "docx" | "pptx" | "odt" => Err(anyhow!(
            "unsupported material type for {} (document Office formats are deferred)",
            path.display()
        )),
        other => Err(anyhow!(
            "unsupported material type for {} (.{other})",
            path.display()
        )),
    }
}

fn snippet_from_text(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(500).collect::<String>()
}

fn derive_topic_tags(title: &str, text: &str, courses: &CourseCatalog) -> Vec<String> {
    let haystack = format!("{title} {text}").to_lowercase();
    let mut tags = HashSet::new();

    for course in &courses.courses {
        for concept in &course.concepts {
            let title_match = haystack.contains(&concept.title.to_lowercase());
            let tag_match = concept
                .tags
                .iter()
                .any(|tag| haystack.contains(&normalize_lookup_token(tag)));
            if title_match || tag_match {
                tags.insert(concept.id.clone());
                for tag in &concept.tags {
                    tags.insert(tag.clone());
                }
            }
        }
    }

    let mut tags = tags.into_iter().collect::<Vec<_>>();
    tags.sort();
    tags.truncate(6);
    tags
}

fn infer_course(
    relative_path: &str,
    title: &str,
    text: &str,
    courses: &CourseCatalog,
    topic_tags: &[String],
) -> String {
    let haystack =
        format!("{relative_path} {title} {text} {}", topic_tags.join(" ")).to_lowercase();
    let mut best_course = None::<(&str, usize)>;

    for course in &courses.courses {
        let mut score = 0usize;
        if haystack.contains(&normalize_lookup_token(&course.course_id)) {
            score += 4;
        }
        if haystack.contains(&course.title.to_lowercase()) {
            score += 3;
        }
        for keyword in course_lookup_keywords(course) {
            if haystack.contains(&keyword) {
                score += 2;
            }
        }
        for concept in &course.concepts {
            if haystack.contains(&concept.title.to_lowercase()) {
                score += 2;
            }
            for tag in &concept.tags {
                if haystack.contains(&normalize_lookup_token(tag)) {
                    score += 1;
                }
            }
        }
        if score > best_course.map(|(_, best)| best).unwrap_or(0) {
            best_course = Some((course.title.as_str(), score));
        }
    }

    best_course
        .filter(|(_, score)| *score > 0)
        .map(|(title, _)| title.to_string())
        .unwrap_or_else(|| "Uncategorized".to_string())
}

fn course_lookup_keywords(course: &crate::CourseDefinition) -> Vec<String> {
    let mut keywords = HashSet::new();
    keywords.insert(normalize_lookup_token(&course.course_id));

    for token in course
        .title
        .split(|character: char| !character.is_alphanumeric())
        .map(normalize_lookup_token)
        .filter(|token| token.len() >= 4)
    {
        keywords.insert(token);
    }

    let title = course.title.to_lowercase();
    if title.contains("matrix algebra") || title.contains("linear models") {
        keywords.insert("matrix".to_string());
        keywords.insert("linear models".to_string());
        keywords.insert("matrix algebra".to_string());
        keywords.insert("eigenvalues".to_string());
        keywords.insert("eigenvectors".to_string());
        keywords.insert("orthogonal".to_string());
        keywords.insert("projection".to_string());
        keywords.insert("qr".to_string());
    }
    if title.contains("probability") || title.contains("statistics") {
        keywords.insert("probability".to_string());
        keywords.insert("statistics".to_string());
        keywords.insert("stats".to_string());
        keywords.insert("probstats".to_string());
        keywords.insert("covariance".to_string());
        keywords.insert("distribution".to_string());
    }

    keywords.into_iter().collect()
}

fn infer_material_type(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("unknown")
        .to_ascii_lowercase()
}

fn normalize_lookup_token(token: &str) -> String {
    token.to_lowercase().replace(['_', '-'], " ")
}

fn hash_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(format!("{:x}", hasher.finish()))
}

fn system_time_to_rfc3339(system_time: SystemTime) -> Option<String> {
    let duration = system_time.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    OffsetDateTime::from_unix_timestamp(duration.as_secs() as i64)
        .ok()
        .and_then(|time| time.format(&Rfc3339).ok())
}

fn make_material_id(seed: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    format!("material-{:x}", hasher.finish())
}

fn load_json_file<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;
    use crate::{AppPaths, ConceptDefinition, CourseDefinition, TopicDefinition};

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_json_path() -> std::path::PathBuf {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "studyos-local-data-{}-{}-{}.json",
            std::process::id(),
            counter,
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let _ = fs::remove_file(&path);
        path
    }

    fn temp_data_root(label: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "studyos-materials-{label}-{}-{}",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap_or_else(|err| panic!("temp dir create failed: {err}"));
        path
    }

    fn sample_catalog() -> CourseCatalog {
        CourseCatalog {
            courses: vec![
                CourseDefinition {
                    course_id: "linear".to_string(),
                    title: "Matrix Algebra & Linear Models".to_string(),
                    topics: vec![TopicDefinition {
                        id: "matrix-basics".to_string(),
                        title: "Matrix Basics".to_string(),
                        summary: "Matrices and linear models.".to_string(),
                    }],
                    concepts: vec![ConceptDefinition {
                        id: "matrix_multiplication".to_string(),
                        topic_id: "matrix-basics".to_string(),
                        title: "Matrix multiplication".to_string(),
                        summary: "Row by column products.".to_string(),
                        prerequisite_ids: vec![],
                        tags: vec!["matrix_multiplication".to_string(), "ols".to_string()],
                    }],
                },
                CourseDefinition {
                    course_id: "probability".to_string(),
                    title: "Probability & Statistics for Scientists".to_string(),
                    topics: vec![TopicDefinition {
                        id: "variance".to_string(),
                        title: "Variance".to_string(),
                        summary: "Spread and expectation.".to_string(),
                    }],
                    concepts: vec![ConceptDefinition {
                        id: "variance_definition".to_string(),
                        topic_id: "variance".to_string(),
                        title: "Variance".to_string(),
                        summary: "Expected squared deviation.".to_string(),
                        prerequisite_ids: vec![],
                        tags: vec!["variance".to_string(), "expectation".to_string()],
                    }],
                },
            ],
        }
    }

    #[test]
    fn deadline_round_trip_sorts_by_due_time() {
        let path = temp_json_path();
        let deadlines = vec![
            DeadlineEntry {
                id: "later".to_string(),
                source: "manual".to_string(),
                title: "Later".to_string(),
                due_at: "2026-05-10T12:00:00Z".to_string(),
                course: "Matrix Algebra & Linear Models".to_string(),
                weight: 0.2,
                notes: String::new(),
            },
            DeadlineEntry {
                id: "earlier".to_string(),
                source: "manual".to_string(),
                title: "Earlier".to_string(),
                due_at: "2026-05-01T12:00:00Z".to_string(),
                course: "Probability & Statistics for Scientists".to_string(),
                weight: 0.4,
                notes: String::new(),
            },
        ];

        save_deadlines(&path, &deadlines)
            .unwrap_or_else(|err| panic!("save deadlines failed: {err}"));
        let loaded =
            load_deadlines(&path).unwrap_or_else(|err| panic!("load deadlines failed: {err}"));

        assert_eq!(loaded[0].id, "earlier");
        assert_eq!(loaded[1].id, "later");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn upcoming_deadline_count_ignores_far_future_entries() {
        let now = OffsetDateTime::now_utc();
        let context = LocalContext {
            deadlines: vec![
                DeadlineEntry {
                    id: "soon".to_string(),
                    source: "manual".to_string(),
                    title: "Soon".to_string(),
                    due_at: (now + Duration::days(7))
                        .format(&Rfc3339)
                        .unwrap_or_else(|err| panic!("format failed: {err}")),
                    course: "Matrix Algebra & Linear Models".to_string(),
                    weight: 0.2,
                    notes: String::new(),
                },
                DeadlineEntry {
                    id: "later".to_string(),
                    source: "manual".to_string(),
                    title: "Later".to_string(),
                    due_at: (now + Duration::days(30))
                        .format(&Rfc3339)
                        .unwrap_or_else(|err| panic!("format failed: {err}")),
                    course: "Probability & Statistics for Scientists".to_string(),
                    weight: 0.4,
                    notes: String::new(),
                },
            ],
            timetable: None,
            materials: Vec::new(),
            courses: CourseCatalog::default(),
        };

        assert_eq!(context.upcoming_deadline_count(), 1);
        assert_eq!(
            context.upcoming_deadline_count_for_course("Matrix Algebra & Linear Models"),
            1
        );
        assert_eq!(
            context.upcoming_deadline_count_for_course("Probability & Statistics for Scientists"),
            0
        );
    }

    #[test]
    fn search_materials_prefers_course_and_term_matches() {
        let context = LocalContext {
            deadlines: Vec::new(),
            timetable: None,
            materials: vec![
                MaterialEntry {
                    id: "worksheet".to_string(),
                    title: "Matrix Multiplication Worksheet".to_string(),
                    course: "Matrix Algebra & Linear Models".to_string(),
                    topic_tags: vec!["matrix_multiplication".to_string()],
                    material_type: "worksheet".to_string(),
                    path: "materials/linear/matrix.pdf".to_string(),
                    snippet: "Compute products and explain undefined cases.".to_string(),
                    source_hash: String::new(),
                    source_modified_at: String::new(),
                },
                MaterialEntry {
                    id: "variance-notes".to_string(),
                    title: "Variance Notes".to_string(),
                    course: "Probability & Statistics for Scientists".to_string(),
                    topic_tags: vec!["variance".to_string(), "expectation".to_string()],
                    material_type: "notes".to_string(),
                    path: "materials/probability/variance.md".to_string(),
                    snippet: "Variance as expected squared deviation.".to_string(),
                    source_hash: String::new(),
                    source_modified_at: String::new(),
                },
            ],
            courses: CourseCatalog::default(),
        };

        let matches = context.search_materials(
            Some("Probability & Statistics for Scientists"),
            &[String::from("variance")],
            3,
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "variance-notes");
    }

    #[test]
    fn materials_ingest_infers_course_from_relative_path_keywords() {
        let base = temp_data_root("course-infer-path");
        let paths = AppPaths::discover(&base);
        paths
            .ensure()
            .unwrap_or_else(|err| panic!("paths ensure failed: {err}"));

        let source_fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/materials/raw/linear-models.pdf");
        let target_dir = paths.materials_raw_dir.join("12th_may_matrix");
        fs::create_dir_all(&target_dir)
            .unwrap_or_else(|err| panic!("target dir create failed: {err}"));
        fs::copy(&source_fixture, target_dir.join("Document.pdf"))
            .unwrap_or_else(|err| panic!("fixture copy failed: {err}"));

        let manifest = ingest_materials(&paths, &sample_catalog())
            .unwrap_or_else(|err| panic!("materials ingest failed: {err}"));
        let document = manifest
            .entries
            .iter()
            .find(|entry| entry.path == "12th_may_matrix/Document.pdf")
            .unwrap_or_else(|| panic!("expected Document.pdf to be indexed"));

        assert_eq!(document.course, "Matrix Algebra & Linear Models");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn next_timetable_slots_returns_sorted_upcoming_slots() {
        let context = LocalContext {
            deadlines: Vec::new(),
            timetable: Some(TimetableData {
                timezone: "Europe/London".to_string(),
                slots: vec![
                    TimetableSlot {
                        day: "wednesday".to_string(),
                        start: "14:00".to_string(),
                        end: "15:00".to_string(),
                        title: "Probability Workshop".to_string(),
                    },
                    TimetableSlot {
                        day: "monday".to_string(),
                        start: "09:00".to_string(),
                        end: "10:00".to_string(),
                        title: "Linear Algebra Lecture".to_string(),
                    },
                ],
            }),
            materials: Vec::new(),
            courses: CourseCatalog::default(),
        };

        let slots = context.next_timetable_slots(2);
        assert_eq!(slots.len(), 2);
        assert!(
            slots
                .iter()
                .any(|slot| slot.title == "Linear Algebra Lecture")
        );
        assert!(
            slots
                .iter()
                .any(|slot| slot.title == "Probability Workshop")
        );
    }

    #[test]
    fn timetable_round_trip_sorts_slots() {
        let path = temp_json_path();
        let timetable = TimetableData {
            timezone: "Europe/London".to_string(),
            slots: vec![
                TimetableSlot {
                    day: "wednesday".to_string(),
                    start: "14:00".to_string(),
                    end: "15:00".to_string(),
                    title: "Probability Workshop".to_string(),
                },
                TimetableSlot {
                    day: "monday".to_string(),
                    start: "09:00".to_string(),
                    end: "10:00".to_string(),
                    title: "Linear Algebra Lecture".to_string(),
                },
            ],
        };

        save_timetable(&path, &timetable)
            .unwrap_or_else(|err| panic!("save timetable failed: {err}"));
        let loaded = load_timetable(&path)
            .unwrap_or_else(|err| panic!("load timetable failed: {err}"))
            .unwrap_or_else(|| panic!("timetable should be present"));

        assert_eq!(loaded.slots[0].day, "monday");
        assert_eq!(loaded.slots[1].day, "wednesday");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn materials_ingest_walks_raw_dir_and_writes_manifest() {
        let root = temp_data_root("ingest-manifest");
        let paths = AppPaths::discover(&root);
        paths
            .ensure()
            .unwrap_or_else(|err| panic!("path ensure failed: {err}"));
        fs::write(
            paths.materials_raw_dir.join("matrix-notes.md"),
            "# Matrix Multiplication\nOLS relies on matrix multiplication and linear models.\n",
        )
        .unwrap_or_else(|err| panic!("failed to write raw notes: {err}"));
        fs::write(paths.materials_raw_dir.join("skip.docx"), b"binary")
            .unwrap_or_else(|err| panic!("failed to write unsupported file: {err}"));

        let manifest = ingest_materials(&paths, &sample_catalog())
            .unwrap_or_else(|err| panic!("materials ingest failed: {err}"));

        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].course, "Matrix Algebra & Linear Models");
        assert!(
            manifest.entries[0]
                .topic_tags
                .iter()
                .any(|tag| tag == "matrix_multiplication")
        );
        assert!(paths.materials_manifest_path.exists());
        assert!(paths.materials_concepts_path.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn materials_ingest_is_incremental_for_unchanged_files() {
        let root = temp_data_root("ingest-incremental");
        let paths = AppPaths::discover(&root);
        paths
            .ensure()
            .unwrap_or_else(|err| panic!("path ensure failed: {err}"));
        fs::write(
            paths.materials_raw_dir.join("variance.txt"),
            "Variance is the expected squared deviation from the mean.",
        )
        .unwrap_or_else(|err| panic!("failed to write raw text: {err}"));

        let first = ingest_materials(&paths, &sample_catalog())
            .unwrap_or_else(|err| panic!("first ingest failed: {err}"));
        let second = ingest_materials(&paths, &sample_catalog())
            .unwrap_or_else(|err| panic!("second ingest failed: {err}"));

        assert_eq!(first.entries.len(), 1);
        assert_eq!(first.entries[0], second.entries[0]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ingest_extracts_non_empty_snippet_from_pdf_fixture() {
        let root = temp_data_root("ingest-pdf");
        let paths = AppPaths::discover(&root);
        paths
            .ensure()
            .unwrap_or_else(|err| panic!("path ensure failed: {err}"));
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/materials/raw/linear-models.pdf");
        fs::copy(&fixture, paths.materials_raw_dir.join("linear-models.pdf"))
            .unwrap_or_else(|err| panic!("failed to copy pdf fixture: {err}"));

        let manifest = ingest_materials(&paths, &sample_catalog())
            .unwrap_or_else(|err| panic!("pdf ingest failed: {err}"));

        assert_eq!(manifest.entries.len(), 1);
        assert!(!manifest.entries[0].snippet.trim().is_empty());
        assert!(
            manifest.entries[0]
                .snippet
                .to_lowercase()
                .contains("linear")
        );

        let _ = fs::remove_dir_all(root);
    }
}
