use rusqlite::{params, Connection};

/// Flat record matching the `deployed_configs` table columns.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeployedConfigRecord {
    pub config_id: String,
    pub name: String,
    pub char_id: String,
    pub char_level: Option<i32>,
    pub char_ascension: Option<i32>,
    pub cinemas: Option<String>,
    pub potentials: Option<String>,
    pub wengine_id: Option<String>,
    pub wengine_level: Option<i32>,
    pub wengine_ascension: Option<i32>,
    pub disc_configs: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

// ── Validation ───────────────────────────────────────────

fn validate_deployed_config(rec: &DeployedConfigRecord) -> Result<(), String> {
    if rec.name.is_empty() {
        return Err("name is required".to_string());
    }
    if rec.char_id.is_empty() {
        return Err("char_id is required".to_string());
    }
    if let Some(lvl) = rec.char_level {
        if !(1..=60).contains(&lvl) {
            return Err(format!("char_level {} out of range (1-60)", lvl));
        }
    }
    if let Some(asc) = rec.char_ascension {
        if !(0..=6).contains(&asc) {
            return Err(format!("char_ascension {} out of range (0-6)", asc));
        }
    }
    if let Some(lvl) = rec.wengine_level {
        if !(1..=60).contains(&lvl) {
            return Err(format!("wengine_level {} out of range (1-60)", lvl));
        }
    }
    if let Some(asc) = rec.wengine_ascension {
        if !(0..=6).contains(&asc) {
            return Err(format!("wengine_ascension {} out of range (0-6)", asc));
        }
    }
    Ok(())
}

// ── Row mapping ──────────────────────────────────────────

fn row_to_deployed_config(row: &rusqlite::Row) -> rusqlite::Result<DeployedConfigRecord> {
    Ok(DeployedConfigRecord {
        config_id: row.get(0)?,
        name: row.get(1)?,
        char_id: row.get(2)?,
        char_level: row.get(3)?,
        char_ascension: row.get(4)?,
        cinemas: row.get(5)?,
        potentials: row.get(6)?,
        wengine_id: row.get(7)?,
        wengine_level: row.get(8)?,
        wengine_ascension: row.get(9)?,
        disc_configs: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

// ── CRUD ─────────────────────────────────────────────────

/// Select all rows from deployed_configs, ordered by updated_at DESC.
fn query_all_deployed(conn: &Connection) -> Result<Vec<DeployedConfigRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT config_id, name, char_id, char_level, char_ascension,
                    cinemas, potentials, wengine_id, wengine_level, wengine_ascension,
                    disc_configs, created_at, updated_at
             FROM deployed_configs
             ORDER BY updated_at DESC",
        )
        .map_err(|e| format!("Failed to prepare query: {e}"))?;

    let rows = stmt
        .query_map([], row_to_deployed_config)
        .map_err(|e| format!("Failed to query deployed configs: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read deployed config row: {e}"))?);
    }
    Ok(result)
}

/// Generate a unique config_id based on current timestamp.
fn generate_config_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("config_{ts}")
}

/// List all deployed configs as a JSON array.
pub fn list_deployed_configs(conn: &Connection) -> Result<String, String> {
    let configs = query_all_deployed(conn)?;
    serde_json::to_string(&configs).map_err(|e| format!("Failed to serialize: {e}"))
}

/// Get a single deployed config by config_id as a JSON object.
pub fn get_deployed_config(conn: &Connection, config_id: &str) -> Result<String, String> {
    let config: DeployedConfigRecord = conn
        .query_row(
            "SELECT config_id, name, char_id, char_level, char_ascension,
                    cinemas, potentials, wengine_id, wengine_level, wengine_ascension,
                    disc_configs, created_at, updated_at
             FROM deployed_configs WHERE config_id = ?1",
            params![config_id],
            row_to_deployed_config,
        )
        .map_err(|e| format!("Deployed config '{config_id}' not found: {e}"))?;

    serde_json::to_string(&config).map_err(|e| format!("Failed to serialize: {e}"))
}

/// Create or update a deployed config. Accepts a JSON string.
/// Auto-generates config_id if empty or missing.
pub fn save_deployed_config(conn: &Connection, data: &str) -> Result<String, String> {
    let mut rec: DeployedConfigRecord =
        serde_json::from_str(data).map_err(|e| format!("Invalid deployed config JSON: {e}"))?;

    validate_deployed_config(&rec)?;

    // Auto-generate config_id for new configs
    if rec.config_id.is_empty() {
        rec.config_id = generate_config_id();
    }

    conn.execute(
        "INSERT INTO deployed_configs (config_id, name, char_id, char_level, char_ascension,
            cinemas, potentials, wengine_id, wengine_level, wengine_ascension, disc_configs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(config_id) DO UPDATE SET
            name=excluded.name, char_id=excluded.char_id, char_level=excluded.char_level,
            char_ascension=excluded.char_ascension, cinemas=excluded.cinemas,
            potentials=excluded.potentials, wengine_id=excluded.wengine_id,
            wengine_level=excluded.wengine_level, wengine_ascension=excluded.wengine_ascension,
            disc_configs=excluded.disc_configs,
            updated_at=datetime('now')",
        params![
            rec.config_id, rec.name, rec.char_id,
            rec.char_level, rec.char_ascension,
            rec.cinemas, rec.potentials,
            rec.wengine_id, rec.wengine_level, rec.wengine_ascension,
            rec.disc_configs,
        ],
    )
    .map_err(|e| format!("Failed to save deployed config: {e}"))?;

    Ok(serde_json::json!({"status": "ok", "config_id": rec.config_id}).to_string())
}

/// Delete a deployed config by config_id.
pub fn delete_deployed_config(conn: &Connection, config_id: &str) -> Result<String, String> {
    let affected = conn
        .execute("DELETE FROM deployed_configs WHERE config_id = ?1", params![config_id])
        .map_err(|e| format!("Failed to delete deployed config: {e}"))?;

    if affected == 0 {
        return Err(format!("Deployed config '{config_id}' not found"));
    }

    Ok(serde_json::json!({"status": "ok", "config_id": config_id}).to_string())
}

/// Duplicate an existing deployed config with a new config_id and "(Copy)" suffix on name.
pub fn duplicate_deployed_config(conn: &Connection, config_id: &str) -> Result<String, String> {
    let mut config: DeployedConfigRecord = conn
        .query_row(
            "SELECT config_id, name, char_id, char_level, char_ascension,
                    cinemas, potentials, wengine_id, wengine_level, wengine_ascension,
                    disc_configs, created_at, updated_at
             FROM deployed_configs WHERE config_id = ?1",
            params![config_id],
            row_to_deployed_config,
        )
        .map_err(|e| format!("Deployed config '{config_id}' not found: {e}"))?;

    config.config_id = generate_config_id();
    config.name = format!("{} (Copy)", config.name);
    config.created_at = None;
    config.updated_at = None;

    conn.execute(
        "INSERT INTO deployed_configs (config_id, name, char_id, char_level, char_ascension,
            cinemas, potentials, wengine_id, wengine_level, wengine_ascension, disc_configs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            config.config_id, config.name, config.char_id,
            config.char_level, config.char_ascension,
            config.cinemas, config.potentials,
            config.wengine_id, config.wengine_level, config.wengine_ascension,
            config.disc_configs,
        ],
    )
    .map_err(|e| format!("Failed to duplicate deployed config: {e}"))?;

    Ok(serde_json::json!({"status": "ok", "config_id": config.config_id, "name": config.name}).to_string())
}

// ── Tauri command wrappers ───────────────────────────────

pub fn cmd_list_deployed_configs(conn: &Connection) -> Result<String, String> {
    list_deployed_configs(conn)
}

pub fn cmd_get_deployed_config(conn: &Connection, config_id: String) -> Result<String, String> {
    get_deployed_config(conn, &config_id)
}

pub fn cmd_save_deployed_config(conn: &Connection, data: String) -> Result<String, String> {
    save_deployed_config(conn, &data)
}

pub fn cmd_delete_deployed_config(conn: &Connection, config_id: String) -> Result<String, String> {
    delete_deployed_config(conn, &config_id)
}

pub fn cmd_duplicate_deployed_config(conn: &Connection, config_id: String) -> Result<String, String> {
    duplicate_deployed_config(conn, &config_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_entry::db::init_db;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        init_db(&conn).unwrap();
        conn
    }

    fn insert_test_character(conn: &Connection, char_id: &str, name: &str) {
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES (?1, ?2, 'Gentle_House', 'Attack', 'Physical')",
            params![char_id, name],
        )
        .unwrap();
    }

    fn insert_test_config(conn: &Connection, config_id: &str, name: &str, char_id: &str) {
        conn.execute(
            "INSERT INTO deployed_configs (config_id, name, char_id)
             VALUES (?1, ?2, ?3)",
            params![config_id, name, char_id],
        )
        .unwrap();
    }

    #[test]
    fn test_list_empty() {
        let conn = setup_conn();
        let json = list_deployed_configs(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_list_with_configs() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char1");
        insert_test_config(&conn, "cfg1", "Config 1", "c1");
        insert_test_config(&conn, "cfg2", "Config 2", "c1");

        let json = list_deployed_configs(&conn).unwrap();
        let configs: Vec<DeployedConfigRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_get_config_not_found() {
        let conn = setup_conn();
        let result = get_deployed_config(&conn, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_get_config_found() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char1");
        insert_test_config(&conn, "cfg1", "My Config", "c1");

        let json = get_deployed_config(&conn, "cfg1").unwrap();
        let config: DeployedConfigRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(config.config_id, "cfg1");
        assert_eq!(config.name, "My Config");
        assert_eq!(config.char_id, "c1");
    }

    #[test]
    fn test_save_new_config() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char1");

        let data = r#"{
            "config_id": "",
            "name": "New Config",
            "char_id": "c1",
            "char_level": 60,
            "char_ascension": 6,
            "cinemas": "[true,true,false,false,false,false]",
            "potentials": "[false,false,false,false,false,false]",
            "wengine_id": null,
            "wengine_level": null,
            "wengine_ascension": null,
            "disc_configs": "[]"
        }"#;
        let result = save_deployed_config(&conn, data).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        let config_id = parsed["config_id"].as_str().unwrap();
        assert!(config_id.starts_with("config_"));
        assert!(!config_id.is_empty());
    }

    #[test]
    fn test_save_update_config() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char1");
        insert_test_config(&conn, "cfg1", "Original", "c1");

        let data = r#"{
            "config_id": "cfg1",
            "name": "Updated Config",
            "char_id": "c1",
            "char_level": 50,
            "char_ascension": 5,
            "cinemas": null,
            "potentials": null,
            "wengine_id": null,
            "wengine_level": null,
            "wengine_ascension": null,
            "disc_configs": null
        }"#;
        save_deployed_config(&conn, data).unwrap();

        let json = get_deployed_config(&conn, "cfg1").unwrap();
        let config: DeployedConfigRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(config.name, "Updated Config");
        assert_eq!(config.char_level, Some(50));
    }

    #[test]
    fn test_save_validation_empty_name() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char1");

        let data = r#"{"config_id":"","name":"","char_id":"c1"}"#;
        let result = save_deployed_config(&conn, data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("name is required"));
    }

    #[test]
    fn test_save_validation_empty_char_id() {
        let conn = setup_conn();
        let data = r#"{"config_id":"","name":"Config","char_id":""}"#;
        let result = save_deployed_config(&conn, data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("char_id is required"));
    }

    #[test]
    fn test_save_validation_level_range() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char1");

        let data = r#"{"config_id":"","name":"Cfg","char_id":"c1","char_level":99}"#;
        let result = save_deployed_config(&conn, data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("char_level"));
    }

    #[test]
    fn test_delete_config() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char1");
        insert_test_config(&conn, "cfg1", "Delete Me", "c1");

        assert_eq!(query_all_deployed(&conn).unwrap().len(), 1);
        delete_deployed_config(&conn, "cfg1").unwrap();
        assert_eq!(query_all_deployed(&conn).unwrap().len(), 0);
    }

    #[test]
    fn test_delete_config_not_found() {
        let conn = setup_conn();
        let result = delete_deployed_config(&conn, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_duplicate_config() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char1");
        insert_test_config(&conn, "cfg1", "Original Config", "c1");

        let result = duplicate_deployed_config(&conn, "cfg1").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        let new_id = parsed["config_id"].as_str().unwrap();
        assert_ne!(new_id, "cfg1");
        assert_eq!(parsed["name"], "Original Config (Copy)");

        // Verify independent records exist
        let configs = query_all_deployed(&conn).unwrap();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_duplicate_config_not_found() {
        let conn = setup_conn();
        let result = duplicate_deployed_config(&conn, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
