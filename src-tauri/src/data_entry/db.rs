use rusqlite::{Connection, Result};

/// 初始化数据库：创建所有表（如果不存在）并记录 schema 版本。
pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // ---- schema_version 元数据表 ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now')),
            description TEXT NOT NULL DEFAULT ''
        );",
    )?;

    // 如果还没有记录，插入初始版本 v1
    let v1_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM schema_version WHERE version = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !v1_exists {
        conn.execute(
            "INSERT INTO schema_version (version, description) VALUES (1, 'Initial schema — data entry tables')",
            [],
        )?;
    }

    // ---- characters ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS characters (
            char_id         TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            faction         TEXT NOT NULL,
            specialty       TEXT NOT NULL,
            element         TEXT NOT NULL,
            level           INTEGER NOT NULL DEFAULT 60,
            ascension       INTEGER NOT NULL DEFAULT 6,
            hp              REAL NOT NULL DEFAULT 0,
            atk             REAL NOT NULL DEFAULT 0,
            def             REAL NOT NULL DEFAULT 0,
            impact          REAL NOT NULL DEFAULT 0,
            crit_rate       REAL NOT NULL DEFAULT 0,
            crit_dmg        REAL NOT NULL DEFAULT 0,
            pen_ratio       REAL NOT NULL DEFAULT 0,
            pen_fixed       REAL NOT NULL DEFAULT 0,
            anomaly_mastery       REAL NOT NULL DEFAULT 0,
            anomaly_proficiency    REAL NOT NULL DEFAULT 0,
            energy_regen    REAL NOT NULL DEFAULT 0,
            energy_gen_rate REAL NOT NULL DEFAULT 0,
            constellations  TEXT NOT NULL DEFAULT '[false,false,false,false,false,false]',
            potentials      TEXT NOT NULL DEFAULT '[false,false,false,false,false,false]',
            action_dict     TEXT NOT NULL DEFAULT '[]',
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    // ---- skills ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS skills (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            char_id     TEXT NOT NULL REFERENCES characters(char_id) ON DELETE CASCADE,
            action_id   TEXT NOT NULL,
            action_type TEXT NOT NULL,
            daze_multiplier     REAL NOT NULL DEFAULT 0,
            energy_cost         REAL NOT NULL DEFAULT 0,
            decibel_cost        REAL NOT NULL DEFAULT 0,
            hp_cost             REAL NOT NULL DEFAULT 0,
            cooldown_ticks      INTEGER NOT NULL DEFAULT 0,
            animation_frames    INTEGER NOT NULL DEFAULT 0,
            interruptible_frame INTEGER,
            is_snapshot         INTEGER NOT NULL DEFAULT 0,
            prerequisite_action_id TEXT,
            effect_id           TEXT,
            updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(char_id, action_id)
        );",
    )?;

    // ---- skill_multipliers ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS skill_multipliers (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_id    INTEGER NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
            segment_index INTEGER NOT NULL,
            frame       INTEGER NOT NULL,
            multiplier  REAL NOT NULL,
            decay_coeff REAL DEFAULT 1.0
        );",
    )?;

    // ---- w_engines ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS w_engines (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            level           INTEGER NOT NULL DEFAULT 60,
            ascension       INTEGER NOT NULL DEFAULT 6,
            atk             REAL NOT NULL DEFAULT 0,
            crit_rate       REAL DEFAULT 0,
            crit_dmg        REAL DEFAULT 0,
            pen_ratio       REAL DEFAULT 0,
            energy_regen    REAL DEFAULT 0,
            impact          REAL DEFAULT 0,
            anomaly_mastery REAL DEFAULT 0,
            passive_effects TEXT NOT NULL DEFAULT '[]',
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    // ---- drive_discs ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS drive_discs (
            id          TEXT PRIMARY KEY,
            slot        INTEGER NOT NULL CHECK(slot BETWEEN 1 AND 6),
            level       INTEGER NOT NULL DEFAULT 15,
            set_id      TEXT NOT NULL,
            main_stat_name  TEXT NOT NULL,
            main_stat_value REAL NOT NULL,
            sub_stat_1_name  TEXT,
            sub_stat_1_value REAL,
            sub_stat_2_name  TEXT,
            sub_stat_2_value REAL,
            sub_stat_3_name  TEXT,
            sub_stat_3_value REAL,
            sub_stat_4_name  TEXT,
            sub_stat_4_value REAL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    // ---- disc_sets ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS disc_sets (
            set_id      TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            two_piece_description TEXT,
            two_piece_buff_id     TEXT,
            four_piece_description TEXT,
            four_piece_buff_id     TEXT,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    // ---- enemies ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS enemies (
            enemy_id    TEXT PRIMARY KEY,
            enemy_type  TEXT NOT NULL DEFAULT 'Normal',
            name        TEXT NOT NULL DEFAULT '',
            level       INTEGER NOT NULL DEFAULT 60,
            hp          REAL NOT NULL DEFAULT 0,
            def         REAL NOT NULL DEFAULT 0,
            base_res    REAL NOT NULL DEFAULT 0,
            daze_max    REAL NOT NULL DEFAULT 0,
            resistances TEXT NOT NULL DEFAULT '{}',
            weaknesses  TEXT NOT NULL DEFAULT '[]',
            anomaly_buildup TEXT NOT NULL DEFAULT '{}',
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    // ---- apl ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS apl (
            apl_id      TEXT PRIMARY KEY,
            name        TEXT NOT NULL DEFAULT '',
            tracks      TEXT NOT NULL DEFAULT '[]',
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    // ---- deployed_configs ----
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS deployed_configs (
            config_id        TEXT PRIMARY KEY,
            name             TEXT NOT NULL,
            char_id          TEXT NOT NULL REFERENCES characters(char_id) ON DELETE CASCADE,
            char_level       INTEGER,
            char_ascension   INTEGER,
            cinemas          TEXT,
            potentials       TEXT,
            wengine_id       TEXT REFERENCES w_engines(id) ON DELETE SET NULL,
            wengine_level    INTEGER,
            wengine_ascension INTEGER,
            disc_configs     TEXT,
            created_at       TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_db_creates_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        // 验证所有表都存在
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"characters".to_string()), "characters table");
        assert!(tables.contains(&"skills".to_string()), "skills table");
        assert!(tables.contains(&"skill_multipliers".to_string()), "skill_multipliers table");
        assert!(tables.contains(&"w_engines".to_string()), "w_engines table");
        assert!(tables.contains(&"drive_discs".to_string()), "drive_discs table");
        assert!(tables.contains(&"disc_sets".to_string()), "disc_sets table");
        assert!(tables.contains(&"enemies".to_string()), "enemies table");
        assert!(tables.contains(&"apl".to_string()), "apl table");
        assert!(tables.contains(&"deployed_configs".to_string()), "deployed_configs table");
        assert!(tables.contains(&"schema_version".to_string()), "schema_version table");
    }

    #[test]
    fn test_init_db_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        // 调用两次应该是安全的
        init_db(&conn).unwrap();
        init_db(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "schema_version should only have one row");
    }

    #[test]
    fn test_schema_version_v1() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        let (version, description): (i64, String) = conn
            .query_row(
                "SELECT version, description FROM schema_version WHERE version = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(version, 1);
        assert!(description.contains("data entry"));
    }

    #[test]
    fn test_characters_schema() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        // Insert a character and verify all columns
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element, level, ascension,
                hp, atk, def, impact, crit_rate, crit_dmg, pen_ratio, pen_fixed,
                anomaly_mastery, anomaly_proficiency, energy_regen, energy_gen_rate,
                constellations, potentials, action_dict)
            VALUES ('test_char', 'Test', 'Gentle_House', 'Attack', 'Physical', 60, 6,
                10000.0, 1200.0, 600.0, 110.0, 0.15, 0.80, 0.10, 40.0,
                80.0, 90.0, 1.2, 0.3,
                '[true,false,false,false,false,false]',
                '[true,true,false,false,false,false]',
                '[\"action_1\"]')",
            [],
        ).unwrap();

        let row: (String, String, String, String, String, i64, i64, f64, f64, f64, String) = conn
            .query_row(
                "SELECT char_id, name, faction, specialty, element, level, ascension, hp, atk, def, potentials
                 FROM characters WHERE char_id = 'test_char'",
                [],
                |row| Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                    row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?,
                    row.get(10)?,
                )),
            )
            .unwrap();

        assert_eq!(row.0, "test_char");
        assert_eq!(row.1, "Test");
        assert_eq!(row.2, "Gentle_House");
        assert_eq!(row.3, "Attack");
        assert_eq!(row.4, "Physical");
        assert_eq!(row.5, 60);
        assert_eq!(row.6, 6);
        assert_eq!(row.7, 10000.0);
        assert_eq!(row.8, 1200.0);
        assert_eq!(row.9, 600.0);
        assert_eq!(row.10, "[true,true,false,false,false,false]");
    }

    #[test]
    fn test_cascade_delete_character() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        // Insert character
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('c1', 'Char1', 'FactionA', 'Attack', 'Fire')",
            [],
        ).unwrap();

        // Insert skill
        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type)
             VALUES ('c1', 'atk_1', 'Normal')",
            [],
        ).unwrap();

        // Get skill_id
        let skill_id: i64 = conn
            .query_row("SELECT id FROM skills WHERE char_id = 'c1'", [], |row| row.get(0))
            .unwrap();

        // Insert multiplier
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier)
             VALUES (?1, 1, 10, 0.5)",
            [skill_id],
        ).unwrap();

        // Delete character — should cascade
        conn.execute("DELETE FROM characters WHERE char_id = 'c1'", []).unwrap();

        // Verify skills and multipliers are also deleted
        let skill_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM skills WHERE char_id = 'c1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(skill_count, 0, "skills should be cascade-deleted");

        let mult_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skill_multipliers WHERE skill_id = ?1",
                [skill_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mult_count, 0, "skill_multipliers should be cascade-deleted");
    }

    #[test]
    fn test_cascade_delete_skill() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('c1', 'Char1', 'Gentle_House', 'Attack', 'Fire')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type)
             VALUES ('c1', 'atk_1', 'Normal')",
            [],
        ).unwrap();
        let skill_id: i64 = conn
            .query_row("SELECT id FROM skills WHERE char_id = 'c1'", [], |row| row.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier)
             VALUES (?1, 1, 10, 0.5)",
            [skill_id],
        ).unwrap();

        // Delete skill — should cascade to multipliers
        conn.execute("DELETE FROM skills WHERE id = ?1", [skill_id]).unwrap();

        let mult_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skill_multipliers WHERE skill_id = ?1",
                [skill_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mult_count, 0);
    }

    #[test]
    fn test_deployed_configs_cascade() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        // Insert character and wengine
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('c1', 'Char1', 'Gentle_House', 'Attack', 'Fire')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO w_engines (id, name) VALUES ('w1', 'Engine1')",
            [],
        ).unwrap();

        // Insert deployed config
        conn.execute(
            "INSERT INTO deployed_configs (config_id, name, char_id, wengine_id)
             VALUES ('cfg1', 'Test Config', 'c1', 'w1')",
            [],
        ).unwrap();

        // Delete character — config should cascade-delete
        conn.execute("DELETE FROM characters WHERE char_id = 'c1'", []).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM deployed_configs WHERE config_id = 'cfg1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "deployed_config should be cascade-deleted with character");
    }

    #[test]
    fn test_deployed_configs_wengine_set_null() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('c1', 'Char1', 'Gentle_House', 'Attack', 'Fire')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO w_engines (id, name) VALUES ('w1', 'Engine1')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO deployed_configs (config_id, name, char_id, wengine_id)
             VALUES ('cfg1', 'Test Config', 'c1', 'w1')",
            [],
        ).unwrap();

        // Delete wengine — wengine_id should be set to NULL
        conn.execute("DELETE FROM w_engines WHERE id = 'w1'", []).unwrap();
        let wengine_id: Option<String> = conn
            .query_row(
                "SELECT wengine_id FROM deployed_configs WHERE config_id = 'cfg1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(wengine_id, None, "wengine_id should be SET NULL on wengine delete");
    }
}
