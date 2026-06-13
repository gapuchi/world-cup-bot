use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftStatus {
    Active,
    Complete,
}

impl DraftStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Complete => "complete",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}

pub struct Draft {
    pub season_id: i64,
    pub status: DraftStatus,
    pub rounds: i64,
    pub current_pick: i64,
    pub total_picks: i64,
}

pub struct DraftParticipant {
    pub user_id: u64,
    pub pick_order: i64,
}

impl Draft {
    pub fn get(conn: &Connection, season_id: i64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT season_id, status, rounds, current_pick, total_picks
            FROM drafts
            WHERE season_id = ?1
            ",
            params![season_id],
            |row| {
                let status: String = row.get(1)?;
                Ok(Self {
                    season_id: row.get(0)?,
                    status: DraftStatus::from_str(&status)
                        .ok_or_else(|| rusqlite::Error::InvalidQuery)?,
                    rounds: row.get(2)?,
                    current_pick: row.get(3)?,
                    total_picks: row.get(4)?,
                })
            },
        )
        .optional()
    }

    pub fn create_active(
        conn: &Connection,
        season_id: i64,
        rounds: i64,
        total_picks: i64,
        participants: &[(u64, i64)],
    ) -> rusqlite::Result<Self> {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "
            INSERT INTO drafts (season_id, status, rounds, current_pick, total_picks)
            VALUES (?1, ?2, ?3, 0, ?4)
            ",
            params![
                season_id,
                DraftStatus::Active.as_str(),
                rounds,
                total_picks
            ],
        )?;
        for (user_id, pick_order) in participants {
            tx.execute(
                "
                INSERT INTO draft_participants (season_id, user_id, pick_order)
                VALUES (?1, ?2, ?3)
                ",
                params![season_id, *user_id as i64, pick_order],
            )?;
        }
        tx.commit()?;
        Self::get(conn, season_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn list_participants(
        conn: &Connection,
        season_id: i64,
    ) -> rusqlite::Result<Vec<DraftParticipant>> {
        let mut stmt = conn.prepare(
            "
            SELECT user_id, pick_order
            FROM draft_participants
            WHERE season_id = ?1
            ORDER BY pick_order
            ",
        )?;
        let rows = stmt.query_map(params![season_id], |row| {
            Ok(DraftParticipant {
                user_id: row.get::<_, i64>(0)? as u64,
                pick_order: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn participant_user_ids(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<u64>> {
        Ok(Self::list_participants(conn, season_id)?
            .into_iter()
            .map(|participant| participant.user_id)
            .collect())
    }

    pub fn advance_pick(conn: &Connection, season_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "
            UPDATE drafts
            SET current_pick = current_pick + 1
            WHERE season_id = ?1
            ",
            params![season_id],
        )?;
        Ok(())
    }

    pub fn mark_complete(conn: &Connection, season_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "
            UPDATE drafts
            SET status = ?1
            WHERE season_id = ?2
            ",
            params![DraftStatus::Complete.as_str(), season_id],
        )?;
        Ok(())
    }

    pub fn delete(conn: &Connection, season_id: i64) -> rusqlite::Result<()> {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM draft_participants WHERE season_id = ?1",
            params![season_id],
        )?;
        tx.execute("DELETE FROM drafts WHERE season_id = ?1", params![season_id])?;
        tx.commit()?;
        Ok(())
    }

    /// Returns the user_id whose turn it is for the given pick index (0-based).
    pub fn picker_at_pick(participant_user_ids: &[u64], pick_index: usize) -> Option<u64> {
        let count = participant_user_ids.len();
        if count == 0 {
            return None;
        }
        let round = pick_index / count;
        let position = pick_index % count;
        let order_index = if round.is_multiple_of(2) {
            position
        } else {
            count - 1 - position
        };
        participant_user_ids.get(order_index).copied()
    }

    pub fn current_picker(conn: &Connection, season_id: i64) -> rusqlite::Result<Option<u64>> {
        let Some(draft) = Self::get(conn, season_id)? else {
            return Ok(None);
        };
        if draft.status != DraftStatus::Active {
            return Ok(None);
        }
        let participants = Self::participant_user_ids(conn, season_id)?;
        Ok(Self::picker_at_pick(
            &participants,
            draft.current_pick as usize,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_order_alternates_by_round() {
        let participants = vec![10_u64, 20, 30];

        assert_eq!(Draft::picker_at_pick(&participants, 0), Some(10));
        assert_eq!(Draft::picker_at_pick(&participants, 1), Some(20));
        assert_eq!(Draft::picker_at_pick(&participants, 2), Some(30));
        assert_eq!(Draft::picker_at_pick(&participants, 3), Some(30));
        assert_eq!(Draft::picker_at_pick(&participants, 4), Some(20));
        assert_eq!(Draft::picker_at_pick(&participants, 5), Some(10));
    }
}
