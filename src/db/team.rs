use rusqlite::{Connection, params};

pub fn upsert_name(
    conn: &Connection,
    league_id: i64,
    team_id: i64,
    name: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO teams (league_id, team_id, name)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(league_id, team_id) DO UPDATE SET name = excluded.name
        ",
        params![league_id, team_id, name],
    )?;
    Ok(())
}
