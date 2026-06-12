use rusqlite::{Connection, params};

const WC_LEAGUE_ID: i64 = 1;

pub struct Team {
    pub league_id: i64,
    pub team_id: i64,
    pub name: String,
    pub short_name: Option<String>,
    pub code: Option<String>,
}

impl Team {
    pub fn upsert_name(conn: &Connection, team_id: i64, name: &str) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO teams (league_id, team_id, name)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(league_id, team_id) DO UPDATE SET name = excluded.name
            ",
            params![WC_LEAGUE_ID, team_id, name],
        )?;
        Ok(())
    }
}
