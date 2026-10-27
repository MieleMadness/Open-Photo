//! Durable local preferences and organizational metadata, with previous-value recovery.
use rusqlite::{params, Connection};
use std::collections::BTreeMap;

fn database() -> Result<Connection, String> {
    let directory = dirs::data_local_dir()
        .ok_or("Local data directory unavailable")?
        .join("Open-Photo");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let db = Connection::open(directory.join("user-data.sqlite")).map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA user_version=1;
        CREATE TABLE IF NOT EXISTS user_data(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS user_data_history(id INTEGER PRIMARY KEY, key TEXT NOT NULL, value TEXT NOT NULL);
        CREATE TRIGGER IF NOT EXISTS user_data_backup BEFORE UPDATE ON user_data
        WHEN old.value != new.value BEGIN
            INSERT INTO user_data_history(key,value) VALUES(old.key,old.value);
            DELETE FROM user_data_history WHERE key=old.key AND id NOT IN
              (SELECT id FROM user_data_history WHERE key=old.key ORDER BY id DESC LIMIT 20);
        END;").map_err(|e| e.to_string())?;
    Ok(db)
}

fn validate(key: &str, value: &str) -> Result<(), String> {
    if !key.starts_with("openphoto.") || key.len() > 100 || value.len() > 16 * 1024 * 1024 {
        return Err("Invalid Open-Photo setting or setting too large".into());
    }
    Ok(())
}

fn migrate(
    db: &mut Connection,
    legacy: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    for (key, value) in legacy {
        validate(&key, &value)?;
        // Existing durable values always win on subsequent launches.
        tx.execute(
            "INSERT OR IGNORE INTO user_data(key,value) VALUES(?1,?2)",
            params![key, value],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare("SELECT key,value FROM user_data ORDER BY key")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_user_data(
    legacy: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    migrate(&mut database()?, legacy)
}

#[tauri::command]
pub fn save_user_data(key: String, value: String) -> Result<(), String> {
    validate(&key, &value)?;
    database()?
        .execute(
            "INSERT INTO user_data(key,value) VALUES(?1,?2)
        ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn export_user_data(destination: String) -> Result<(), String> {
    use std::io::Write;
    let values = migrate(&mut database()?, BTreeMap::new())?;
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({"schemaVersion":1,"values":values}))
        .map_err(|e| e.to_string())?;
    let mut output = crate::safe_output::StagedOutput::new(std::path::Path::new(&destination))?;
    output.file().write_all(&bytes).map_err(|e| e.to_string())?;
    output.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_never_overwrites_existing_user_metadata() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE user_data(key TEXT PRIMARY KEY,value TEXT NOT NULL);")
            .unwrap();
        let first = BTreeMap::from([("openphoto.ratings".into(), "{\"a\":5}".into())]);
        assert_eq!(
            migrate(&mut db, first).unwrap()["openphoto.ratings"],
            "{\"a\":5}"
        );
        let second = BTreeMap::from([("openphoto.ratings".into(), "{}".into())]);
        assert_eq!(
            migrate(&mut db, second).unwrap()["openphoto.ratings"],
            "{\"a\":5}"
        );
    }
}
