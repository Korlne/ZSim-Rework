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

// ── Disc Stat Templates ────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscStatTemplateRecord {
    pub id: Option<i64>,
    pub slot: Option<i32>,
    pub stat_type: String,
    pub stat_name: String,
    pub max_value: Option<f64>,
    pub per_roll_value: Option<f64>,
    pub max_rolls: Option<i32>,
}

fn row_to_disc_stat_template(row: &rusqlite::Row) -> rusqlite::Result<DiscStatTemplateRecord> {
    Ok(DiscStatTemplateRecord {
        id: row.get(0)?,
        slot: row.get(1)?,
        stat_type: row.get(2)?,
        stat_name: row.get(3)?,
        max_value: row.get(4)?,
        per_roll_value: row.get(5)?,
        max_rolls: row.get(6)?,
    })
}

/// Seed the disc_stat_templates table with game data. Uses INSERT OR IGNORE for idempotency.
pub fn seed_disc_stat_templates(conn: &Connection) -> Result<(), String> {
    // Main stat templates: (slot, stat_name, max_value)
    let main_stats: Vec<(i32, &str, f64)> = vec![
        (1, "HP", 2200.0),
        (2, "ATK", 316.0),
        (3, "DEF", 184.0),
        (4, "ATK%", 30.0),
        (4, "HP%", 30.0),
        (4, "DEF%", 48.0),
        (4, "Crit DMG", 48.0),
        (4, "Crit Rate", 24.0),
        (4, "Anomaly Proficiency", 92.0),
        (5, "ATK%", 30.0),
        (5, "HP%", 30.0),
        (5, "DEF%", 48.0),
        (5, "PEN Ratio", 24.0),
        (5, "Physical DMG", 30.0),
        (5, "Fire DMG", 30.0),
        (5, "Ice DMG", 30.0),
        (5, "Electric DMG", 30.0),
        (5, "Ether DMG", 30.0),
        (6, "ATK%", 30.0),
        (6, "HP%", 30.0),
        (6, "DEF%", 48.0),
        (6, "Anomaly Mastery", 30.0),
        (6, "Impact", 18.0),
        (6, "Energy Regen", 60.0),
    ];

    for (slot, name, value) in &main_stats {
        conn.execute(
            "INSERT OR IGNORE INTO disc_stat_templates (slot, stat_type, stat_name, max_value)
             VALUES (?1, 'main', ?2, ?3)",
            params![slot, name, value],
        )
        .map_err(|e| format!("Failed to seed main stat ({}, {}): {}", slot, name, e))?;
    }

    // Sub stat templates: (stat_name, per_roll_value, max_rolls)
    let sub_stats: Vec<(&str, f64, i32)> = vec![
        ("HP", 112.0, 6),
        ("HP%", 3.0, 6),
        ("ATK", 19.0, 6),
        ("ATK%", 3.0, 6),
        ("DEF", 15.0, 6),
        ("DEF%", 4.8, 6),
        ("PEN", 9.0, 6),
        ("Crit Rate", 2.4, 6),
        ("Crit DMG", 4.8, 6),
        ("Anomaly Proficiency", 9.0, 6),
    ];

    for (name, per_roll, max_rolls) in &sub_stats {
        conn.execute(
            "INSERT OR IGNORE INTO disc_stat_templates (slot, stat_type, stat_name, per_roll_value, max_rolls)
             VALUES (NULL, 'sub', ?1, ?2, ?3)",
            params![name, per_roll, max_rolls],
        )
        .map_err(|e| format!("Failed to seed sub stat ({}): {}", name, e))?;
    }

    Ok(())
}

/// List disc stat templates, optionally filtered by slot and/or stat_type.
pub fn list_disc_stat_templates(conn: &Connection, slot: Option<i32>, stat_type: Option<String>) -> Result<String, String> {
    let mut sql = String::from(
        "SELECT id, slot, stat_type, stat_name, max_value, per_roll_value, max_rolls
         FROM disc_stat_templates WHERE 1=1"
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(s) = slot {
        sql.push_str(" AND slot = ?");
        param_values.push(Box::new(s));
    }
    if let Some(ref t) = stat_type {
        sql.push_str(" AND stat_type = ?");
        param_values.push(Box::new(t.clone()));
    }
    sql.push_str(" ORDER BY slot NULLS LAST, stat_type, stat_name");

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("Failed to prepare query: {e}"))?;

    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(param_values.iter().map(|p| p.as_ref())),
            row_to_disc_stat_template,
        )
        .map_err(|e| format!("Failed to query disc stat templates: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read disc stat template row: {e}"))?);
    }
    serde_json::to_string(&result).map_err(|e| format!("Failed to serialize: {e}"))
}

/// Create or update a disc stat template. Accepts a JSON string.
pub fn save_disc_stat_template(conn: &Connection, data: &str) -> Result<String, String> {
    let rec: DiscStatTemplateRecord =
        serde_json::from_str(data).map_err(|e| format!("Invalid disc stat template JSON: {e}"))?;

    if rec.stat_type.is_empty() {
        return Err("stat_type is required".to_string());
    }
    if rec.stat_name.is_empty() {
        return Err("stat_name is required".to_string());
    }

    if let Some(id) = rec.id {
        conn.execute(
            "UPDATE disc_stat_templates SET slot=?1, stat_type=?2, stat_name=?3, max_value=?4, per_roll_value=?5, max_rolls=?6
             WHERE id=?7",
            params![rec.slot, rec.stat_type, rec.stat_name, rec.max_value, rec.per_roll_value, rec.max_rolls, id],
        )
        .map_err(|e| format!("Failed to update disc stat template: {e}"))?;
        Ok(serde_json::json!({"status": "ok", "id": id}).to_string())
    } else {
        conn.execute(
            "INSERT INTO disc_stat_templates (slot, stat_type, stat_name, max_value, per_roll_value, max_rolls)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![rec.slot, rec.stat_type, rec.stat_name, rec.max_value, rec.per_roll_value, rec.max_rolls],
        )
        .map_err(|e| format!("Failed to insert disc stat template: {e}"))?;
        let new_id = conn.last_insert_rowid();
        Ok(serde_json::json!({"status": "ok", "id": new_id}).to_string())
    }
}

/// Delete a disc stat template by id.
pub fn delete_disc_stat_template(conn: &Connection, id: i64) -> Result<String, String> {
    let affected = conn
        .execute("DELETE FROM disc_stat_templates WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete disc stat template: {e}"))?;

    if affected == 0 {
        return Err(format!("Disc stat template {} not found", id));
    }
    Ok(serde_json::json!({"status": "ok", "id": id}).to_string())
}

// ── Tauri command wrappers for disc stat templates ─────────

pub fn cmd_list_disc_stat_templates(conn: &Connection, slot: Option<i32>, stat_type: Option<String>) -> Result<String, String> {
    list_disc_stat_templates(conn, slot, stat_type)
}

pub fn cmd_save_disc_stat_template(conn: &Connection, data: String) -> Result<String, String> {
    save_disc_stat_template(conn, &data)
}

pub fn cmd_delete_disc_stat_template(conn: &Connection, id: i64) -> Result<String, String> {
    delete_disc_stat_template(conn, id)
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
