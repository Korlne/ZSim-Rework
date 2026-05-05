use rusqlite::{params, Connection};

/// Flat character record matching the `characters` table columns.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CharacterRecord {
    pub char_id: String,
    pub name: String,
    pub faction: String,
    pub specialty: String,
    pub element: String,
    pub level: i32,
    pub ascension: i32,
    pub hp: f64,
    pub atk: f64,
    pub def: f64,
    pub impact: f64,
    pub crit_rate: f64,
    pub crit_dmg: f64,
    pub pen_ratio: f64,
    pub pen_fixed: f64,
    pub anomaly_mastery: f64,
    pub anomaly_proficiency: f64,
    pub energy_regen: f64,
    pub energy_gen_rate: f64,
    pub constellations: String,
    pub potentials: String,
    pub action_dict: String,
}

/// A skill record summary included when fetching a single character.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillSummary {
    pub id: i64,
    pub action_id: String,
    pub action_type: String,
    pub energy_cost: f64,
    pub cooldown_ticks: i32,
}

/// Character with associated skills (used by `get_character`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CharacterWithSkills {
    pub character: CharacterRecord,
    pub skills: Vec<SkillSummary>,
}

// ── Validation ───────────────────────────────────────────

/// Validate a char_id: only lowercase letters, digits, underscores allowed.
fn validate_char_id(char_id: &str) -> Result<(), String> {
    if char_id.is_empty() {
        return Err("char_id is required".to_string());
    }
    if !char_id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!(
            "char_id '{char_id}' contains invalid characters. Only lowercase letters, digits, and underscores are allowed."
        ));
    }
    Ok(())
}

/// Validate basic numeric constraints on a character record.
fn validate_character_record(rec: &CharacterRecord) -> Result<(), String> {
    validate_char_id(&rec.char_id)?;

    if rec.name.is_empty() {
        return Err("name is required".to_string());
    }
    if !(1..=60).contains(&rec.level) {
        return Err(format!("level {} out of range (1-60)", rec.level));
    }
    if !(0..=6).contains(&rec.ascension) {
        return Err(format!("ascension {} out of range (0-6)", rec.ascension));
    }

    for (field, val) in [
        ("crit_rate", rec.crit_rate),
        ("crit_dmg", rec.crit_dmg),
        ("pen_ratio", rec.pen_ratio),
    ] {
        if !(0.0..=1.0).contains(&val) {
            return Err(format!("{field} {val} out of range (0.0-1.0)"));
        }
    }

    for (field, val) in [
        ("hp", rec.hp),
        ("atk", rec.atk),
        ("def", rec.def),
        ("impact", rec.impact),
        ("pen_fixed", rec.pen_fixed),
        ("anomaly_mastery", rec.anomaly_mastery),
        ("anomaly_proficiency", rec.anomaly_proficiency),
        ("energy_regen", rec.energy_regen),
        ("energy_gen_rate", rec.energy_gen_rate),
    ] {
        if val < 0.0 {
            return Err(format!("{field} must be >= 0, got {val}"));
        }
    }

    Ok(())
}

// ── CRUD ─────────────────────────────────────────────────

/// Select all rows from the characters table, ordered by char_id.
fn query_all_characters(conn: &Connection) -> Result<Vec<CharacterRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT char_id, name, faction, specialty, element, level, ascension,
                    hp, atk, def, impact, crit_rate, crit_dmg,
                    pen_ratio, pen_fixed, anomaly_mastery, anomaly_proficiency,
                    energy_regen, energy_gen_rate, constellations, potentials, action_dict
             FROM characters
             ORDER BY char_id",
        )
        .map_err(|e| format!("Failed to prepare query: {e}"))?;

    let rows = stmt
        .query_map([], row_to_character)
        .map_err(|e| format!("Failed to query characters: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read character row: {e}))"))?);
    }
    Ok(result)
}

/// Map a SQLite row to a CharacterRecord.
fn row_to_character(row: &rusqlite::Row) -> rusqlite::Result<CharacterRecord> {
    Ok(CharacterRecord {
        char_id: row.get(0)?,
        name: row.get(1)?,
        faction: row.get(2)?,
        specialty: row.get(3)?,
        element: row.get(4)?,
        level: row.get(5)?,
        ascension: row.get(6)?,
        hp: row.get(7)?,
        atk: row.get(8)?,
        def: row.get(9)?,
        impact: row.get(10)?,
        crit_rate: row.get(11)?,
        crit_dmg: row.get(12)?,
        pen_ratio: row.get(13)?,
        pen_fixed: row.get(14)?,
        anomaly_mastery: row.get(15)?,
        anomaly_proficiency: row.get(16)?,
        energy_regen: row.get(17)?,
        energy_gen_rate: row.get(18)?,
        constellations: row.get(19)?,
        potentials: row.get(20)?,
        action_dict: row.get(21)?,
    })
}

/// Get all characters as a JSON string.
pub fn get_characters(conn: &Connection) -> Result<String, String> {
    let chars = query_all_characters(conn)?;
    serde_json::to_string(&chars).map_err(|e| format!("Failed to serialize characters: {e}"))
}

/// Get a single character with its associated skills as a JSON string.
pub fn get_character(conn: &Connection, char_id: &str) -> Result<String, String> {
    let character: CharacterRecord = conn
        .query_row(
            "SELECT char_id, name, faction, specialty, element, level, ascension,
                    hp, atk, def, impact, crit_rate, crit_dmg,
                    pen_ratio, pen_fixed, anomaly_mastery, anomaly_proficiency,
                    energy_regen, energy_gen_rate, constellations, potentials, action_dict
             FROM characters WHERE char_id = ?1",
            params![char_id],
            row_to_character,
        )
        .map_err(|e| format!("Character '{char_id}' not found: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, action_id, action_type, energy_cost, cooldown_ticks
             FROM skills WHERE char_id = ?1 ORDER BY action_id",
        )
        .map_err(|e| format!("Failed to prepare skills query: {e}"))?;

    let skills: Vec<SkillSummary> = stmt
        .query_map(params![char_id], |row| {
            Ok(SkillSummary {
                id: row.get(0)?,
                action_id: row.get(1)?,
                action_type: row.get(2)?,
                energy_cost: row.get(3)?,
                cooldown_ticks: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to query skills: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let result = CharacterWithSkills { character, skills };
    serde_json::to_string(&result).map_err(|e| format!("Failed to serialize: {e}"))
}

/// Create or update a character. Accepts a JSON string matching CharacterRecord.
/// Returns JSON with the saved char_id.
pub fn save_character(conn: &Connection, data: &str) -> Result<String, String> {
    let rec: CharacterRecord =
        serde_json::from_str(data).map_err(|e| format!("Invalid character JSON: {e}"))?;

    validate_character_record(&rec)?;

    conn.execute(
        "INSERT INTO characters (char_id, name, faction, specialty, element, level, ascension,
            hp, atk, def, impact, crit_rate, crit_dmg, pen_ratio, pen_fixed,
            anomaly_mastery, anomaly_proficiency, energy_regen, energy_gen_rate,
            constellations, potentials, action_dict)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                 ?16, ?17, ?18, ?19, ?20, ?21, ?22)
         ON CONFLICT(char_id) DO UPDATE SET
            name=excluded.name, faction=excluded.faction, specialty=excluded.specialty,
            element=excluded.element, level=excluded.level, ascension=excluded.ascension,
            hp=excluded.hp, atk=excluded.atk, def=excluded.def, impact=excluded.impact,
            crit_rate=excluded.crit_rate, crit_dmg=excluded.crit_dmg,
            pen_ratio=excluded.pen_ratio, pen_fixed=excluded.pen_fixed,
            anomaly_mastery=excluded.anomaly_mastery, anomaly_proficiency=excluded.anomaly_proficiency,
            energy_regen=excluded.energy_regen, energy_gen_rate=excluded.energy_gen_rate,
            constellations=excluded.constellations, potentials=excluded.potentials,
            action_dict=excluded.action_dict,
            updated_at=datetime('now')",
        params![
            rec.char_id, rec.name, rec.faction, rec.specialty, rec.element,
            rec.level, rec.ascension, rec.hp, rec.atk, rec.def, rec.impact,
            rec.crit_rate, rec.crit_dmg, rec.pen_ratio, rec.pen_fixed,
            rec.anomaly_mastery, rec.anomaly_proficiency, rec.energy_regen, rec.energy_gen_rate,
            rec.constellations, rec.potentials, rec.action_dict
        ],
    )
    .map_err(|e| format!("Failed to save character: {e}"))?;

    Ok(serde_json::json!({"status": "ok", "char_id": rec.char_id}).to_string())
}

/// Delete a character by char_id. Skills and skill_multipliers are cascade-deleted.
pub fn delete_character(conn: &Connection, char_id: &str) -> Result<String, String> {
    let affected = conn
        .execute("DELETE FROM characters WHERE char_id = ?1", params![char_id])
        .map_err(|e| format!("Failed to delete character: {e}"))?;

    if affected == 0 {
        return Err(format!("Character '{char_id}' not found"));
    }

    Ok(serde_json::json!({"status": "ok", "char_id": char_id}).to_string())
}

// ── Tauri command wrappers (called from lib.rs) ──────────

/// Tauri command: get all characters as JSON array.
pub fn cmd_get_characters(conn: &Connection) -> Result<String, String> {
    get_characters(conn)
}

/// Tauri command: get a single character with skills as JSON.
pub fn cmd_get_character(conn: &Connection, char_id: String) -> Result<String, String> {
    get_character(conn, &char_id)
}

/// Tauri command: save (insert or update) a character from JSON string.
pub fn cmd_save_character(conn: &Connection, data: String) -> Result<String, String> {
    save_character(conn, &data)
}

/// Tauri command: delete a character by char_id.
pub fn cmd_delete_character(conn: &Connection, char_id: String) -> Result<String, String> {
    delete_character(conn, &char_id)
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

    #[test]
    fn test_get_characters_empty() {
        let conn = setup_conn();
        let json = get_characters(&conn).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_get_characters_single() {
        let conn = setup_conn();
        insert_test_character(&conn, "test_01", "Test One");
        let json = get_characters(&conn).unwrap();
        let chars: Vec<CharacterRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].char_id, "test_01");
        assert_eq!(chars[0].name, "Test One");
    }

    #[test]
    fn test_get_characters_multiple_ordered() {
        let conn = setup_conn();
        insert_test_character(&conn, "b_char", "B");
        insert_test_character(&conn, "a_char", "A");
        let json = get_characters(&conn).unwrap();
        let chars: Vec<CharacterRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].char_id, "a_char"); // ordered by char_id
        assert_eq!(chars[1].char_id, "b_char");
    }

    #[test]
    fn test_get_character_not_found() {
        let conn = setup_conn();
        let result = get_character(&conn, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_get_character_with_skills() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Char 1");
        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type, energy_cost, cooldown_ticks)
             VALUES ('c1', 'Normal_1', 'Normal', 0, 0)",
            [],
        )
        .unwrap();

        let json = get_character(&conn, "c1").unwrap();
        let result: CharacterWithSkills = serde_json::from_str(&json).unwrap();
        assert_eq!(result.character.char_id, "c1");
        assert_eq!(result.skills.len(), 1);
        assert_eq!(result.skills[0].action_id, "Normal_1");
    }

    #[test]
    fn test_save_character_new() {
        let conn = setup_conn();
        let data = r#"{
            "char_id": "new_char",
            "name": "New Character",
            "faction": "Gentle_House",
            "specialty": "Stun",
            "element": "Electric",
            "level": 60,
            "ascension": 6,
            "hp": 9500.0,
            "atk": 1100.0,
            "def": 550.0,
            "impact": 120.0,
            "crit_rate": 0.15,
            "crit_dmg": 0.80,
            "pen_ratio": 0.10,
            "pen_fixed": 40.0,
            "anomaly_mastery": 80.0,
            "anomaly_proficiency": 95.0,
            "energy_regen": 1.2,
            "energy_gen_rate": 0.3,
            "constellations": "[false,false,false,false,false,false]",
            "potentials": "[false,false,false,false,false,false]",
            "action_dict": "[]"
        }"#;
        let result = save_character(&conn, data).unwrap();
        assert!(result.contains("new_char"));

        // Verify it was inserted
        let json = get_characters(&conn).unwrap();
        let chars: Vec<CharacterRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "New Character");
        assert_eq!(chars[0].hp, 9500.0);
        assert_eq!(chars[0].crit_rate, 0.15);
    }

    #[test]
    fn test_save_character_update() {
        let conn = setup_conn();
        insert_test_character(&conn, "upd_char", "Original");

        let data = r#"{
            "char_id": "upd_char",
            "name": "Updated Name",
            "faction": "Gentle_House",
            "specialty": "Attack",
            "element": "Fire",
            "level": 60,
            "ascension": 6,
            "hp": 10000.0,
            "atk": 1200.0,
            "def": 500.0,
            "impact": 100.0,
            "crit_rate": 0.20,
            "crit_dmg": 1.00,
            "pen_ratio": 0.05,
            "pen_fixed": 0.0,
            "anomaly_mastery": 0.0,
            "anomaly_proficiency": 0.0,
            "energy_regen": 0.0,
            "energy_gen_rate": 0.0,
            "constellations": "[true,false,false,false,false,false]",
            "potentials": "[false,false,false,false,false,false]",
            "action_dict": "[\"action_1\"]"
        }"#;
        save_character(&conn, data).unwrap();

        let json = get_character(&conn, "upd_char").unwrap();
        let result: CharacterWithSkills = serde_json::from_str(&json).unwrap();
        assert_eq!(result.character.name, "Updated Name");
        assert_eq!(result.character.element, "Fire");
        assert_eq!(result.character.crit_rate, 0.20);
    }

    #[test]
    fn test_save_character_validation_empty_id() {
        let conn = setup_conn();
        let data = r#"{"char_id":"","name":"Bad","faction":"X","specialty":"Attack","element":"Fire","level":60,"ascension":6,"hp":0,"atk":0,"def":0,"impact":0,"crit_rate":0,"crit_dmg":0,"pen_ratio":0,"pen_fixed":0,"anomaly_mastery":0,"anomaly_proficiency":0,"energy_regen":0,"energy_gen_rate":0,"constellations":"[]","potentials":"[]","action_dict":"[]"}"#;
        let result = save_character(&conn, data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("char_id is required"));
    }

    #[test]
    fn test_save_character_validation_invalid_id() {
        let conn = setup_conn();
        let data = r#"{"char_id":"UPPERCASE","name":"Bad","faction":"X","specialty":"Attack","element":"Fire","level":60,"ascension":6,"hp":0,"atk":0,"def":0,"impact":0,"crit_rate":0,"crit_dmg":0,"pen_ratio":0,"pen_fixed":0,"anomaly_mastery":0,"anomaly_proficiency":0,"energy_regen":0,"energy_gen_rate":0,"constellations":"[]","potentials":"[]","action_dict":"[]"}"#;
        let result = save_character(&conn, data);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_character_validation_level_range() {
        let conn = setup_conn();
        let data = r#"{"char_id":"bad_lvl","name":"Bad","faction":"X","specialty":"Attack","element":"Fire","level":99,"ascension":6,"hp":0,"atk":0,"def":0,"impact":0,"crit_rate":0,"crit_dmg":0,"pen_ratio":0,"pen_fixed":0,"anomaly_mastery":0,"anomaly_proficiency":0,"energy_regen":0,"energy_gen_rate":0,"constellations":"[]","potentials":"[]","action_dict":"[]"}"#;
        let result = save_character(&conn, data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("range"));
    }

    #[test]
    fn test_save_character_validation_crit_range() {
        let conn = setup_conn();
        let data = r#"{"char_id":"bad_crit","name":"Bad","faction":"X","specialty":"Attack","element":"Fire","level":60,"ascension":6,"hp":0,"atk":0,"def":0,"impact":0,"crit_rate":1.5,"crit_dmg":0,"pen_ratio":0,"pen_fixed":0,"anomaly_mastery":0,"anomaly_proficiency":0,"energy_regen":0,"energy_gen_rate":0,"constellations":"[]","potentials":"[]","action_dict":"[]"}"#;
        let result = save_character(&conn, data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("crit_rate"));
    }

    #[test]
    fn test_delete_character() {
        let conn = setup_conn();
        insert_test_character(&conn, "del_me", "Delete Me");
        assert_eq!(
            query_all_characters(&conn).unwrap().len(),
            1,
            "should have 1 character before delete"
        );

        delete_character(&conn, "del_me").unwrap();
        assert_eq!(
            query_all_characters(&conn).unwrap().len(),
            0,
            "should be empty after delete"
        );
    }

    #[test]
    fn test_delete_character_not_found() {
        let conn = setup_conn();
        let result = delete_character(&conn, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_delete_character_cascade_skills() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1", "Cascade Test");
        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type)
             VALUES ('c1', 'skill_1', 'Normal')",
            [],
        )
        .unwrap();

        let skill_id: i64 = conn
            .query_row(
                "SELECT id FROM skills WHERE char_id = 'c1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier)
             VALUES (?1, 1, 10, 0.5)",
            params![skill_id],
        )
        .unwrap();

        // Delete character — should cascade
        delete_character(&conn, "c1").unwrap();

        let skill_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skills WHERE char_id = 'c1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(skill_count, 0);
    }

    #[test]
    fn test_validate_char_id_valid() {
        assert!(validate_char_id("anby_demara").is_ok());
        assert!(validate_char_id("char_001").is_ok());
        assert!(validate_char_id("a").is_ok());
    }

    #[test]
    fn test_validate_char_id_invalid() {
        assert!(validate_char_id("").is_err());
        assert!(validate_char_id("UpperCase").is_err());
        assert!(validate_char_id("has space").is_err());
        assert!(validate_char_id("has-dash").is_err());
    }
}
