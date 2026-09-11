use crate::{config, dsp::Report};
use anyhow::Result;
use rusqlite::{Connection, params};
pub fn open() -> Result<Connection> {
    std::fs::create_dir_all(config::data_dir())?;
    let db = Connection::open(config::data_dir().join("investigations.sqlite3"))?;
    db.busy_timeout(std::time::Duration::from_secs(3))?;
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS reports (id INTEGER PRIMARY KEY, created TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, source TEXT NOT NULL, data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS findings (id INTEGER PRIMARY KEY, created TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, kind TEXT NOT NULL, data TEXT NOT NULL); PRAGMA user_version=1;")?;
    Ok(db)
}
pub fn save(r: &Report) -> Result<i64> {
    let db = open()?;
    db.execute(
        "INSERT INTO reports(source,data) VALUES(?1,?2)",
        params![r.source, serde_json::to_string(r)?],
    )?;
    Ok(db.last_insert_rowid())
}
pub fn finding(kind: &str, data: &str) -> Result<()> {
    open()?.execute(
        "INSERT INTO findings(kind,data) VALUES(?1,?2)",
        params![kind, data],
    )?;
    Ok(())
}
pub fn history() -> Result<String> {
    let db = open()?;
    let mut q = db.prepare("SELECT id,created,source FROM reports ORDER BY id DESC LIMIT 100")?;
    let lines = q
        .query_map([], |r| {
            Ok(format!(
                "#{:<5} {}  {}",
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(lines.join("\n"))
}
pub fn export(id: i64) -> Result<String> {
    Ok(open()?.query_row("SELECT data FROM reports WHERE id=?1", [id], |r| r.get(0))?)
}
