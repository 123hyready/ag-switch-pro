//! Route configuration and bounded, metadata-only decision history.
use crate::{
    database::{lock_conn, Database},
    error::AppError,
};
use ag_intent_router::RouteGroup;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRecord {
    pub id: String,
    pub created_at: i64,
    pub group_id: String,
    pub app_type: String,
    pub session_id: Option<String>,
    pub branch: String,
    pub phase: String,
    pub source: String,
    pub worker_id: String,
    pub model: String,
    pub reason: String,
    pub judge_millis: u64,
    pub judge_input_tokens: Option<u64>,
    pub judge_output_tokens: Option<u64>,
    pub judge_cost_usd: Option<String>,
    pub judge_model: Option<String>,
    pub decision_source: Option<String>,
    pub selected_model: Option<String>,
    pub recommended_model: Option<String>,
    #[serde(default)]
    pub execution_attempts: Vec<ExecutionAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionAttempt {
    pub model: String,
    pub provider_id: String,
    pub status: String,
    pub elapsed_ms: u64,
}

impl Database {
    pub fn list_intent_groups(&self) -> Result<Vec<RouteGroup>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut statement = conn
            .prepare("SELECT config FROM intent_route_groups ORDER BY id")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(database_error)?;
        rows.map(|row| {
            let text = row.map_err(database_error)?;
            serde_json::from_str(&text)
                .map_err(|error| AppError::Database(format!("Invalid intent group: {error}")))
        })
        .collect()
    }

    pub fn get_intent_group(&self, id: &str) -> Result<Option<RouteGroup>, AppError> {
        let conn = lock_conn!(self.conn);
        let text: Option<String> = conn
            .query_row(
                "SELECT config FROM intent_route_groups WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(database_error)?;
        text.map(|text| {
            serde_json::from_str(&text).map_err(|error| AppError::Database(error.to_string()))
        })
        .transpose()
    }

    pub fn save_intent_group(&self, mut group: RouteGroup) -> Result<RouteGroup, AppError> {
        group.validate().map_err(AppError::Config)?;
        let conn = lock_conn!(self.conn);
        let revision: Option<u64> = conn
            .query_row(
                "SELECT revision FROM intent_route_groups WHERE id=?1",
                [&group.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(database_error)?;
        group.revision = revision
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| AppError::Config("Group revision overflow".into()))?;
        let config =
            serde_json::to_string(&group).map_err(|error| AppError::Config(error.to_string()))?;
        conn.execute("INSERT INTO intent_route_groups(id, app_type, revision, config) VALUES (?1,?2,?3,?4)
            ON CONFLICT(id) DO UPDATE SET app_type=excluded.app_type, revision=excluded.revision, config=excluded.config",
            params![group.id, group.app_type, group.revision, config]).map_err(database_error)?;
        Ok(group)
    }

    pub fn delete_intent_group(&self, id: &str) -> Result<(), AppError> {
        lock_conn!(self.conn)
            .execute("DELETE FROM intent_route_groups WHERE id=?1", [id])
            .map_err(database_error)?;
        Ok(())
    }

    pub fn record_intent_decision(&self, record: &DecisionRecord) -> Result<(), AppError> {
        let text =
            serde_json::to_string(record).map_err(|error| AppError::Database(error.to_string()))?;
        let conn = lock_conn!(self.conn);
        conn.execute("INSERT OR REPLACE INTO intent_route_decisions(id, created_at, record) VALUES(?1,?2,?3)",
            params![record.id, record.created_at, text]).map_err(database_error)?;
        conn.execute("DELETE FROM intent_route_decisions WHERE id NOT IN
            (SELECT id FROM intent_route_decisions ORDER BY created_at DESC, rowid DESC LIMIT 2000)", []).map_err(database_error)?;
        Ok(())
    }

    pub fn list_intent_decisions(&self, limit: u32) -> Result<Vec<DecisionRecord>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut statement = conn.prepare("SELECT record FROM intent_route_decisions ORDER BY created_at DESC, rowid DESC LIMIT ?1").map_err(database_error)?;
        let rows = statement
            .query_map([limit.min(500)], |row| row.get::<_, String>(0))
            .map_err(database_error)?;
        rows.map(|row| {
            let text = row.map_err(database_error)?;
            serde_json::from_str(&text).map_err(|error| AppError::Database(error.to_string()))
        })
        .collect()
    }
}

fn database_error(error: rusqlite::Error) -> AppError {
    AppError::Database(error.to_string())
}
