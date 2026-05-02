use rusqlite::{params, Connection};

// ── W-Engine ────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WEngineRecord {
    pub id: String,
    pub name: String,
    pub level: i32,
    pub ascension: i32,
    pub atk: f64,
    pub crit_rate: f64,
    pub crit_dmg: f64,
    pub pen_ratio: f64,
    pub energy_regen: f64,
    pub impact: f64,
    pub anomaly_mastery: f64,
    pub passive_effects: String,
}

// ── Drive Disc ──────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DriveDiscRecord {
    pub id: String,
    pub slot: i32,
    pub level: i32,
    pub set_id: String,
    pub main_stat_name: String,
    pub main_stat_value: f64,
    pub sub_stat_1_name: Option<String>,
    pub sub_stat_1_value: Option<f64>,
    pub sub_stat_2_name: Option<String>,
    pub sub_stat_2_value: Option<f64>,
    pub sub_stat_3_name: Option<String>,
    pub sub_stat_3_value: Option<f64>,
    pub sub_stat_4_name: Option<String>,
    pub sub_stat_4_value: Option<f64>,
}

// ── Disc Set ────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscSetRecord {
    pub set_id: String,
    pub name: String,
    pub two_piece_description: Option<String>,
    pub two_piece_buff_id: Option<String>,
    pub four_piece_description: Option<String>,
    pub four_piece_buff_id: Option<String>,
}

// ── Combined response ───────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AllEquipment {
    pub w_engines: Vec<WEngineRecord>,
    pub drive_discs: Vec<DriveDiscRecord>,
    pub disc_sets: Vec<DiscSetRecord>,
}

// ── W-Engine CRUD ───────────────────────────────────────

fn query_all_w_engines(conn: &Connection) -> Result<Vec<WEngineRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, level, ascension, atk, crit_rate, crit_dmg,
                    pen_ratio, energy_regen, impact, anomaly_mastery, passive_effects
             FROM w_engines ORDER BY id",
        )
        .map_err(|e| format!("Failed to prepare w_engines query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(WEngineRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                level: row.get(2)?,
                ascension: row.get(3)?,
                atk: row.get(4)?,
                crit_rate: row.get(5)?,
                crit_dmg: row.get(6)?,
                pen_ratio: row.get(7)?,
                energy_regen: row.get(8)?,
                impact: row.get(9)?,
                anomaly_mastery: row.get(10)?,
                passive_effects: row.get(11)?,
            })
        })
        .map_err(|e| format!("Failed to query w_engines: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read w_engine row: {e}"))?);
    }
    Ok(result)
}

pub fn get_w_engines(conn: &Connection) -> Result<String, String> {
    let items = query_all_w_engines(conn)?;
    serde_json::to_string(&items).map_err(|e| format!("Failed to serialize w_engines: {e}"))
}

pub fn save_w_engine(conn: &Connection, data: &str) -> Result<String, String> {
    let rec: WEngineRecord =
        serde_json::from_str(data).map_err(|e| format!("Invalid w_engine JSON: {e}"))?;

    if rec.id.is_empty() {
        return Err("id is required".to_string());
    }
    if rec.name.is_empty() {
        return Err("name is required".to_string());
    }

    conn.execute(
        "INSERT INTO w_engines (id, name, level, ascension, atk, crit_rate, crit_dmg,
            pen_ratio, energy_regen, impact, anomaly_mastery, passive_effects)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
            name=excluded.name, level=excluded.level, ascension=excluded.ascension,
            atk=excluded.atk, crit_rate=excluded.crit_rate, crit_dmg=excluded.crit_dmg,
            pen_ratio=excluded.pen_ratio, energy_regen=excluded.energy_regen,
            impact=excluded.impact, anomaly_mastery=excluded.anomaly_mastery,
            passive_effects=excluded.passive_effects,
            updated_at=datetime('now')",
        params![
            rec.id, rec.name, rec.level, rec.ascension,
            rec.atk, rec.crit_rate, rec.crit_dmg, rec.pen_ratio,
            rec.energy_regen, rec.impact, rec.anomaly_mastery, rec.passive_effects,
        ],
    )
    .map_err(|e| format!("Failed to save w_engine: {e}"))?;

    Ok(serde_json::json!({"status": "ok", "id": rec.id}).to_string())
}

pub fn delete_w_engine(conn: &Connection, id: &str) -> Result<String, String> {
    let affected = conn
        .execute("DELETE FROM w_engines WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete w_engine: {e}"))?;

    if affected == 0 {
        return Err(format!("W-Engine '{id}' not found"));
    }
    Ok(serde_json::json!({"status": "ok", "id": id}).to_string())
}

// ── Drive Disc CRUD ─────────────────────────────────────

fn query_all_drive_discs(conn: &Connection) -> Result<Vec<DriveDiscRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, slot, level, set_id,
                    main_stat_name, main_stat_value,
                    sub_stat_1_name, sub_stat_1_value,
                    sub_stat_2_name, sub_stat_2_value,
                    sub_stat_3_name, sub_stat_3_value,
                    sub_stat_4_name, sub_stat_4_value
             FROM drive_discs ORDER BY id",
        )
        .map_err(|e| format!("Failed to prepare drive_discs query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DriveDiscRecord {
                id: row.get(0)?,
                slot: row.get(1)?,
                level: row.get(2)?,
                set_id: row.get(3)?,
                main_stat_name: row.get(4)?,
                main_stat_value: row.get(5)?,
                sub_stat_1_name: row.get(6)?,
                sub_stat_1_value: row.get(7)?,
                sub_stat_2_name: row.get(8)?,
                sub_stat_2_value: row.get(9)?,
                sub_stat_3_name: row.get(10)?,
                sub_stat_3_value: row.get(11)?,
                sub_stat_4_name: row.get(12)?,
                sub_stat_4_value: row.get(13)?,
            })
        })
        .map_err(|e| format!("Failed to query drive_discs: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read drive_disc row: {e}"))?);
    }
    Ok(result)
}

pub fn get_drive_discs(conn: &Connection) -> Result<String, String> {
    let items = query_all_drive_discs(conn)?;
    serde_json::to_string(&items).map_err(|e| format!("Failed to serialize drive_discs: {e}"))
}

pub fn save_drive_disc(conn: &Connection, data: &str) -> Result<String, String> {
    let rec: DriveDiscRecord =
        serde_json::from_str(data).map_err(|e| format!("Invalid drive_disc JSON: {e}"))?;

    if rec.id.is_empty() {
        return Err("id is required".to_string());
    }
    if !(1..=6).contains(&rec.slot) {
        return Err(format!("slot must be 1-6, got {}", rec.slot));
    }
    if rec.set_id.is_empty() {
        return Err("set_id is required".to_string());
    }

    conn.execute(
        "INSERT INTO drive_discs (id, slot, level, set_id,
            main_stat_name, main_stat_value,
            sub_stat_1_name, sub_stat_1_value,
            sub_stat_2_name, sub_stat_2_value,
            sub_stat_3_name, sub_stat_3_value,
            sub_stat_4_name, sub_stat_4_value)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(id) DO UPDATE SET
            slot=excluded.slot, level=excluded.level, set_id=excluded.set_id,
            main_stat_name=excluded.main_stat_name, main_stat_value=excluded.main_stat_value,
            sub_stat_1_name=excluded.sub_stat_1_name, sub_stat_1_value=excluded.sub_stat_1_value,
            sub_stat_2_name=excluded.sub_stat_2_name, sub_stat_2_value=excluded.sub_stat_2_value,
            sub_stat_3_name=excluded.sub_stat_3_name, sub_stat_3_value=excluded.sub_stat_3_value,
            sub_stat_4_name=excluded.sub_stat_4_name, sub_stat_4_value=excluded.sub_stat_4_value,
            updated_at=datetime('now')",
        params![
            rec.id, rec.slot, rec.level, rec.set_id,
            rec.main_stat_name, rec.main_stat_value,
            rec.sub_stat_1_name, rec.sub_stat_1_value,
            rec.sub_stat_2_name, rec.sub_stat_2_value,
            rec.sub_stat_3_name, rec.sub_stat_3_value,
            rec.sub_stat_4_name, rec.sub_stat_4_value,
        ],
    )
    .map_err(|e| format!("Failed to save drive_disc: {e}"))?;

    Ok(serde_json::json!({"status": "ok", "id": rec.id}).to_string())
}

pub fn delete_drive_disc(conn: &Connection, id: &str) -> Result<String, String> {
    let affected = conn
        .execute("DELETE FROM drive_discs WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete drive_disc: {e}"))?;

    if affected == 0 {
        return Err(format!("Drive Disc '{id}' not found"));
    }
    Ok(serde_json::json!({"status": "ok", "id": id}).to_string())
}

// ── Disc Set CRUD ───────────────────────────────────────

fn query_all_disc_sets(conn: &Connection) -> Result<Vec<DiscSetRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT set_id, name, two_piece_description, two_piece_buff_id,
                    four_piece_description, four_piece_buff_id
             FROM disc_sets ORDER BY set_id",
        )
        .map_err(|e| format!("Failed to prepare disc_sets query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DiscSetRecord {
                set_id: row.get(0)?,
                name: row.get(1)?,
                two_piece_description: row.get(2)?,
                two_piece_buff_id: row.get(3)?,
                four_piece_description: row.get(4)?,
                four_piece_buff_id: row.get(5)?,
            })
        })
        .map_err(|e| format!("Failed to query disc_sets: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read disc_set row: {e}"))?);
    }
    Ok(result)
}

pub fn get_disc_sets(conn: &Connection) -> Result<String, String> {
    let items = query_all_disc_sets(conn)?;
    serde_json::to_string(&items).map_err(|e| format!("Failed to serialize disc_sets: {e}"))
}

pub fn save_disc_set(conn: &Connection, data: &str) -> Result<String, String> {
    let rec: DiscSetRecord =
        serde_json::from_str(data).map_err(|e| format!("Invalid disc_set JSON: {e}"))?;

    if rec.set_id.is_empty() {
        return Err("set_id is required".to_string());
    }
    if rec.name.is_empty() {
        return Err("name is required".to_string());
    }

    conn.execute(
        "INSERT INTO disc_sets (set_id, name, two_piece_description, two_piece_buff_id,
            four_piece_description, four_piece_buff_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(set_id) DO UPDATE SET
            name=excluded.name,
            two_piece_description=excluded.two_piece_description,
            two_piece_buff_id=excluded.two_piece_buff_id,
            four_piece_description=excluded.four_piece_description,
            four_piece_buff_id=excluded.four_piece_buff_id,
            updated_at=datetime('now')",
        params![
            rec.set_id, rec.name,
            rec.two_piece_description, rec.two_piece_buff_id,
            rec.four_piece_description, rec.four_piece_buff_id,
        ],
    )
    .map_err(|e| format!("Failed to save disc_set: {e}"))?;

    Ok(serde_json::json!({"status": "ok", "set_id": rec.set_id}).to_string())
}

pub fn delete_disc_set(conn: &Connection, set_id: &str) -> Result<String, String> {
    let affected = conn
        .execute("DELETE FROM disc_sets WHERE set_id = ?1", params![set_id])
        .map_err(|e| format!("Failed to delete disc_set: {e}"))?;

    if affected == 0 {
        return Err(format!("Disc Set '{set_id}' not found"));
    }
    Ok(serde_json::json!({"status": "ok", "set_id": set_id}).to_string())
}

// ── Combined query ──────────────────────────────────────

pub fn get_all_equipment(conn: &Connection) -> Result<String, String> {
    let w_engines = query_all_w_engines(conn)?;
    let drive_discs = query_all_drive_discs(conn)?;
    let disc_sets = query_all_disc_sets(conn)?;
    let result = AllEquipment {
        w_engines,
        drive_discs,
        disc_sets,
    };
    serde_json::to_string(&result).map_err(|e| format!("Failed to serialize equipment: {e}"))
}

// ── Tauri command wrappers ──────────────────────────────

pub fn cmd_get_all_equipment(conn: &Connection) -> Result<String, String> {
    get_all_equipment(conn)
}

pub fn cmd_get_w_engines(conn: &Connection) -> Result<String, String> {
    get_w_engines(conn)
}

pub fn cmd_save_w_engine(conn: &Connection, data: String) -> Result<String, String> {
    save_w_engine(conn, &data)
}

pub fn cmd_delete_w_engine(conn: &Connection, id: String) -> Result<String, String> {
    delete_w_engine(conn, &id)
}

pub fn cmd_get_drive_discs(conn: &Connection) -> Result<String, String> {
    get_drive_discs(conn)
}

pub fn cmd_save_drive_disc(conn: &Connection, data: String) -> Result<String, String> {
    save_drive_disc(conn, &data)
}

pub fn cmd_delete_drive_disc(conn: &Connection, id: String) -> Result<String, String> {
    delete_drive_disc(conn, &id)
}

pub fn cmd_get_disc_sets(conn: &Connection) -> Result<String, String> {
    get_disc_sets(conn)
}

pub fn cmd_save_disc_set(conn: &Connection, data: String) -> Result<String, String> {
    save_disc_set(conn, &data)
}

pub fn cmd_delete_disc_set(conn: &Connection, set_id: String) -> Result<String, String> {
    delete_disc_set(conn, &set_id)
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

    // ── W-Engine tests ─────────────────────────────────

    #[test]
    fn test_get_w_engines_empty() {
        let conn = setup_conn();
        let json = get_w_engines(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_save_and_get_w_engine() {
        let conn = setup_conn();
        let data = r#"{
            "id": "we_001",
            "name": "Test Engine",
            "level": 60,
            "ascension": 6,
            "atk": 700.0,
            "crit_rate": 0.24,
            "crit_dmg": 0.0,
            "pen_ratio": 0.0,
            "energy_regen": 0.0,
            "impact": 0.0,
            "anomaly_mastery": 0.0,
            "passive_effects": "[\"effect_1\"]"
        }"#;
        save_w_engine(&conn, data).unwrap();
        let json = get_w_engines(&conn).unwrap();
        let items: Vec<WEngineRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "we_001");
        assert_eq!(items[0].atk, 700.0);
    }

    #[test]
    fn test_delete_w_engine() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO w_engines (id, name) VALUES ('we_001', 'Test')",
            [],
        )
        .unwrap();
        delete_w_engine(&conn, "we_001").unwrap();
        let json = get_w_engines(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_delete_w_engine_not_found() {
        let conn = setup_conn();
        let result = delete_w_engine(&conn, "nonexistent");
        assert!(result.is_err());
    }

    // ── Drive Disc tests ────────────────────────────────

    #[test]
    fn test_get_drive_discs_empty() {
        let conn = setup_conn();
        let json = get_drive_discs(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_save_and_get_drive_disc() {
        let conn = setup_conn();
        // First insert a disc set for FK reference (even though no FK in schema)
        conn.execute(
            "INSERT INTO disc_sets (set_id, name) VALUES ('set_01', 'Test Set')",
            [],
        )
        .unwrap();

        let data = r#"{
            "id": "dd_001",
            "slot": 1,
            "level": 15,
            "set_id": "set_01",
            "main_stat_name": "HP",
            "main_stat_value": 1000.0,
            "sub_stat_1_name": "ATK",
            "sub_stat_1_value": 50.0,
            "sub_stat_2_name": null,
            "sub_stat_2_value": null,
            "sub_stat_3_name": null,
            "sub_stat_3_value": null,
            "sub_stat_4_name": null,
            "sub_stat_4_value": null
        }"#;
        save_drive_disc(&conn, data).unwrap();
        let json = get_drive_discs(&conn).unwrap();
        let items: Vec<DriveDiscRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "dd_001");
        assert_eq!(items[0].slot, 1);
    }

    #[test]
    fn test_save_drive_disc_invalid_slot() {
        let conn = setup_conn();
        let data = r#"{
            "id": "bad",
            "slot": 7,
            "level": 15,
            "set_id": "set_01",
            "main_stat_name": "HP",
            "main_stat_value": 1000.0,
            "sub_stat_1_name": null,
            "sub_stat_1_value": null,
            "sub_stat_2_name": null,
            "sub_stat_2_value": null,
            "sub_stat_3_name": null,
            "sub_stat_3_value": null,
            "sub_stat_4_name": null,
            "sub_stat_4_value": null
        }"#;
        let result = save_drive_disc(&conn, data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("slot must be 1-6"));
    }

    #[test]
    fn test_delete_drive_disc() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO drive_discs (id, slot, set_id, main_stat_name, main_stat_value)
             VALUES ('dd_001', 1, 'set_01', 'HP', 1000.0)",
            [],
        )
        .unwrap();
        delete_drive_disc(&conn, "dd_001").unwrap();
        let json = get_drive_discs(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    // ── Disc Set tests ──────────────────────────────────

    #[test]
    fn test_get_disc_sets_empty() {
        let conn = setup_conn();
        let json = get_disc_sets(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_save_and_get_disc_set() {
        let conn = setup_conn();
        let data = r#"{
            "set_id": "ds_001",
            "name": "Test Set",
            "two_piece_description": "+10% ATK",
            "two_piece_buff_id": "atk_buff_10",
            "four_piece_description": "+25% Crit DMG",
            "four_piece_buff_id": "crit_dmg_25"
        }"#;
        save_disc_set(&conn, data).unwrap();
        let json = get_disc_sets(&conn).unwrap();
        let items: Vec<DiscSetRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].set_id, "ds_001");
        assert_eq!(items[0].four_piece_buff_id.as_deref(), Some("crit_dmg_25"));
    }

    #[test]
    fn test_delete_disc_set() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO disc_sets (set_id, name) VALUES ('ds_001', 'Test')",
            [],
        )
        .unwrap();
        delete_disc_set(&conn, "ds_001").unwrap();
        let json = get_disc_sets(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    // ── Combined ────────────────────────────────────────

    #[test]
    fn test_get_all_equipment_empty() {
        let conn = setup_conn();
        let json = get_all_equipment(&conn).unwrap();
        let all: AllEquipment = serde_json::from_str(&json).unwrap();
        assert_eq!(all.w_engines.len(), 0);
        assert_eq!(all.drive_discs.len(), 0);
        assert_eq!(all.disc_sets.len(), 0);
    }

    #[test]
    fn test_get_all_equipment_with_data() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO w_engines (id, name) VALUES ('we_001', 'Engine')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO disc_sets (set_id, name) VALUES ('ds_001', 'Set')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO drive_discs (id, slot, set_id, main_stat_name, main_stat_value)
             VALUES ('dd_001', 1, 'ds_001', 'HP', 1000.0)",
            [],
        )
        .unwrap();

        let json = get_all_equipment(&conn).unwrap();
        let all: AllEquipment = serde_json::from_str(&json).unwrap();
        assert_eq!(all.w_engines.len(), 1);
        assert_eq!(all.drive_discs.len(), 1);
        assert_eq!(all.disc_sets.len(), 1);
    }
}
