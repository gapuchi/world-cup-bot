use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftOrderKind {
    Snake,
    Linear,
}

impl DraftOrderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snake => "snake",
            Self::Linear => "linear",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "snake" => Some(Self::Snake),
            "linear" => Some(Self::Linear),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftSessionStatus {
    Active,
    Complete,
}

impl DraftSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Complete => "complete",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DraftSession {
    pub season_id: i64,
    pub order_kind: DraftOrderKind,
    pub status: DraftSessionStatus,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct DraftParticipant {
    pub season_id: i64,
    pub position: i64,
    pub user_id: u64,
}

impl DraftSession {
    pub fn get(conn: &Connection, season_id: i64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT season_id, order_kind, status, created_at
            FROM draft_sessions
            WHERE season_id = ?1
            ",
            params![season_id],
            |row| {
                let kind_raw: String = row.get(1)?;
                let status_raw: String = row.get(2)?;
                let order_kind = DraftOrderKind::parse(&kind_raw).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        format!("unknown order_kind `{kind_raw}`").into(),
                    )
                })?;
                let status = DraftSessionStatus::parse(&status_raw).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        format!("unknown draft status `{status_raw}`").into(),
                    )
                })?;
                Ok(Self {
                    season_id: row.get(0)?,
                    order_kind,
                    status,
                    created_at: row.get(3)?,
                })
            },
        )
        .optional()
    }

    pub fn upsert(
        conn: &Connection,
        season_id: i64,
        order_kind: DraftOrderKind,
        status: DraftSessionStatus,
        created_at: &str,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO draft_sessions (season_id, order_kind, status, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(season_id) DO UPDATE SET
                order_kind = excluded.order_kind,
                status = excluded.status,
                created_at = excluded.created_at
            ",
            params![
                season_id,
                order_kind.as_str(),
                status.as_str(),
                created_at
            ],
        )?;
        Ok(())
    }

    pub fn set_status(
        conn: &Connection,
        season_id: i64,
        status: DraftSessionStatus,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "UPDATE draft_sessions SET status = ?1 WHERE season_id = ?2",
            params![status.as_str(), season_id],
        )?;
        Ok(())
    }

    pub fn delete(conn: &Connection, season_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "DELETE FROM draft_participants WHERE season_id = ?1",
            params![season_id],
        )?;
        conn.execute(
            "DELETE FROM draft_sessions WHERE season_id = ?1",
            params![season_id],
        )?;
        Ok(())
    }
}

impl DraftParticipant {
    pub fn replace_all(
        conn: &Connection,
        season_id: i64,
        user_ids_in_order: &[u64],
    ) -> rusqlite::Result<()> {
        conn.execute(
            "DELETE FROM draft_participants WHERE season_id = ?1",
            params![season_id],
        )?;
        for (position, user_id) in user_ids_in_order.iter().enumerate() {
            conn.execute(
                "
                INSERT INTO draft_participants (season_id, position, user_id)
                VALUES (?1, ?2, ?3)
                ",
                params![season_id, position as i64, *user_id as i64],
            )?;
        }
        Ok(())
    }

    pub fn list_ordered(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "
            SELECT season_id, position, user_id
            FROM draft_participants
            WHERE season_id = ?1
            ORDER BY position
            ",
        )?;
        let rows = stmt.query_map(params![season_id], |row| {
            Ok(Self {
                season_id: row.get(0)?,
                position: row.get(1)?,
                user_id: row.get::<_, i64>(2)? as u64,
            })
        })?;
        rows.collect()
    }

    pub fn user_ids_ordered(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<u64>> {
        Ok(Self::list_ordered(conn, season_id)?
            .into_iter()
            .map(|p| p.user_id)
            .collect())
    }
}
