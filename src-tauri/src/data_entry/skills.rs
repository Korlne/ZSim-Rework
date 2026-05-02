use rusqlite::{params, Connection};

/// Flat skill record matching the `skills` table columns.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillRecord {
    pub id: Option<i64>,
    pub char_id: String,
    pub action_id: String,
    pub action_type: String,
    pub daze_multiplier: f64,
    pub energy_cost: f64,
    pub decibel_cost: f64,
    pub hp_cost: f64,
    pub cooldown_ticks: i32,
    pub animation_frames: i32,
    pub interruptible_frame: Option<i32>,
    pub is_snapshot: bool,
    pub prerequisite_action_id: Option<String>,
    pub effect_id: Option<String>,
}

/// A multiplier segment for a skill.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultiplierRecord {
    pub segment_index: i32,
    pub frame: i32,
    pub multiplier: f64,
    pub decay_coeff: Option<f64>,
}

/// Skill with embedded multipliers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillWithMultipliers {
    pub skill: SkillRecord,
    pub multipliers: Vec<MultiplierRecord>,
}

// ── Validation ───────────────────────────────────────────

fn validate_skill_record(rec: &SkillRecord) -> Result<(), String> {
    if rec.char_id.is_empty() {
        return Err("char_id is required".to_string());
    }
    if rec.action_id.is_empty() {
        return Err("action_id is required".to_string());
    }
    if rec.action_type.is_empty() {
        return Err("action_type is required".to_string());
    }
    if rec.cooldown_ticks < 0 {
        return Err(format!("cooldown_ticks must be >= 0, got {}", rec.cooldown_ticks));
    }
    if rec.animation_frames < 0 {
        return Err(format!("animation_frames must be >= 0, got {}", rec.animation_frames));
    }
    Ok(())
}

// ── CRUD ─────────────────────────────────────────────────

/// Select all skills for a character, each with embedded multipliers.
fn query_skills_for_character(conn: &Connection, char_id: &str) -> Result<Vec<SkillWithMultipliers>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, char_id, action_id, action_type,
                    daze_multiplier, energy_cost, decibel_cost, hp_cost,
                    cooldown_ticks, animation_frames, interruptible_frame,
                    is_snapshot, prerequisite_action_id, effect_id
             FROM skills
             WHERE char_id = ?1
             ORDER BY action_id",
        )
        .map_err(|e| format!("Failed to prepare skills query: {e}"))?;

    let skill_rows = stmt
        .query_map(params![char_id], row_to_skill)
        .map_err(|e| format!("Failed to query skills: {e}"))?;

    let mut result = Vec::new();
    for row in skill_rows {
        let skill = row.map_err(|e| format!("Failed to read skill row: {e}"))?;
        let skill_id = skill.id.unwrap_or(0);
        let multipliers = query_multipliers_for_skill(conn, skill_id)?;
        result.push(SkillWithMultipliers { skill, multipliers });
    }
    Ok(result)
}

/// Select all multiplier segments for a skill, ordered by segment_index.
fn query_multipliers_for_skill(conn: &Connection, skill_id: i64) -> Result<Vec<MultiplierRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT segment_index, frame, multiplier, decay_coeff
             FROM skill_multipliers
             WHERE skill_id = ?1
             ORDER BY segment_index",
        )
        .map_err(|e| format!("Failed to prepare multipliers query: {e}"))?;

    let rows = stmt
        .query_map(params![skill_id], |row| {
            Ok(MultiplierRecord {
                segment_index: row.get(0)?,
                frame: row.get(1)?,
                multiplier: row.get(2)?,
                decay_coeff: row.get(3)?,
            })
        })
        .map_err(|e| format!("Failed to query multipliers: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read multiplier row: {e}"))?);
    }
    Ok(result)
}

/// Map a SQLite row to a SkillRecord.
fn row_to_skill(row: &rusqlite::Row) -> rusqlite::Result<SkillRecord> {
    Ok(SkillRecord {
        id: Some(row.get(0)?),
        char_id: row.get(1)?,
        action_id: row.get(2)?,
        action_type: row.get(3)?,
        daze_multiplier: row.get(4)?,
        energy_cost: row.get(5)?,
        decibel_cost: row.get(6)?,
        hp_cost: row.get(7)?,
        cooldown_ticks: row.get(8)?,
        animation_frames: row.get(9)?,
        interruptible_frame: row.get(10)?,
        is_snapshot: row.get::<_, i32>(11)? != 0,
        prerequisite_action_id: row.get(12)?,
        effect_id: row.get(13)?,
    })
}

/// Get all skills for a character with embedded multipliers as a JSON string.
pub fn get_skills(conn: &Connection, char_id: &str) -> Result<String, String> {
    let skills = query_skills_for_character(conn, char_id)?;
    serde_json::to_string(&skills).map_err(|e| format!("Failed to serialize skills: {e}"))
}

/// Save a skill with its multipliers.
///
/// Accepts a JSON string matching SkillWithMultipliers structure.
/// Uses a transaction to atomically upsert the skill and replace multipliers.
pub fn save_skill(conn: &Connection, data: &str) -> Result<String, String> {
    let input: SkillWithMultipliers =
        serde_json::from_str(data).map_err(|e| format!("Invalid skill JSON: {e}"))?;

    validate_skill_record(&input.skill)?;

    // Use a transaction for atomicity
    conn.execute_batch("BEGIN TRANSACTION")
        .map_err(|e| format!("Failed to begin transaction: {e}"))?;

    let result = (|| -> Result<String, String> {
        // Insert or update the skill record
        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type,
                daze_multiplier, energy_cost, decibel_cost, hp_cost,
                cooldown_ticks, animation_frames, interruptible_frame,
                is_snapshot, prerequisite_action_id, effect_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(char_id, action_id) DO UPDATE SET
                action_type=excluded.action_type,
                daze_multiplier=excluded.daze_multiplier,
                energy_cost=excluded.energy_cost,
                decibel_cost=excluded.decibel_cost,
                hp_cost=excluded.hp_cost,
                cooldown_ticks=excluded.cooldown_ticks,
                animation_frames=excluded.animation_frames,
                interruptible_frame=excluded.interruptible_frame,
                is_snapshot=excluded.is_snapshot,
                prerequisite_action_id=excluded.prerequisite_action_id,
                effect_id=excluded.effect_id,
                updated_at=datetime('now')",
            params![
                input.skill.char_id,
                input.skill.action_id,
                input.skill.action_type,
                input.skill.daze_multiplier,
                input.skill.energy_cost,
                input.skill.decibel_cost,
                input.skill.hp_cost,
                input.skill.cooldown_ticks,
                input.skill.animation_frames,
                input.skill.interruptible_frame,
                input.skill.is_snapshot as i32,
                input.skill.prerequisite_action_id,
                input.skill.effect_id,
            ],
        )
        .map_err(|e| format!("Failed to save skill: {e}"))?;

        // Get the skill_id (last_insert_rowid works for INSERT OR REPLACE)
        let skill_id: i64 = conn
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
            .map_err(|e| format!("Failed to get skill id: {e}"))?;

        // Delete existing multipliers for this skill
        conn.execute(
            "DELETE FROM skill_multipliers WHERE skill_id = ?1",
            params![skill_id],
        )
        .map_err(|e| format!("Failed to clear existing multipliers: {e}"))?;

        // Insert new multipliers
        for mult in &input.multipliers {
            conn.execute(
                "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier, decay_coeff)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    skill_id,
                    mult.segment_index,
                    mult.frame,
                    mult.multiplier,
                    mult.decay_coeff,
                ],
            )
            .map_err(|e| format!("Failed to insert multiplier: {e}"))?;
        }

        Ok(serde_json::json!({"status": "ok", "skill_id": skill_id, "char_id": input.skill.char_id, "action_id": input.skill.action_id}).to_string())
    })();

    match &result {
        Ok(_) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| format!("Failed to commit transaction: {e}"))?;
        }
        Err(_) => {
            conn.execute_batch("ROLLBACK")
                .map_err(|e| format!("Failed to rollback transaction: {e}"))?;
        }
    }

    result
}

/// Delete a skill by its id. Multipliers are cascade-deleted.
pub fn delete_skill(conn: &Connection, skill_id: i64) -> Result<String, String> {
    let affected = conn
        .execute("DELETE FROM skills WHERE id = ?1", params![skill_id])
        .map_err(|e| format!("Failed to delete skill: {e}"))?;

    if affected == 0 {
        return Err(format!("Skill with id {skill_id} not found"));
    }

    Ok(serde_json::json!({"status": "ok", "skill_id": skill_id}).to_string())
}

// ── Tauri command wrappers ───────────────────────────────

pub fn cmd_get_skills(conn: &Connection, char_id: String) -> Result<String, String> {
    get_skills(conn, &char_id)
}

pub fn cmd_save_skill(conn: &Connection, data: String) -> Result<String, String> {
    save_skill(conn, &data)
}

pub fn cmd_delete_skill(conn: &Connection, skill_id: i64) -> Result<String, String> {
    delete_skill(conn, skill_id)
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

    fn insert_test_character(conn: &Connection, char_id: &str) {
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES (?1, 'Test', 'Gentle_House', 'Attack', 'Physical')",
            params![char_id],
        )
        .unwrap();
    }

    fn insert_test_skill(conn: &Connection, char_id: &str, action_id: &str) -> i64 {
        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type, energy_cost, cooldown_ticks)
             VALUES (?1, ?2, 'Normal', 20, 30)",
            params![char_id, action_id],
        )
        .unwrap();
        conn.query_row("SELECT last_insert_rowid()", [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn test_get_skills_empty() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1");
        let json = get_skills(&conn, "c1").unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_get_skills_single() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1");
        let skill_id = insert_test_skill(&conn, "c1", "Normal_1");

        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier)
             VALUES (?1, 1, 10, 0.5)",
            params![skill_id],
        )
        .unwrap();

        let json = get_skills(&conn, "c1").unwrap();
        let skills: Vec<SkillWithMultipliers> = serde_json::from_str(&json).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill.action_id, "Normal_1");
        assert_eq!(skills[0].multipliers.len(), 1);
        assert_eq!(skills[0].multipliers[0].multiplier, 0.5);
    }

    #[test]
    fn test_get_skills_multiple() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1");
        insert_test_skill(&conn, "c1", "Atk_1");
        insert_test_skill(&conn, "c1", "Atk_2");

        let json = get_skills(&conn, "c1").unwrap();
        let skills: Vec<SkillWithMultipliers> = serde_json::from_str(&json).unwrap();
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].skill.action_id, "Atk_1");
        assert_eq!(skills[1].skill.action_id, "Atk_2");
    }

    #[test]
    fn test_save_skill_new() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1");

        let data = r#"{
            "skill": {
                "id": null,
                "char_id": "c1",
                "action_id": "Special_1",
                "action_type": "Special",
                "daze_multiplier": 1.5,
                "energy_cost": 30,
                "decibel_cost": 0,
                "hp_cost": 0,
                "cooldown_ticks": 60,
                "animation_frames": 45,
                "interruptible_frame": 20,
                "is_snapshot": false,
                "prerequisite_action_id": null,
                "effect_id": null
            },
            "multipliers": [
                {"segment_index": 1, "frame": 10, "multiplier": 0.8, "decay_coeff": null},
                {"segment_index": 2, "frame": 30, "multiplier": 1.2, "decay_coeff": 0.9}
            ]
        }"#;

        let result = save_skill(&conn, data).unwrap();
        assert!(result.contains("Special_1"));

        // Verify skill was saved
        let json = get_skills(&conn, "c1").unwrap();
        let skills: Vec<SkillWithMultipliers> = serde_json::from_str(&json).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].multipliers.len(), 2);
    }

    #[test]
    fn test_save_skill_update() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1");
        let skill_id = insert_test_skill(&conn, "c1", "Normal_1");

        // Add a multiplier
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier)
             VALUES (?1, 1, 10, 0.5)",
            params![skill_id],
        )
        .unwrap();

        // Update skill with new multiplier set
        let data = r#"{
            "skill": {
                "id": null,
                "char_id": "c1",
                "action_id": "Normal_1",
                "action_type": "Normal",
                "daze_multiplier": 2.0,
                "energy_cost": 40,
                "decibel_cost": 0,
                "hp_cost": 0,
                "cooldown_ticks": 30,
                "animation_frames": 40,
                "interruptible_frame": null,
                "is_snapshot": true,
                "prerequisite_action_id": null,
                "effect_id": "buff_01"
            },
            "multipliers": [
                {"segment_index": 1, "frame": 5, "multiplier": 1.0, "decay_coeff": 1.0}
            ]
        }"#;

        let result = save_skill(&conn, data).unwrap();
        assert!(result.contains("Normal_1"));

        let json = get_skills(&conn, "c1").unwrap();
        let skills: Vec<SkillWithMultipliers> = serde_json::from_str(&json).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill.daze_multiplier, 2.0);
        assert_eq!(skills[0].skill.is_snapshot, true);
        assert_eq!(skills[0].multipliers.len(), 1);
        assert_eq!(skills[0].multipliers[0].multiplier, 1.0);
    }

    #[test]
    fn test_save_skill_empty_char_id() {
        let conn = setup_conn();
        let data = r#"{
            "skill": {
                "id": null,
                "char_id": "",
                "action_id": "Test",
                "action_type": "Normal",
                "daze_multiplier": 0,
                "energy_cost": 0,
                "decibel_cost": 0,
                "hp_cost": 0,
                "cooldown_ticks": 0,
                "animation_frames": 0,
                "interruptible_frame": null,
                "is_snapshot": false,
                "prerequisite_action_id": null,
                "effect_id": null
            },
            "multipliers": []
        }"#;
        let result = save_skill(&conn, data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("char_id is required"));
    }

    #[test]
    fn test_save_skill_missing_char_fk() {
        let conn = setup_conn();
        // No character with id 'nonexistent'
        let data = r#"{
            "skill": {
                "id": null,
                "char_id": "nonexistent",
                "action_id": "Test",
                "action_type": "Normal",
                "daze_multiplier": 0,
                "energy_cost": 0,
                "decibel_cost": 0,
                "hp_cost": 0,
                "cooldown_ticks": 0,
                "animation_frames": 0,
                "interruptible_frame": null,
                "is_snapshot": false,
                "prerequisite_action_id": null,
                "effect_id": null
            },
            "multipliers": []
        }"#;
        let result = save_skill(&conn, data);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_skill() {
        let conn = setup_conn();
        insert_test_character(&conn, "c1");
        let skill_id = insert_test_skill(&conn, "c1", "Del_1");

        // Add a multiplier
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier)
             VALUES (?1, 1, 10, 0.5)",
            params![skill_id],
        )
        .unwrap();

        // Delete
        delete_skill(&conn, skill_id).unwrap();

        // Verify cascaded
        let skill_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skills WHERE char_id = 'c1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(skill_count, 0);

        let mult_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skill_multipliers WHERE skill_id = ?1",
                params![skill_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mult_count, 0);
    }

    #[test]
    fn test_delete_skill_not_found() {
        let conn = setup_conn();
        let result = delete_skill(&conn, 99999);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
