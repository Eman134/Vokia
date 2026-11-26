use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::connection::Db;
use super::error::DbError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryOriginType {
    Manual,
    Pinned,
    Review,
    Auto,
}

impl MemoryOriginType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Pinned => "pinned",
            Self::Review => "review",
            Self::Auto => "auto",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "manual" => Self::Manual,
            "pinned" => Self::Pinned,
            "auto" => Self::Auto,
            _ => Self::Review,
        }
    }

    pub fn is_protected(self) -> bool {
        matches!(self, Self::Manual | Self::Pinned)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMemory {
    pub id: String,
    pub project_id: i64,
    pub fact: String,
    pub source: String,
    pub origin_type: MemoryOriginType,
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ProjectMemory {
    pub fn is_protected(&self) -> bool {
        self.pinned || self.origin_type.is_protected()
    }

    pub fn is_active(&self) -> bool {
        self.valid_until.is_none() && self.superseded_by.is_none()
    }
}

#[derive(Clone, Debug)]
pub struct NewProjectMemory<'a> {
    pub id: &'a str,
    pub project_id: i64,
    pub fact: &'a str,
    pub source: &'a str,
    pub origin_type: MemoryOriginType,
    pub pinned: bool,
    pub author: Option<&'a str>,
    pub now_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionKind {
    Memory,
    Skill,
}

impl SuggestionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Skill => "skill",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "memory" => Some(Self::Memory),
            "skill" => Some(Self::Skill),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionOperation {
    Add,
    Update,
    Forget,
    Create,
}

impl SuggestionOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Update => "update",
            Self::Forget => "forget",
            Self::Create => "create",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "add" => Some(Self::Add),
            "update" => Some(Self::Update),
            "forget" => Some(Self::Forget),
            "create" => Some(Self::Create),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionStatus {
    Pending,
    Applied,
    Dismissed,
}

impl SuggestionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Dismissed => "dismissed",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "applied" => Some(Self::Applied),
            "dismissed" => Some(Self::Dismissed),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillScope {
    #[default]
    Project,
    Global,
}

impl SkillScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Global => "global",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "global" => Self::Global,
            _ => Self::Project,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningSuggestion {
    pub id: String,
    pub project_id: i64,
    pub kind: SuggestionKind,
    pub operation: SuggestionOperation,
    pub status: SuggestionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_slug: Option<String>,
    pub skill_scope: SkillScope,
    pub title: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_content: Option<String>,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_card_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct NewLearningSuggestion<'a> {
    pub id: &'a str,
    pub project_id: i64,
    pub kind: SuggestionKind,
    pub operation: SuggestionOperation,
    pub status: SuggestionStatus,
    pub target_memory_id: Option<&'a str>,
    pub skill_slug: Option<&'a str>,
    pub skill_scope: SkillScope,
    pub title: &'a str,
    pub content: &'a str,
    pub previous_content: Option<&'a str>,
    pub rationale: &'a str,
    pub source_agent: Option<&'a str>,
    pub source_card_id: Option<&'a str>,
    pub provider_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub now_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LearningReviewState {
    pub project_id: i64,
    pub last_reviewed_message_id: i64,
    pub turns_since_review: u32,
    pub last_turn_at: i64,
    pub last_review_at: i64,
}

#[derive(Clone, Debug)]
pub struct ReviewMessageEntry {
    pub id: i64,
    pub from_agent: String,
    pub to_agent: String,
    pub text: String,
    pub item_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEdge {
    pub source_node: String,
    pub target_node: String,
    pub relation: String,
    pub weight: i64,
}

impl Db {
    pub fn upsert_memory_edge(
        &self,
        project_id: i64,
        source_node: &str,
        target_node: &str,
        relation: &str,
        now_ms: i64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO memory_edges (project_id, source_node, target_node, relation, weight, created_at) \
             VALUES (?1, ?2, ?3, ?4, 1, ?5) \
             ON CONFLICT(project_id, source_node, target_node, relation) DO UPDATE SET weight = weight + 1",
            params![project_id, source_node, target_node, relation, now_ms],
        )?;
        Ok(())
    }

    pub fn list_memory_edges(&self, project_id: i64) -> Result<Vec<MemoryEdge>, DbError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT source_node, target_node, relation, weight \
             FROM memory_edges \
             WHERE project_id = ?1 AND valid_until IS NULL \
             ORDER BY weight DESC, id DESC LIMIT 200",
        )?;
        let rows = stmt.query_map([project_id], |row| {
            Ok(MemoryEdge {
                source_node: row.get(0)?,
                target_node: row.get(1)?,
                relation: row.get(2)?,
                weight: row.get(3)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(DbError::from)
    }

    pub fn search_project_memories(
        &self,
        project_id: i64,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ProjectMemory>, DbError> {
        let needle = query.trim().to_lowercase();
        let all: Vec<ProjectMemory> = self
            .list_project_memories(project_id)?
            .into_iter()
            .filter(|m| m.is_active())
            .collect();
        if needle.is_empty() {
            return Ok(all.into_iter().take(limit).collect());
        }
        let terms: Vec<&str> = needle.split_whitespace().filter(|t| !t.is_empty()).collect();
        let mut scored: Vec<(usize, ProjectMemory)> = all
            .into_iter()
            .filter_map(|mem| {
                let hay = format!(
                    "{} {} {} {} {} {}",
                    mem.fact.to_lowercase(),
                    mem.source.to_lowercase(),
                    mem.origin_type.as_str(),
                    mem.author.as_deref().unwrap_or("").to_lowercase(),
                    mem.wing.as_deref().unwrap_or("").to_lowercase(),
                    mem.room.as_deref().unwrap_or("").to_lowercase(),
                );
                let hits = terms.iter().filter(|t| hay.contains(*t)).count();
                if hits > 0 {
                    Some((hits + if mem.pinned { 2 } else { 0 }, mem))
                } else {
                    None
                }
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(scored.into_iter().take(limit).map(|(_, m)| m).collect())
    }

    pub fn list_project_memories(&self, project_id: i64) -> Result<Vec<ProjectMemory>, DbError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, project_id, fact, source, origin_type, pinned, author, \
                    valid_from, valid_until, superseded_by, wing, room, created_at, updated_at \
             FROM project_memories \
             WHERE project_id = ?1 \
             ORDER BY (valid_until IS NOT NULL) ASC, pinned DESC, updated_at DESC, id ASC",
        )?;
        let rows = stmt.query_map([project_id], decode_memory_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(DbError::from)
    }

    pub fn get_project_memory(
        &self,
        project_id: i64,
        memory_id: &str,
    ) -> Result<Option<ProjectMemory>, DbError> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, project_id, fact, source, origin_type, pinned, author, \
                    valid_from, valid_until, superseded_by, wing, room, created_at, updated_at \
             FROM project_memories \
             WHERE project_id = ?1 AND id = ?2",
            params![project_id, memory_id],
            decode_memory_row,
        )
        .optional()
        .map_err(DbError::from)
    }

    pub fn insert_project_memory(&self, mem: &NewProjectMemory<'_>) -> Result<(), DbError> {
        let (wing, room) = infer_wing_and_room(mem.source, mem.fact);
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT INTO project_memories \
                (id, project_id, fact, source, origin_type, pinned, author, \
                 valid_from, valid_until, superseded_by, wing, room, created_at, updated_at) \
             SELECT ?1, id, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10, ?8, ?8 FROM projects WHERE id = ?2",
            params![
                mem.id,
                mem.project_id,
                mem.fact,
                mem.source,
                mem.origin_type.as_str(),
                if mem.pinned { 1 } else { 0 },
                mem.author,
                mem.now_ms,
                wing,
                room,
            ],
        )?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(mem.project_id));
        }
        Ok(())
    }

    pub fn set_memory_palace_location(
        &self,
        project_id: i64,
        memory_id: &str,
        wing: Option<&str>,
        room: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE project_memories SET wing = COALESCE(?3, wing), room = COALESCE(?4, room) \
             WHERE project_id = ?1 AND id = ?2",
            params![project_id, memory_id, wing, room],
        )?;
        Ok(())
    }

    pub fn touch_project_memory(
        &self,
        project_id: i64,
        memory_id: &str,
        now_ms: i64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE project_memories SET updated_at = ?3 WHERE project_id = ?1 AND id = ?2",
            params![project_id, memory_id, now_ms],
        )?;
        Ok(())
    }

    pub fn supersede_project_memory(
        &self,
        project_id: i64,
        old_id: &str,
        new_id: &str,
        now_ms: i64,
    ) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let updated = conn.execute(
            "UPDATE project_memories \
             SET valid_until = ?3, superseded_by = ?4, updated_at = ?3 \
             WHERE project_id = ?1 AND id = ?2 AND valid_until IS NULL",
            params![project_id, old_id, now_ms, new_id],
        )?;
        if updated > 0 {
            let _ = conn.execute(
                "INSERT INTO memory_edges (project_id, source_node, target_node, relation, weight, created_at) \
                 VALUES (?1, ?2, ?3, 'superseded_by', 1, ?4) \
                 ON CONFLICT(project_id, source_node, target_node, relation) DO UPDATE SET weight = weight + 1",
                params![project_id, format!("memory:{old_id}"), format!("memory:{new_id}"), now_ms],
            );
        }
        Ok(updated > 0)
    }

    pub fn record_orchestration_event(
        &self,
        project_id: i64,
        event_type: &str,
        agent: Option<&str>,
        item_id: Option<&str>,
        payload_json: &str,
        now_ms: i64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO orchestration_events (project_id, event_type, agent, item_id, payload_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![project_id, event_type, agent, item_id, payload_json, now_ms],
        )?;
        Ok(())
    }

    /// Updates a memory only if it was NOT created or pinned manually by the user.
    /// Returns `Ok(true)` if updated, `Ok(false)` if protected or not found.
    pub fn update_project_memory_guarded(
        &self,
        project_id: i64,
        memory_id: &str,
        new_fact: &str,
        source: &str,
        now_ms: i64,
    ) -> Result<bool, DbError> {
        let Some(existing) = self.get_project_memory(project_id, memory_id)? else {
            return Ok(false);
        };
        if existing.is_protected() {
            return Ok(false);
        }
        let conn = self.conn()?;
        let updated = conn.execute(
            "UPDATE project_memories \
             SET fact = ?3, source = ?4, updated_at = ?5 \
             WHERE project_id = ?1 AND id = ?2 AND pinned = 0 AND origin_type NOT IN ('manual','pinned')",
            params![project_id, memory_id, new_fact, source, now_ms],
        )?;
        Ok(updated > 0)
    }

    /// Deletes a memory only if it was NOT created or pinned manually by the user.
    /// Returns `Ok(true)` if deleted, `Ok(false)` if protected or not found.
    pub fn delete_project_memory_guarded(
        &self,
        project_id: i64,
        memory_id: &str,
    ) -> Result<bool, DbError> {
        let Some(existing) = self.get_project_memory(project_id, memory_id)? else {
            return Ok(false);
        };
        if existing.is_protected() {
            return Ok(false);
        }
        let conn = self.conn()?;
        let deleted = conn.execute(
            "DELETE FROM project_memories \
             WHERE project_id = ?1 AND id = ?2 AND pinned = 0 AND origin_type NOT IN ('manual','pinned')",
            params![project_id, memory_id],
        )?;
        Ok(deleted > 0)
    }

    pub fn insert_learning_suggestion(
        &self,
        suggestion: &NewLearningSuggestion<'_>,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT INTO learning_suggestions \
                (id, project_id, kind, operation, status, target_memory_id, skill_slug, skill_scope, \
                 title, content, previous_content, rationale, source_agent, source_card_id, \
                 provider_id, model, created_at, updated_at, resolved_at) \
             SELECT ?1, id, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17, ?18 \
             FROM projects WHERE id = ?2",
            params![
                suggestion.id,
                suggestion.project_id,
                suggestion.kind.as_str(),
                suggestion.operation.as_str(),
                suggestion.status.as_str(),
                suggestion.target_memory_id,
                suggestion.skill_slug,
                suggestion.skill_scope.as_str(),
                suggestion.title,
                suggestion.content,
                suggestion.previous_content,
                suggestion.rationale,
                suggestion.source_agent,
                suggestion.source_card_id,
                suggestion.provider_id,
                suggestion.model,
                suggestion.now_ms,
                if suggestion.status == SuggestionStatus::Pending {
                    None::<i64>
                } else {
                    Some(suggestion.now_ms)
                },
            ],
        )?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(suggestion.project_id));
        }
        Ok(())
    }

    pub fn list_learning_suggestions(
        &self,
        project_id: Option<i64>,
        status: Option<SuggestionStatus>,
    ) -> Result<Vec<LearningSuggestion>, DbError> {
        let conn = self.conn()?;
        let status_str = status.map(SuggestionStatus::as_str);
        let mut stmt = conn.prepare(
            "SELECT id, project_id, kind, operation, status, target_memory_id, skill_slug, skill_scope, \
                    title, content, previous_content, rationale, source_agent, source_card_id, \
                    provider_id, model, created_at, updated_at, resolved_at \
             FROM learning_suggestions \
             WHERE (?1 IS NULL OR project_id = ?1) \
               AND (?2 IS NULL OR status = ?2) \
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![project_id, status_str], decode_suggestion_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(DbError::from)
    }

    pub fn get_learning_suggestion(&self, id: &str) -> Result<Option<LearningSuggestion>, DbError> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, project_id, kind, operation, status, target_memory_id, skill_slug, skill_scope, \
                    title, content, previous_content, rationale, source_agent, source_card_id, \
                    provider_id, model, created_at, updated_at, resolved_at \
             FROM learning_suggestions \
             WHERE id = ?1",
            [id],
            decode_suggestion_row,
        )
        .optional()
        .map_err(DbError::from)
    }

    pub fn resolve_learning_suggestion(
        &self,
        id: &str,
        status: SuggestionStatus,
        resolved_at: i64,
    ) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let updated = conn.execute(
            "UPDATE learning_suggestions \
             SET status = ?2, updated_at = ?3, resolved_at = ?3 \
             WHERE id = ?1 AND status = 'pending'",
            params![id, status.as_str(), resolved_at],
        )?;
        Ok(updated > 0)
    }

    pub fn count_pending_learning_suggestions(
        &self,
        project_id: Option<i64>,
    ) -> Result<usize, DbError> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM learning_suggestions \
             WHERE status = 'pending' AND (?1 IS NULL OR project_id = ?1)",
            [project_id],
            |row| row.get(0),
        )?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    pub fn get_learning_review_state(
        &self,
        project_id: i64,
    ) -> Result<LearningReviewState, DbError> {
        let conn = self.conn()?;
        let row = conn
            .query_row(
                "SELECT project_id, last_reviewed_message_id, turns_since_review, last_turn_at, last_review_at \
                 FROM learning_review_state WHERE project_id = ?1",
                [project_id],
                |row| {
                    let turns: i64 = row.get(2)?;
                    Ok(LearningReviewState {
                        project_id: row.get(0)?,
                        last_reviewed_message_id: row.get(1)?,
                        turns_since_review: u32::try_from(turns).unwrap_or(0),
                        last_turn_at: row.get(3)?,
                        last_review_at: row.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(row.unwrap_or(LearningReviewState {
            project_id,
            ..Default::default()
        }))
    }

    pub fn record_turn_completed_for_learning(
        &self,
        project_id: i64,
        now_ms: i64,
    ) -> Result<LearningReviewState, DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO learning_review_state (project_id, last_reviewed_message_id, turns_since_review, last_turn_at, last_review_at) \
             SELECT id, 0, 1, ?2, 0 FROM projects WHERE id = ?1 \
             ON CONFLICT(project_id) DO UPDATE SET \
                turns_since_review = learning_review_state.turns_since_review + 1, \
                last_turn_at = excluded.last_turn_at",
            params![project_id, now_ms],
        )?;
        drop(conn);
        self.get_learning_review_state(project_id)
    }

    pub fn mark_learning_review_completed(
        &self,
        project_id: i64,
        last_message_id: i64,
        now_ms: i64,
    ) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO learning_review_state (project_id, last_reviewed_message_id, turns_since_review, last_turn_at, last_review_at) \
             SELECT id, ?2, 0, ?3, ?3 FROM projects WHERE id = ?1 \
             ON CONFLICT(project_id) DO UPDATE SET \
                last_reviewed_message_id = MAX(learning_review_state.last_reviewed_message_id, excluded.last_reviewed_message_id), \
                turns_since_review = 0, \
                last_review_at = excluded.last_review_at",
            params![project_id, last_message_id, now_ms],
        )?;
        Ok(())
    }

    pub fn list_idle_learning_candidates(
        &self,
        idle_before_ms: i64,
        min_turns: u32,
    ) -> Result<Vec<i64>, DbError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT project_id FROM learning_review_state \
             WHERE turns_since_review >= ?1 \
               AND last_turn_at > 0 \
               AND last_turn_at <= ?2",
        )?;
        let rows = stmt.query_map(params![min_turns as i64, idle_before_ms], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(DbError::from)
    }

    pub fn list_messages_for_review(
        &self,
        project_id: i64,
        after_id: i64,
        limit: usize,
    ) -> Result<Vec<ReviewMessageEntry>, DbError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, from_agent, to_agent, text, item_id FROM ( \
                 SELECT id, from_agent, to_agent, text, item_id \
                 FROM messages \
                 WHERE project_id = ?1 AND id > ?2 AND trim(text) <> '' \
                 ORDER BY id DESC LIMIT ?3 \
             ) ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![project_id, after_id, limit as i64], |row| {
            Ok(ReviewMessageEntry {
                id: row.get(0)?,
                from_agent: row.get(1)?,
                to_agent: row.get(2)?,
                text: row.get(3)?,
                item_id: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(DbError::from)
    }

    pub fn list_work_steps_for_review(
        &self,
        project_id: i64,
        limit: usize,
    ) -> Result<Vec<super::work_steps::TurnWorkStepsRecord>, DbError> {
        let (records, _) = self.list_history_work_steps(project_id, None, None, limit, 0)?;
        Ok(records)
    }
}

pub fn infer_wing_and_room(source: &str, fact: &str) -> (String, String) {
    let src_l = source.trim().to_ascii_lowercase();
    let fact_l = fact.to_ascii_lowercase();
    let wing = if fact_l.contains("src-tauri")
        || fact_l.contains(".rs")
        || fact_l.contains("sqlite")
        || fact_l.contains("rust")
    {
        "backend"
    } else if fact_l.contains(".vue")
        || fact_l.contains(".ts")
        || fact_l.contains("css")
        || fact_l.contains("frontend")
    {
        "frontend"
    } else if src_l == "macro" || src_l == "arquitetura" || fact_l.contains("arquitetura") {
        "arquitetura"
    } else if fact_l.contains("orquestra") || fact_l.contains("deleg") || fact_l.contains("triagem") {
        "orquestracao"
    } else {
        "geral"
    };
    let room = if src_l == "mapa-arquivos" || fact_l.contains("arquivo") || fact_l.contains("src/") {
        "mapa-arquivos"
    } else if src_l == "macro" || src_l == "arquitetura" {
        "decisoes"
    } else if src_l == "convencao" || fact_l.contains("padrão") || fact_l.contains("sempre") {
        "convencoes"
    } else {
        "notas"
    };
    (wing.to_string(), room.to_string())
}

fn decode_memory_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectMemory> {
    let origin_raw: String = row.get(4)?;
    let pinned_int: i64 = row.get(5)?;
    Ok(ProjectMemory {
        id: row.get(0)?,
        project_id: row.get(1)?,
        fact: row.get(2)?,
        source: row.get(3)?,
        origin_type: MemoryOriginType::parse(&origin_raw),
        pinned: pinned_int != 0,
        author: row.get(6)?,
        valid_from: row.get(7)?,
        valid_until: row.get(8)?,
        superseded_by: row.get(9)?,
        wing: row.get(10)?,
        room: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn decode_suggestion_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LearningSuggestion> {
    let kind_raw: String = row.get(2)?;
    let op_raw: String = row.get(3)?;
    let status_raw: String = row.get(4)?;
    let scope_raw: String = row.get(7)?;
    Ok(LearningSuggestion {
        id: row.get(0)?,
        project_id: row.get(1)?,
        kind: SuggestionKind::parse(&kind_raw).unwrap_or(SuggestionKind::Memory),
        operation: SuggestionOperation::parse(&op_raw).unwrap_or(SuggestionOperation::Add),
        status: SuggestionStatus::parse(&status_raw).unwrap_or(SuggestionStatus::Pending),
        target_memory_id: row.get(5)?,
        skill_slug: row.get(6)?,
        skill_scope: SkillScope::parse(&scope_raw),
        title: row.get(8)?,
        content: row.get(9)?,
        previous_content: row.get(10)?,
        rationale: row.get(11)?,
        source_agent: row.get(12)?,
        source_card_id: row.get(13)?,
        provider_id: row.get(14)?,
        model: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
        resolved_at: row.get(18)?,
    })
}
