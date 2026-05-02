use rusqlite::{params, Connection};

/// Record counts for each table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataSummary {
    pub characters: i64,
    pub skills: i64,
    pub w_engines: i64,
    pub drive_discs: i64,
    pub disc_sets: i64,
    pub enemies: i64,
    pub apl: i64,
}

/// A single search result row.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub data_type: String,
    pub id: String,
    pub name: String,
}

/// Get record counts for all tables as a JSON string.
pub fn get_data_summary(conn: &Connection) -> Result<String, String> {
    let count = |table: &str| -> Result<i64, String> {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {table}"),
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("Failed to count {table}: {e}"))
    };

    let summary = DataSummary {
        characters: count("characters")?,
        skills: count("skills")?,
        w_engines: count("w_engines")?,
        drive_discs: count("drive_discs")?,
        disc_sets: count("disc_sets")?,
        enemies: count("enemies")?,
        apl: count("apl")?,
    };

    serde_json::to_string(&summary).map_err(|e| format!("Failed to serialize summary: {e}"))
}

/// Search across data types using LIKE on name/id fields.
///
/// - `query`: the search term (applied as `%query%`)
/// - `data_type`: `"characters"`, `"skills"`, `"enemies"`, `"w_engines"`, `"disc_sets"`,
///   or `"all"` to search all types
///
/// Returns JSON array of `{ data_type, id, name }` results.
pub fn search_data(conn: &Connection, query: &str, data_type: &str) -> Result<String, String> {
    let pattern = format!("%{}%", query);
    let mut results: Vec<SearchResult> = Vec::new();

    let search_characters = |conn: &Connection, pattern: &str| -> Result<Vec<SearchResult>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT char_id, name FROM characters
                 WHERE char_id LIKE ?1 OR name LIKE ?1
                 ORDER BY char_id LIMIT 50",
            )
            .map_err(|e| format!("Search prepare error: {e}"))?;
        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok(SearchResult {
                    data_type: "character".to_string(),
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| format!("Search query error: {e}"))?;
        let mut r = Vec::new();
        for row in rows {
            r.push(row.map_err(|e| format!("Search read error: {e}"))?);
        }
        Ok(r)
    };

    let search_skills = |conn: &Connection, pattern: &str| -> Result<Vec<SearchResult>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT s.action_id, c.name || ' / ' || s.action_type
                 FROM skills s JOIN characters c ON s.char_id = c.char_id
                 WHERE s.action_id LIKE ?1 OR s.action_type LIKE ?1
                 ORDER BY s.action_id LIMIT 50",
            )
            .map_err(|e| format!("Search prepare error: {e}"))?;
        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok(SearchResult {
                    data_type: "skill".to_string(),
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| format!("Search query error: {e}"))?;
        let mut r = Vec::new();
        for row in rows {
            r.push(row.map_err(|e| format!("Search read error: {e}"))?);
        }
        Ok(r)
    };

    let search_enemies = |conn: &Connection, pattern: &str| -> Result<Vec<SearchResult>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT enemy_id, name FROM enemies
                 WHERE enemy_id LIKE ?1 OR name LIKE ?1
                 ORDER BY enemy_id LIMIT 50",
            )
            .map_err(|e| format!("Search prepare error: {e}"))?;
        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok(SearchResult {
                    data_type: "enemy".to_string(),
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| format!("Search query error: {e}"))?;
        let mut r = Vec::new();
        for row in rows {
            r.push(row.map_err(|e| format!("Search read error: {e}"))?);
        }
        Ok(r)
    };

    let search_w_engines = |conn: &Connection, pattern: &str| -> Result<Vec<SearchResult>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name FROM w_engines
                 WHERE id LIKE ?1 OR name LIKE ?1
                 ORDER BY id LIMIT 50",
            )
            .map_err(|e| format!("Search prepare error: {e}"))?;
        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok(SearchResult {
                    data_type: "w_engine".to_string(),
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| format!("Search query error: {e}"))?;
        let mut r = Vec::new();
        for row in rows {
            r.push(row.map_err(|e| format!("Search read error: {e}"))?);
        }
        Ok(r)
    };

    let search_disc_sets = |conn: &Connection, pattern: &str| -> Result<Vec<SearchResult>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT set_id, name FROM disc_sets
                 WHERE set_id LIKE ?1 OR name LIKE ?1
                 ORDER BY set_id LIMIT 50",
            )
            .map_err(|e| format!("Search prepare error: {e}"))?;
        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok(SearchResult {
                    data_type: "disc_set".to_string(),
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| format!("Search query error: {e}"))?;
        let mut r = Vec::new();
        for row in rows {
            r.push(row.map_err(|e| format!("Search read error: {e}"))?);
        }
        Ok(r)
    };

    match data_type {
        "characters" => results = search_characters(conn, &pattern)?,
        "skills" => results = search_skills(conn, &pattern)?,
        "enemies" => results = search_enemies(conn, &pattern)?,
        "w_engines" => results = search_w_engines(conn, &pattern)?,
        "disc_sets" => results = search_disc_sets(conn, &pattern)?,
        "all" => {
            results.extend(search_characters(conn, &pattern)?);
            results.extend(search_skills(conn, &pattern)?);
            results.extend(search_enemies(conn, &pattern)?);
            results.extend(search_w_engines(conn, &pattern)?);
            results.extend(search_disc_sets(conn, &pattern)?);
        }
        _ => {
            return Err(format!(
                "Unknown data_type '{data_type}'. Must be one of: characters, skills, enemies, w_engines, disc_sets, all"
            ));
        }
    }

    serde_json::to_string(&results).map_err(|e| format!("Failed to serialize search results: {e}"))
}

// ── Tauri command wrappers ──────────────────────────────

pub fn cmd_get_data_summary(conn: &Connection) -> Result<String, String> {
    get_data_summary(conn)
}

pub fn cmd_search_data(conn: &Connection, query: String, data_type: String) -> Result<String, String> {
    search_data(conn, &query, &data_type)
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
    fn test_get_data_summary_empty() {
        let conn = setup_conn();
        let json = get_data_summary(&conn).unwrap();
        let summary: DataSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary.characters, 0);
        assert_eq!(summary.skills, 0);
        assert_eq!(summary.enemies, 0);
    }

    #[test]
    fn test_get_data_summary_with_data() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('c1', 'Test', 'Gentle_House', 'Attack', 'Fire')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO enemies (enemy_id, name) VALUES ('e1', 'Enemy 1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO w_engines (id, name) VALUES ('w1', 'Engine 1')",
            [],
        )
        .unwrap();

        let json = get_data_summary(&conn).unwrap();
        let summary: DataSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary.characters, 1);
        assert_eq!(summary.enemies, 1);
        assert_eq!(summary.w_engines, 1);
    }

    #[test]
    fn test_search_data_character() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('anby', 'Anby', 'Gentle_House', 'Attack', 'Electric')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('billy', 'Billy', 'Cunning_Hares', 'Attack', 'Physical')",
            [],
        )
        .unwrap();

        let json = search_data(&conn, "anby", "characters").unwrap();
        let results: Vec<SearchResult> = serde_json::from_str(&json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "anby");
    }

    #[test]
    fn test_search_data_all_types() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('soldier_11', 'Soldier 11', 'Obol', 'Attack', 'Fire')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO w_engines (id, name) VALUES ('starlight', 'Starlight Engine')",
            [],
        )
        .unwrap();

        let json = search_data(&conn, "star", "all").unwrap();
        let results: Vec<SearchResult> = serde_json::from_str(&json).unwrap();
        assert!(results.iter().any(|r| r.id == "starlight"), "expected starlight in results, got: {results:?}");
    }

    #[test]
    fn test_search_data_no_results() {
        let conn = setup_conn();
        let json = search_data(&conn, "nonexistent", "all").unwrap();
        let results: Vec<SearchResult> = serde_json::from_str(&json).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_data_invalid_type() {
        let conn = setup_conn();
        let result = search_data(&conn, "query", "invalid_type");
        assert!(result.is_err());
    }
}
