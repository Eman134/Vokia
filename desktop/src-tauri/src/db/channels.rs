use rusqlite::{params, ErrorCode, Row};
use serde::Serialize;

use super::connection::Db;
use super::error::DbError;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub members: Vec<String>,
    pub created_at: i64,
}

const CHANNEL_COLUMNS: &str = "id, project_id, name, members, created_at";
const MEMBERS_COLUMN: usize = 3;

fn channel_from_row(row: &Row<'_>) -> rusqlite::Result<Channel> {
    let members: String = row.get(MEMBERS_COLUMN)?;
    let members = serde_json::from_str(&members).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            MEMBERS_COLUMN,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(Channel {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        members,
        created_at: row.get(4)?,
    })
}

impl Db {
    pub fn list_channels(&self, project_id: i64) -> Result<Vec<Channel>, DbError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(&format!(
            "SELECT {CHANNEL_COLUMNS} FROM channels WHERE project_id = ?1 ORDER BY id"
        ))?;
        let rows = statement.query_map([project_id], channel_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn create_channel(
        &self,
        project_id: i64,
        name: &str,
        members: &[String],
    ) -> Result<Channel, DbError> {
        let members_json =
            serde_json::to_string(members).expect("uma lista de textos sempre serializa para JSON");
        let conn = self.conn()?;
        let inserted = conn
            .execute(
                "INSERT INTO channels(project_id, name, members) \
                 SELECT id, ?2, ?3 FROM projects WHERE id = ?1",
                params![project_id, name, members_json],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(ErrorCode::ConstraintViolation) => DbError::ChannelNameTaken(name.to_string()),
                _ => DbError::Sqlite(error),
            })?;
        if inserted == 0 {
            return Err(DbError::ProjectNotFound(project_id));
        }
        let channel = conn.query_row(
            &format!("SELECT {CHANNEL_COLUMNS} FROM channels WHERE id = last_insert_rowid()"),
            [],
            channel_from_row,
        )?;
        Ok(channel)
    }

    pub fn delete_channel(&self, project_id: i64, id: i64) -> Result<(), DbError> {
        let conn = self.conn()?;
        let deleted = conn.execute(
            "DELETE FROM channels WHERE project_id = ?1 AND id = ?2",
            params![project_id, id],
        )?;
        if deleted == 0 {
            return Err(DbError::ChannelNotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/db_channels_tests.rs"]
mod tests;
