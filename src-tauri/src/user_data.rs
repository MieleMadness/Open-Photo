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

/// Project ownership follows saved workspaces; imported project files cannot grant file access.
pub(crate) fn workspace_roots(id: &str) -> Result<Vec<String>, String> {
    workspace_roots_from(&database()?, id)
}
fn workspace_roots_from(db: &Connection, id: &str) -> Result<Vec<String>, String> {
    #[derive(serde::Deserialize)]
    struct Workspace {
        id: String,
        #[serde(default)]
        root: String,
        #[serde(default)]
        roots: Vec<String>,
    }
    let value: String = db
        .query_row(
            "SELECT value FROM user_data WHERE key='openphoto.workspaces'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "Save a workspace before opening its projects")?;
    let workspaces: Vec<Workspace> =
        serde_json::from_str(&value).map_err(|_| "Saved workspace settings need repair")?;
    let workspace = workspaces
        .into_iter()
        .find(|w| w.id == id)
        .ok_or("Workspace is no longer available")?;
    let roots = if workspace.roots.is_empty() && !workspace.root.is_empty() {
        vec![workspace.root]
    } else {
        workspace.roots
    };
    if roots.len() > 1000 || roots.iter().any(|r| r.len() > 4096 || r.contains('\0')) {
        return Err("Workspace folder references are invalid".into());
    }
    Ok(roots.into_iter().filter(|r| !r.is_empty()).collect())
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
    fn project_access_uses_only_the_named_saved_workspace_and_retains_offline_roots() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE user_data(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
            .unwrap();
        let values = BTreeMap::from([("openphoto.workspaces".into(), r#"[{"id":"one","root":"offline-a","roots":["offline-a","offline-b"]},{"id":"two","root":"other"}]"#.into())]);
        migrate(&mut db, values).unwrap();
        assert_eq!(
            workspace_roots_from(&db, "one").unwrap(),
            vec!["offline-a", "offline-b"]
        );
        assert_eq!(workspace_roots_from(&db, "two").unwrap(), vec!["other"]);
        assert!(workspace_roots_from(&db, "missing").is_err());
    }

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
