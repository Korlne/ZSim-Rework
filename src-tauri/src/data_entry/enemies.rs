use rusqlite::{params, Connection};

/// Summary fields returned by get_enemies list.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnemySummary {
    pub enemy_id: String,
    pub enemy_type: String,
    pub name: String,
    pub level: i32,
    pub hp: f64,
    pub def: f64,
    pub base_res: f64,
}

/// Full enemy record matching the `enemies` table columns.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnemyRecord {
    pub enemy_id: String,
    pub name: String,
    pub enemy_type: String,
    pub level: i32,
    pub hp: f64,
    pub def: f64,
    pub base_res: f64,
    pub daze_max: f64,
    pub resistances: String,
    pub weaknesses: String,
    pub anomaly_buildup: String,
}

// ── CRUD ─────────────────────────────────────────────────

/// Query all enemies returning summary fields only.
fn query_enemy_summaries(conn: &Connection) -> Result<Vec<EnemySummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT enemy_id, enemy_type, name, level, hp, def, base_res
             FROM enemies ORDER BY enemy_id",
        )
        .map_err(|e| format!("Failed to prepare enemies query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(EnemySummary {
                enemy_id: row.get(0)?,
                enemy_type: row.get(1)?,
                name: row.get(2)?,
                level: row.get(3)?,
                hp: row.get(4)?,
                def: row.get(5)?,
                base_res: row.get(6)?,
            })
        })
        .map_err(|e| format!("Failed to query enemies: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read enemy row: {e}"))?);
    }
    Ok(result)
}

/// Query a single enemy with all fields.
fn query_enemy_full(conn: &Connection, enemy_id: &str) -> Result<EnemyRecord, String> {
    conn.query_row(
        "SELECT enemy_id, name, enemy_type, level, hp, def, base_res,
                daze_max, resistances, weaknesses, anomaly_buildup
         FROM enemies WHERE enemy_id = ?1",
        params![enemy_id],
        |row| {
            Ok(EnemyRecord {
                enemy_id: row.get(0)?,
                name: row.get(1)?,
                enemy_type: row.get(2)?,
                level: row.get(3)?,
                hp: row.get(4)?,
                def: row.get(5)?,
                base_res: row.get(6)?,
                daze_max: row.get(7)?,
                resistances: row.get(8)?,
                weaknesses: row.get(9)?,
                anomaly_buildup: row.get(10)?,
            })
        },
    )
    .map_err(|e| format!("Enemy '{enemy_id}' not found: {e}"))
}

/// Get all enemies as JSON array (summary fields only).
pub fn get_enemies(conn: &Connection) -> Result<String, String> {
    let enemies = query_enemy_summaries(conn)?;
    serde_json::to_string(&enemies).map_err(|e| format!("Failed to serialize enemies: {e}"))
}

/// Get a single enemy with all fields as JSON object.
pub fn get_enemy(conn: &Connection, enemy_id: &str) -> Result<String, String> {
    let enemy = query_enemy_full(conn, enemy_id)?;
    serde_json::to_string(&enemy).map_err(|e| format!("Failed to serialize enemy: {e}"))
}

/// Create or update an enemy. Accepts JSON string matching EnemyRecord.
pub fn save_enemy(conn: &Connection, data: &str) -> Result<String, String> {
    let rec: EnemyRecord =
        serde_json::from_str(data).map_err(|e| format!("Invalid enemy JSON: {e}"))?;

    if rec.enemy_id.is_empty() {
        return Err("enemy_id is required".to_string());
    }

    conn.execute(
        "INSERT INTO enemies (enemy_id, name, enemy_type, level, hp, def, base_res,
            daze_max, resistances, weaknesses, anomaly_buildup)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(enemy_id) DO UPDATE SET
            name=excluded.name, enemy_type=excluded.enemy_type,
            level=excluded.level, hp=excluded.hp, def=excluded.def,
            base_res=excluded.base_res, daze_max=excluded.daze_max,
            resistances=excluded.resistances, weaknesses=excluded.weaknesses,
            anomaly_buildup=excluded.anomaly_buildup,
            updated_at=datetime('now')",
        params![
            rec.enemy_id, rec.name, rec.enemy_type, rec.level,
            rec.hp, rec.def, rec.base_res, rec.daze_max,
            rec.resistances, rec.weaknesses, rec.anomaly_buildup,
        ],
    )
    .map_err(|e| format!("Failed to save enemy: {e}"))?;

    Ok(serde_json::json!({"status": "ok", "enemy_id": rec.enemy_id}).to_string())
}

/// Delete an enemy by enemy_id.
pub fn delete_enemy(conn: &Connection, enemy_id: &str) -> Result<String, String> {
    let affected = conn
        .execute("DELETE FROM enemies WHERE enemy_id = ?1", params![enemy_id])
        .map_err(|e| format!("Failed to delete enemy: {e}"))?;

    if affected == 0 {
        return Err(format!("Enemy '{enemy_id}' not found"));
    }
    Ok(serde_json::json!({"status": "ok", "enemy_id": enemy_id}).to_string())
}

// ── Tauri command wrappers ──────────────────────────────

pub fn cmd_get_enemies(conn: &Connection) -> Result<String, String> {
    get_enemies(conn)
}

pub fn cmd_get_enemy(conn: &Connection, enemy_id: String) -> Result<String, String> {
    get_enemy(conn, &enemy_id)
}

pub fn cmd_save_enemy(conn: &Connection, data: String) -> Result<String, String> {
    save_enemy(conn, &data)
}

pub fn cmd_delete_enemy(conn: &Connection, enemy_id: String) -> Result<String, String> {
    delete_enemy(conn, &enemy_id)
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

    #[test]
    fn test_get_enemies_empty() {
        let conn = setup_conn();
        let json = get_enemies(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_save_and_get_enemy() {
        let conn = setup_conn();
        let data = r#"{
            "enemy_id": "e_001",
            "name": "Test Enemy",
            "enemy_type": "Elite",
            "level": 60,
            "hp": 500000.0,
            "def": 800.0,
            "base_res": 0.2,
            "daze_max": 2000.0,
            "resistances": "{\"Fire\": 0.4, \"Ice\": -0.2}",
            "weaknesses": "[\"Electric\"]",
            "anomaly_buildup": "{}"
        }"#;
        save_enemy(&conn, data).unwrap();

        let json = get_enemies(&conn).unwrap();
        let enemies: Vec<EnemySummary> = serde_json::from_str(&json).unwrap();
        assert_eq!(enemies.len(), 1);
        assert_eq!(enemies[0].enemy_id, "e_001");
        assert_eq!(enemies[0].enemy_type, "Elite");
    }

    #[test]
    fn test_get_enemy_full() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO enemies (enemy_id, name, enemy_type, level, hp, def, base_res, daze_max,
                resistances, weaknesses, anomaly_buildup)
             VALUES ('e_001', 'Full Enemy', 'Boss', 60, 1e6, 900.0, 0.3, 5000.0,
                '{\"Fire\": 0.5}', '[\"Ice\"]', '{\"Fire\": 100}')",
            [],
        )
        .unwrap();

        let json = get_enemy(&conn, "e_001").unwrap();
        let enemy: EnemyRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(enemy.name, "Full Enemy");
        assert_eq!(enemy.base_res, 0.3);
        assert!(enemy.resistances.contains("Fire"));
        assert!(enemy.weaknesses.contains("Ice"));
    }

    #[test]
    fn test_get_enemy_not_found() {
        let conn = setup_conn();
        let result = get_enemy(&conn, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_save_enemy_empty_id() {
        let conn = setup_conn();
        let data = r#"{
            "enemy_id": "",
            "name": "Bad",
            "enemy_type": "Normal", "level": 1,
            "hp": 0, "def": 0, "base_res": 0, "daze_max": 0,
            "resistances": "{}", "weaknesses": "[]", "anomaly_buildup": "{}"
        }"#;
        let result = save_enemy(&conn, data);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_enemy() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO enemies (enemy_id, name) VALUES ('e_001', 'Delete Me')",
            [],
        )
        .unwrap();
        delete_enemy(&conn, "e_001").unwrap();
        let json = get_enemies(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_delete_enemy_not_found() {
        let conn = setup_conn();
        let result = delete_enemy(&conn, "nonexistent");
        assert!(result.is_err());
    }
}
