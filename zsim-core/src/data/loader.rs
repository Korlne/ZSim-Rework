use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::combat::skill::{HitFrame, SkillData};
use crate::data::apl::{APLData, Track};
use crate::data::equipment::{DiscBonus, DiscSet, DriveDisc, EquipmentData, StatEntry, WEngine};
use crate::entities::character::{Character, ResourceSet};
use crate::entities::enemy::EnemyState;
use crate::entities::enums::{
    CharacterState, ElementTag, EnemyType, FactionTag, SkillType, SpecialtyTag,
};
use crate::entities::models::BaseStats;

/// Central data loader that reads game data from a SQLite database.
///
/// Wraps a `rusqlite::Connection` and provides typed load methods for each
/// game data entity. Created via [`DataLoader::new`] (existing DB) or
/// [`DataLoader::from_json_dir`] (bootstrap from JSON files).
pub struct DataLoader {
    conn: Connection,
}

impl DataLoader {
    /// Open an existing SQLite database at `db_path`.
    ///
    /// The database must have been initialized with the schema defined in
    /// `data_entry::db::init_db()` (or the equivalent CREATE TABLE statements).
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("failed to open SQLite database: {}", db_path.display()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode=WAL;")?;
        Ok(Self { conn })
    }

    // -----------------------------------------------------------------------
    // Characters
    // -----------------------------------------------------------------------

    /// Load all characters from the `characters` table.
    pub fn load_characters(&self) -> Result<Vec<Character>> {
        let mut stmt = self.conn.prepare(
            "SELECT char_id, name, faction, specialty, element, level, ascension,
                    hp, atk, def, impact, crit_rate, crit_dmg, pen_ratio, pen_fixed,
                    anomaly_mastery, anomaly_proficiency, energy_regen, energy_gen_rate,
                    constellations, action_dict
             FROM characters
             ORDER BY char_id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,  // char_id
                row.get::<_, String>(1)?,  // name
                row.get::<_, String>(2)?,  // faction
                row.get::<_, String>(3)?,  // specialty
                row.get::<_, String>(4)?,  // element
                row.get::<_, i64>(5)?,     // level
                row.get::<_, i64>(6)?,     // ascension
                row.get::<_, f64>(7)?,     // hp
                row.get::<_, f64>(8)?,     // atk
                row.get::<_, f64>(9)?,     // def
                row.get::<_, f64>(10)?,    // impact
                row.get::<_, f64>(11)?,    // crit_rate
                row.get::<_, f64>(12)?,    // crit_dmg
                row.get::<_, f64>(13)?,    // pen_ratio
                row.get::<_, f64>(14)?,    // pen_fixed
                row.get::<_, f64>(15)?,    // anomaly_mastery
                row.get::<_, f64>(16)?,    // anomaly_proficiency
                row.get::<_, f64>(17)?,    // energy_regen
                row.get::<_, f64>(18)?,    // energy_gen_rate
                row.get::<_, String>(19)?, // constellations (JSON)
                row.get::<_, String>(20)?, // action_dict (JSON)
            ))
        })?;

        let mut characters = Vec::new();
        for row in rows {
            let (
                char_id,
                name,
                faction_str,
                specialty_str,
                element_str,
                level,
                ascension,
                hp,
                atk,
                def,
                impact,
                crit_rate,
                crit_dmg,
                pen_ratio,
                pen_fixed,
                anomaly_mastery,
                anomaly_proficiency,
                energy_regen,
                energy_gen_rate,
                constellations_json,
                action_dict_json,
            ) = row?;

            let faction: FactionTag = serde_json::from_str(&format!("\"{}\"", faction_str))
                .with_context(|| format!("invalid faction string: {faction_str}"))?;
            let specialty: SpecialtyTag =
                serde_json::from_str(&format!("\"{}\"", specialty_str))
                    .with_context(|| format!("invalid specialty string: {specialty_str}"))?;
            let element: ElementTag = serde_json::from_str(&format!("\"{}\"", element_str))
                .with_context(|| format!("invalid element string: {element_str}"))?;

            let constellations: [bool; 6] =
                serde_json::from_str(&constellations_json).unwrap_or([false; 6]);
            let action_dict: HashSet<String> =
                serde_json::from_str(&action_dict_json).unwrap_or_default();

            let base_stats = BaseStats {
                hp,
                atk,
                def,
                impact,
                crit_rate,
                crit_dmg,
                pen: pen_fixed,
                pen_ratio,
                anomaly_mastery,
                anomaly_proficiency,
                energy_regen,
                dmg_bonus: energy_gen_rate,
            };

            characters.push(Character {
                char_id,
                name,
                faction,
                specialty,
                element,
                level: level as u32,
                ascension: ascension as u32,
                base_stats: base_stats.clone(),
                current_stats: base_stats,
                resources: ResourceSet::default(),
                state: CharacterState::Standby,
                action_dict,
                constellations,
                realtime_modifiers: HashMap::new(),
            });
        }

        Ok(characters)
    }

    // -----------------------------------------------------------------------
    // Enemies
    // -----------------------------------------------------------------------

    /// Load all enemies from the `enemies` table.
    pub fn load_enemies(&self) -> Result<Vec<EnemyState>> {
        let mut stmt = self.conn.prepare(
            "SELECT enemy_id, enemy_type, name, level, hp, def, base_res, daze_max,
                    resistances, weaknesses, anomaly_buildup
             FROM enemies
             ORDER BY enemy_id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, f64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        })?;

        let mut enemies = Vec::new();
        for row in rows {
            let (
                enemy_id,
                enemy_type_str,
                _name,
                _level,
                hp,
                def_val,
                base_res,
                daze_max,
                resistances_json,
                weaknesses_json,
                anomaly_buildup_json,
            ) = row?;

            let enemy_type: EnemyType = serde_json::from_str(&format!("\"{}\"", enemy_type_str))
                .with_context(|| format!("invalid enemy_type string: {enemy_type_str}"))?;
            let resistances: HashMap<ElementTag, f64> =
                serde_json::from_str(&resistances_json).unwrap_or_default();
            let weaknesses: HashSet<ElementTag> =
                serde_json::from_str(&weaknesses_json).unwrap_or_default();
            let anomaly_buildup: HashMap<ElementTag, f64> =
                serde_json::from_str(&anomaly_buildup_json).unwrap_or_default();

            enemies.push(EnemyState {
                enemy_id,
                enemy_type,
                hp,
                def_val,
                base_res,
                stun_gauge: 0.0,
                stun_max: daze_max,
                is_stunned: false,
                anomaly_buildup,
                resistances,
                weaknesses,
            });
        }

        Ok(enemies)
    }

    // -----------------------------------------------------------------------
    // Skills
    // -----------------------------------------------------------------------

    /// Load all skills for a given character from `skills` JOIN `skill_multipliers`.
    pub fn load_skills(&self, char_id: &str) -> Result<Vec<SkillData>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.action_id, s.action_type, s.daze_multiplier,
                    s.energy_cost, s.decibel_cost, s.hp_cost, s.cooldown_ticks,
                    s.animation_frames, s.interruptible_frame, s.is_snapshot,
                    s.prerequisite_action_id
             FROM skills s
             WHERE s.char_id = ?1
             ORDER BY s.action_id",
        )?;

        let skill_rows = stmt.query_map(params![char_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        })?;

        // Pre-load all multipliers keyed by (char_id, action_id)
        let mut mult_stmt = self.conn.prepare(
            "SELECT sm.skill_id, sm.segment_index, sm.frame, sm.multiplier
             FROM skill_multipliers sm
             JOIN skills s ON sm.skill_id = s.id
             WHERE s.char_id = ?1
             ORDER BY s.action_id, sm.segment_index",
        )?;

        #[allow(clippy::type_complexity)]
        let mult_rows: Vec<(i64, i64, u64, f64)> = mult_stmt
            .query_map(params![char_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, f64>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // We need to map skill_id → char_id → action_id. Let's get the skill_id→action_id map first.
        let mut id_map_stmt = self
            .conn
            .prepare("SELECT id, action_id FROM skills WHERE char_id = ?1")?;
        let id_map: HashMap<i64, String> = id_map_stmt
            .query_map(params![char_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        // Group multipliers by action_id
        let mut mults_by_action: HashMap<String, Vec<HitFrame>> = HashMap::new();
        for (skill_id, _segment_idx, frame, multiplier) in mult_rows {
            if let Some(action_id) = id_map.get(&skill_id) {
                mults_by_action
                    .entry(action_id.clone())
                    .or_default()
                    .push(HitFrame { frame, multiplier });
            }
        }

        let mut skills = Vec::new();
        for row in skill_rows {
            let (
                action_id,
                action_type_str,
                daze_multiplier,
                energy_cost,
                decibel_cost,
                hp_cost,
                cooldown_ticks,
                animation_frames,
                interruptible_frame,
                is_snapshot,
                prerequisite_action_id,
            ) = row?;

            let action_type: SkillType = serde_json::from_str(&format!("\"{}\"", action_type_str))
                .with_context(|| format!("invalid action_type string: {action_type_str}"))?;

            let damage_multipliers = mults_by_action.remove(&action_id).unwrap_or_default();
            let hit_frames: Vec<u64> = damage_multipliers.iter().map(|hf| hf.frame).collect();

            skills.push(SkillData {
                action_id,
                action_type,
                damage_multipliers,
                daze_multiplier,
                hit_frames,
                invincible_frames: Vec::new(),
                interruptible_frame: interruptible_frame.unwrap_or(0) as u64,
                is_snapshot: is_snapshot != 0,
                charge_branches: Vec::new(),
                prerequisite_action_id,
                hp_cost,
                energy_cost,
                decibel_cost,
                cooldown_ticks: cooldown_ticks as u64,
                animation_frames: animation_frames as u64,
            });
        }

        Ok(skills)
    }

    /// Load skills across all characters.
    pub fn load_all_skills(&self) -> Result<Vec<SkillData>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT char_id FROM skills ORDER BY char_id")?;
        let char_ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut all_skills = Vec::new();
        for char_id in &char_ids {
            let skills = self.load_skills(char_id)?;
            all_skills.extend(skills);
        }
        Ok(all_skills)
    }

    // -----------------------------------------------------------------------
    // Equipment
    // -----------------------------------------------------------------------

    /// Load all equipment data (w_engines + drive_discs + disc_sets).
    pub fn load_equipment(&self) -> Result<EquipmentData> {
        Ok(EquipmentData {
            w_engines: self.load_w_engines()?,
            drive_discs: self.load_drive_discs()?,
            disc_sets: self.load_disc_sets()?,
        })
    }

    fn load_w_engines(&self) -> Result<Vec<WEngine>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, level, ascension,
                    atk, crit_rate, crit_dmg, pen_ratio, energy_regen, impact, anomaly_mastery,
                    passive_effects
             FROM w_engines
             ORDER BY id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, Option<f64>>(8)?,
                row.get::<_, Option<f64>>(9)?,
                row.get::<_, Option<f64>>(10)?,
                row.get::<_, String>(11)?,
            ))
        })?;

        let mut engines = Vec::new();
        for row in rows {
            let (
                id,
                name,
                level,
                ascension,
                atk,
                crit_rate,
                crit_dmg,
                pen_ratio,
                energy_regen,
                impact,
                anomaly_mastery,
                passive_json,
            ) = row?;

            let passive_effects: Vec<String> =
                serde_json::from_str(&passive_json).unwrap_or_default();

            engines.push(WEngine {
                id,
                name,
                level: level as u32,
                ascension: ascension as u32,
                base_stats: BaseStats {
                    atk,
                    crit_rate,
                    crit_dmg,
                    pen_ratio: pen_ratio.unwrap_or(0.0),
                    energy_regen: energy_regen.unwrap_or(0.0),
                    impact: impact.unwrap_or(0.0),
                    anomaly_mastery: anomaly_mastery.unwrap_or(0.0),
                    ..BaseStats::default()
                },
                passive_effects,
            });
        }

        Ok(engines)
    }

    fn load_drive_discs(&self) -> Result<Vec<DriveDisc>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, slot, level, set_id,
                    main_stat_name, main_stat_value,
                    sub_stat_1_name, sub_stat_1_value,
                    sub_stat_2_name, sub_stat_2_value,
                    sub_stat_3_name, sub_stat_3_value,
                    sub_stat_4_name, sub_stat_4_value
             FROM drive_discs
             ORDER BY id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<f64>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<f64>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<f64>>(13)?,
            ))
        })?;

        let mut discs = Vec::new();
        for row in rows {
            let (
                id,
                slot,
                level,
                set_id,
                main_stat_name,
                main_stat_value,
                sn1,
                sv1,
                sn2,
                sv2,
                sn3,
                sv3,
                sn4,
                sv4,
            ) = row?;

            let mut sub_stats = Vec::new();
            if let Some(n) = sn1 {
                if !n.is_empty() {
                    sub_stats.push(StatEntry {
                        stat_name: n,
                        value: sv1.unwrap_or(0.0),
                    });
                }
            }
            if let Some(n) = sn2 {
                if !n.is_empty() {
                    sub_stats.push(StatEntry {
                        stat_name: n,
                        value: sv2.unwrap_or(0.0),
                    });
                }
            }
            if let Some(n) = sn3 {
                if !n.is_empty() {
                    sub_stats.push(StatEntry {
                        stat_name: n,
                        value: sv3.unwrap_or(0.0),
                    });
                }
            }
            if let Some(n) = sn4 {
                if !n.is_empty() {
                    sub_stats.push(StatEntry {
                        stat_name: n,
                        value: sv4.unwrap_or(0.0),
                    });
                }
            }

            discs.push(DriveDisc {
                id,
                slot: slot as u8,
                level: level as u32,
                main_stat: StatEntry {
                    stat_name: main_stat_name,
                    value: main_stat_value,
                },
                sub_stats,
                set_id,
            });
        }

        Ok(discs)
    }

    fn load_disc_sets(&self) -> Result<Vec<DiscSet>> {
        let mut stmt = self.conn.prepare(
            "SELECT set_id, name,
                    two_piece_description, two_piece_buff_id,
                    four_piece_description, four_piece_buff_id
             FROM disc_sets
             ORDER BY set_id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;

        let mut sets = Vec::new();
        for row in rows {
            let (set_id, name, tp_desc, tp_buff, fp_desc, fp_buff) = row?;

            let two_piece_bonus = match (tp_desc, tp_buff) {
                (Some(desc), Some(buff_id)) => Some(DiscBonus {
                    description: desc,
                    buff_id,
                }),
                _ => None,
            };

            let four_piece_bonus = match (fp_desc, fp_buff) {
                (Some(desc), Some(buff_id)) => Some(DiscBonus {
                    description: desc,
                    buff_id,
                }),
                _ => None,
            };

            sets.push(DiscSet {
                set_id,
                name,
                two_piece_bonus,
                four_piece_bonus,
            });
        }

        Ok(sets)
    }

    // -----------------------------------------------------------------------
    // APL
    // -----------------------------------------------------------------------

    /// Load APL data by `apl_id` from the `apl` table.
    pub fn load_apl(&self, apl_id: &str) -> Result<APLData> {
        let tracks_json: String = self
            .conn
            .query_row(
                "SELECT tracks FROM apl WHERE apl_id = ?1",
                params![apl_id],
                |row| row.get(0),
            )
            .with_context(|| format!("APL record not found: {apl_id}"))?;

        let tracks: Vec<Track> = serde_json::from_str(&tracks_json)
            .with_context(|| format!("failed to parse APL tracks JSON for {apl_id}"))?;

        Ok(APLData { tracks })
    }

    // -----------------------------------------------------------------------
    // Convenience: bootstrap from JSON files
    // -----------------------------------------------------------------------

    /// Convenience method that imports all JSON data from `data_dir` into a
    /// SQLite database at `db_path`, then returns a `DataLoader` reading from it.
    ///
    /// This is a one-stop bootstrap for migrating from flat-file JSON to SQLite.
    /// The database is created (or overwritten) with the full schema, populated
    /// from the JSON files, and returned ready for querying.
    pub fn from_json_dir(db_path: &Path, data_dir: &Path) -> Result<Self> {
        // Remove existing database so we start fresh
        let _ = fs::remove_file(db_path);

        let conn = Connection::open(db_path)
            .with_context(|| format!("failed to create database: {}", db_path.display()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode=WAL;")?;

        // Create all tables (same schema as data_entry::db::init_db)
        Self::init_schema(&conn)?;

        // Import JSON data
        Self::import_characters_from_json(&conn, data_dir)?;
        Self::import_skills_from_json(&conn, data_dir)?;
        Self::import_equipment_from_json(&conn, data_dir)?;
        Self::import_enemies_from_json(&conn, data_dir)?;
        Self::import_apl_from_json(&conn, data_dir)?;

        Ok(Self { conn })
    }

    // -----------------------------------------------------------------------
    // Schema initialization (mirrors data_entry::db::init_db)
    // -----------------------------------------------------------------------

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now')),
                description TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS characters (
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
                action_dict     TEXT NOT NULL DEFAULT '[]',
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS skills (
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
            );

            CREATE TABLE IF NOT EXISTS skill_multipliers (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_id    INTEGER NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
                segment_index INTEGER NOT NULL,
                frame       INTEGER NOT NULL,
                multiplier  REAL NOT NULL,
                decay_coeff REAL DEFAULT 1.0
            );

            CREATE TABLE IF NOT EXISTS w_engines (
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
            );

            CREATE TABLE IF NOT EXISTS drive_discs (
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
            );

            CREATE TABLE IF NOT EXISTS disc_sets (
                set_id      TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                two_piece_description TEXT,
                two_piece_buff_id     TEXT,
                four_piece_description TEXT,
                four_piece_buff_id     TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS enemies (
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
            );

            CREATE TABLE IF NOT EXISTS apl (
                apl_id      TEXT PRIMARY KEY,
                name        TEXT NOT NULL DEFAULT '',
                tracks      TEXT NOT NULL DEFAULT '[]',
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;

        // Record schema version
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

        Ok(())
    }

    // -----------------------------------------------------------------------
    // JSON import helpers (used by from_json_dir)
    // -----------------------------------------------------------------------

    fn import_characters_from_json(conn: &Connection, data_dir: &Path) -> Result<()> {
        let dir = data_dir.join("characters");
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Ok(()), // directory optional
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let character: Character = match serde_json::from_str(&content) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let faction_str = Self::enum_to_string(&character.faction);
            let specialty_str = Self::enum_to_string(&character.specialty);
            let element_str = Self::enum_to_string(&character.element);
            let constellations_json =
                serde_json::to_string(&character.constellations).unwrap_or_default();
            let action_set: Vec<&String> = character.action_dict.iter().collect();
            let action_dict_json = serde_json::to_string(&action_set).unwrap_or_default();

            let _ = conn.execute(
                "INSERT OR REPLACE INTO characters
                 (char_id, name, faction, specialty, element, level, ascension,
                  hp, atk, def, impact, crit_rate, crit_dmg, pen_ratio, pen_fixed,
                  anomaly_mastery, anomaly_proficiency, energy_regen, energy_gen_rate,
                  constellations, action_dict, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                         ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                         ?16, ?17, ?18, ?19,
                         ?20, ?21, datetime('now'))",
                params![
                    character.char_id,
                    character.name,
                    faction_str,
                    specialty_str,
                    element_str,
                    character.level,
                    character.ascension,
                    character.base_stats.hp,
                    character.base_stats.atk,
                    character.base_stats.def,
                    character.base_stats.impact,
                    character.base_stats.crit_rate,
                    character.base_stats.crit_dmg,
                    character.base_stats.pen_ratio,
                    character.base_stats.pen,
                    character.base_stats.anomaly_mastery,
                    character.base_stats.anomaly_proficiency,
                    character.base_stats.energy_regen,
                    character.base_stats.dmg_bonus,
                    constellations_json,
                    action_dict_json,
                ],
            );
        }
        Ok(())
    }

    fn import_skills_from_json(conn: &Connection, data_dir: &Path) -> Result<()> {
        let dir = data_dir.join("skills");
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let skills: Vec<SkillData> = match serde_json::from_str(&content) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let char_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            for skill in &skills {
                let action_type_str = Self::enum_to_string(&skill.action_type);
                let is_snapshot = if skill.is_snapshot { 1 } else { 0 };

                if let Err(e) = conn.execute(
                    "INSERT OR REPLACE INTO skills
                     (char_id, action_id, action_type, daze_multiplier,
                      energy_cost, decibel_cost, hp_cost, cooldown_ticks,
                      animation_frames, interruptible_frame, is_snapshot,
                      prerequisite_action_id, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))",
                    params![
                        char_id,
                        skill.action_id,
                        action_type_str,
                        skill.daze_multiplier,
                        skill.energy_cost,
                        skill.decibel_cost,
                        skill.hp_cost,
                        skill.cooldown_ticks,
                        skill.animation_frames,
                        skill.interruptible_frame as i64,
                        is_snapshot,
                        skill.prerequisite_action_id,
                    ],
                ) {
                    eprintln!(
                        "Warning: failed to insert skill {}/{}: {e}",
                        char_id, skill.action_id
                    );
                    continue;
                }

                // Get the skill_id
                let skill_id: i64 = match conn.query_row(
                    "SELECT id FROM skills WHERE char_id = ?1 AND action_id = ?2",
                    params![char_id, skill.action_id],
                    |row| row.get(0),
                ) {
                    Ok(id) => id,
                    Err(_) => continue,
                };

                // Remove old multipliers and insert new ones
                let _ = conn.execute(
                    "DELETE FROM skill_multipliers WHERE skill_id = ?1",
                    params![skill_id],
                );

                for (idx, hf) in skill.damage_multipliers.iter().enumerate() {
                    let _ = conn.execute(
                        "INSERT INTO skill_multipliers
                         (skill_id, segment_index, frame, multiplier, decay_coeff)
                         VALUES (?1, ?2, ?3, ?4, 1.0)",
                        params![skill_id, idx as i64 + 1, hf.frame, hf.multiplier],
                    );
                }
            }
        }
        Ok(())
    }

    fn import_equipment_from_json(conn: &Connection, data_dir: &Path) -> Result<()> {
        let path = data_dir.join("equipment").join("equipment.json");
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let equipment: EquipmentData = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        // W-Engines
        for we in &equipment.w_engines {
            let passive_json =
                serde_json::to_string(&we.passive_effects).unwrap_or_else(|_| "[]".to_string());
            let _ = conn.execute(
                "INSERT OR REPLACE INTO w_engines
                 (id, name, level, ascension,
                  atk, crit_rate, crit_dmg, pen_ratio, energy_regen, impact, anomaly_mastery,
                  passive_effects, updated_at)
                 VALUES (?1, ?2, ?3, ?4,
                         ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                         ?12, datetime('now'))",
                params![
                    we.id,
                    we.name,
                    we.level,
                    we.ascension,
                    we.base_stats.atk,
                    we.base_stats.crit_rate,
                    we.base_stats.crit_dmg,
                    we.base_stats.pen_ratio,
                    we.base_stats.energy_regen,
                    we.base_stats.impact,
                    we.base_stats.anomaly_mastery,
                    passive_json,
                ],
            );
        }

        // Drive discs
        for dd in &equipment.drive_discs {
            let sub = &dd.sub_stats;
            let (sn1, sv1) = sub
                .get(0)
                .map(|s| (s.stat_name.as_str(), s.value))
                .unwrap_or(("", 0.0));
            let (sn2, sv2) = sub
                .get(1)
                .map(|s| (s.stat_name.as_str(), s.value))
                .unwrap_or(("", 0.0));
            let (sn3, sv3) = sub
                .get(2)
                .map(|s| (s.stat_name.as_str(), s.value))
                .unwrap_or(("", 0.0));
            let (sn4, sv4) = sub
                .get(3)
                .map(|s| (s.stat_name.as_str(), s.value))
                .unwrap_or(("", 0.0));

            let _ = conn.execute(
                "INSERT OR REPLACE INTO drive_discs
                 (id, slot, level, set_id,
                  main_stat_name, main_stat_value,
                  sub_stat_1_name, sub_stat_1_value,
                  sub_stat_2_name, sub_stat_2_value,
                  sub_stat_3_name, sub_stat_3_value,
                  sub_stat_4_name, sub_stat_4_value,
                  updated_at)
                 VALUES (?1, ?2, ?3, ?4,
                         ?5, ?6,
                         ?7, ?8, ?9, ?10,
                         ?11, ?12, ?13, ?14,
                         datetime('now'))",
                params![
                    dd.id,
                    dd.slot,
                    dd.level,
                    dd.set_id,
                    dd.main_stat.stat_name,
                    dd.main_stat.value,
                    sn1,
                    sv1,
                    sn2,
                    sv2,
                    sn3,
                    sv3,
                    sn4,
                    sv4,
                ],
            );
        }

        // Disc sets
        for ds in &equipment.disc_sets {
            let (tp_desc, tp_buff) = ds
                .two_piece_bonus
                .as_ref()
                .map(|b| (Some(b.description.as_str()), Some(b.buff_id.as_str())))
                .unwrap_or((None, None));

            let (fp_desc, fp_buff) = ds
                .four_piece_bonus
                .as_ref()
                .map(|b| (Some(b.description.as_str()), Some(b.buff_id.as_str())))
                .unwrap_or((None, None));

            let _ = conn.execute(
                "INSERT OR REPLACE INTO disc_sets
                 (set_id, name,
                  two_piece_description, two_piece_buff_id,
                  four_piece_description, four_piece_buff_id,
                  updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
                params![ds.set_id, ds.name, tp_desc, tp_buff, fp_desc, fp_buff],
            );
        }

        Ok(())
    }

    fn import_enemies_from_json(conn: &Connection, data_dir: &Path) -> Result<()> {
        let dir = data_dir.join("enemies");
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let enemy: EnemyState = match serde_json::from_str(&content) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let enemy_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&enemy.enemy_id)
                .to_string();

            let enemy_type_str = Self::enum_to_string(&enemy.enemy_type);
            let resistances_json =
                serde_json::to_string(&enemy.resistances).unwrap_or_else(|_| "{}".to_string());
            let weaknesses_json =
                serde_json::to_string(&enemy.weaknesses).unwrap_or_else(|_| "[]".to_string());
            let anomaly_buildup_json =
                serde_json::to_string(&enemy.anomaly_buildup).unwrap_or_else(|_| "{}".to_string());

            let _ = conn.execute(
                "INSERT OR REPLACE INTO enemies
                 (enemy_id, enemy_type, name, level, hp, def, base_res, daze_max,
                  resistances, weaknesses, anomaly_buildup, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
                params![
                    enemy_id,
                    enemy_type_str,
                    enemy_id,
                    60,
                    enemy.hp,
                    enemy.def_val,
                    enemy.base_res,
                    enemy.stun_max,
                    resistances_json,
                    weaknesses_json,
                    anomaly_buildup_json,
                ],
            );
        }
        Ok(())
    }

    fn import_apl_from_json(conn: &Connection, data_dir: &Path) -> Result<()> {
        let dir = data_dir.join("apl");
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let apl: APLData = match serde_json::from_str(&content) {
                Ok(a) => a,
                Err(_) => continue,
            };

            let apl_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let tracks_json =
                serde_json::to_string(&apl.tracks).unwrap_or_else(|_| "[]".to_string());

            let _ = conn.execute(
                "INSERT OR REPLACE INTO apl (apl_id, name, tracks, updated_at)
                 VALUES (?1, ?2, ?3, datetime('now'))",
                params![apl_id, apl_id, tracks_json],
            );
        }
        Ok(())
    }

    // Helper: serialize an enum to its serde-rename string
    fn enum_to_string<T: serde::Serialize>(value: &T) -> String {
        serde_json::to_value(value)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Create an in-memory DataLoader seeded with test data for characters.
    fn seed_character_db() -> DataLoader {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        DataLoader::init_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element, level, ascension,
             hp, atk, def, impact, crit_rate, crit_dmg, pen_ratio, pen_fixed,
             anomaly_mastery, anomaly_proficiency, energy_regen, energy_gen_rate,
             constellations, action_dict)
             VALUES ('anby_demara', 'Anby Demara', 'Gentle_House', 'Stun', 'Electric', 60, 6,
             9500.0, 1100.0, 550.0, 120.0, 0.15, 0.80, 0.10, 40.0,
             80.0, 95.0, 1.2, 0.3,
             '[true,false,false,false,false,false]', '[\"Attack_Normal_1\",\"Attack_Normal_2\",\"Skill_Ex_1\",\"Ultimate_1\",\"Dodge_1\",\"Assist_1\"]')",
            [],
        ).unwrap();

        DataLoader { conn }
    }

    /// Create an in-memory DataLoader seeded with skill + multiplier test data.
    fn seed_skills_db() -> DataLoader {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        DataLoader::init_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('anby_demara', 'Anby', 'Gentle_House', 'Stun', 'Electric')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type, daze_multiplier,
             energy_cost, cooldown_ticks, animation_frames, interruptible_frame, is_snapshot)
             VALUES ('anby_demara', 'Attack_Normal_1', 'Normal', 0.4,
             0.0, 0, 30, 25, 0)",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type, daze_multiplier,
             energy_cost, cooldown_ticks, animation_frames, interruptible_frame)
             VALUES ('anby_demara', 'Skill_Ex_1', 'Special', 2.5,
             40.0, 8, 50, 45)",
            [],
        )
        .unwrap();

        // Get skill IDs and insert multipliers
        let skill1_id: i64 = conn
            .query_row(
                "SELECT id FROM skills WHERE action_id = 'Attack_Normal_1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let skill2_id: i64 = conn
            .query_row(
                "SELECT id FROM skills WHERE action_id = 'Skill_Ex_1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier) VALUES (?1, 1, 8, 0.5)",
            params![skill1_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier) VALUES (?1, 2, 16, 0.7)",
            params![skill1_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier) VALUES (?1, 1, 15, 3.0)",
            params![skill2_id],
        ).unwrap();

        DataLoader { conn }
    }

    fn write_json(dir: &Path, subdir: &str, filename: &str, content: &str) {
        let d = dir.join(subdir);
        fs::create_dir_all(&d).expect("create subdir");
        let mut f = fs::File::create(d.join(filename)).expect("create file");
        f.write_all(content.as_bytes()).expect("write file");
    }

    // -----------------------------------------------------------------------
    // Character tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_characters() {
        let loader = seed_character_db();
        let characters = loader.load_characters().expect("load characters");
        assert_eq!(characters.len(), 1);

        let anby = &characters[0];
        assert_eq!(anby.char_id, "anby_demara");
        assert_eq!(anby.name, "Anby Demara");
        assert_eq!(anby.level, 60);
        assert_eq!(anby.ascension, 6);
        assert_eq!(anby.base_stats.hp, 9500.0);
        assert_eq!(anby.base_stats.atk, 1100.0);
        assert_eq!(anby.base_stats.def, 550.0);
        assert_eq!(anby.base_stats.impact, 120.0);
        assert_eq!(anby.base_stats.crit_rate, 0.15);
        assert_eq!(anby.base_stats.crit_dmg, 0.80);
        assert_eq!(anby.base_stats.pen_ratio, 0.10);
        assert_eq!(anby.base_stats.pen, 40.0);
        assert_eq!(anby.base_stats.anomaly_mastery, 80.0);
        assert_eq!(anby.base_stats.anomaly_proficiency, 95.0);
        assert_eq!(anby.base_stats.energy_regen, 1.2);
        assert_eq!(anby.base_stats.dmg_bonus, 0.3);
        assert!(anby.constellations[0]);
        assert_eq!(anby.action_dict.len(), 6);
        assert!(anby.validate_action("Attack_Normal_1"));
        assert!(anby.validate_action("Ultimate_1"));
    }

    #[test]
    fn test_load_characters_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();
        let loader = DataLoader { conn };
        let characters = loader.load_characters().expect("load characters");
        assert!(characters.is_empty());
    }

    // -----------------------------------------------------------------------
    // Enemy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_enemies() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO enemies (enemy_id, enemy_type, hp, def, base_res, daze_max,
             resistances, weaknesses)
             VALUES ('boss_dullahan', 'Boss', 150000.0, 600.0, 0.15, 200.0,
             '{\"Ice\":0.4,\"Ether\":0.6}', '[\"Fire\",\"Physical\"]')",
            [],
        )
        .unwrap();

        let loader = DataLoader { conn };
        let enemies = loader.load_enemies().expect("load enemies");
        assert_eq!(enemies.len(), 1);

        let boss = &enemies[0];
        assert_eq!(boss.enemy_id, "boss_dullahan");
        assert_eq!(boss.hp, 150000.0);
        assert_eq!(boss.def_val, 600.0);
        assert_eq!(boss.base_res, 0.15);
        assert_eq!(boss.stun_max, 200.0);
        assert_eq!(boss.resistance_for(&ElementTag::Ice), 0.4);
        assert_eq!(boss.resistance_for(&ElementTag::Ether), 0.6);
        assert!(!boss.weaknesses.is_empty());
        assert!(boss.weaknesses.contains(&ElementTag::Fire));
        assert!(boss.weaknesses.contains(&ElementTag::Physical));
        assert_eq!(boss.stun_gauge, 0.0);
        assert!(!boss.is_stunned);
    }

    #[test]
    fn test_load_enemies_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();
        let loader = DataLoader { conn };
        let enemies = loader.load_enemies().expect("load enemies");
        assert!(enemies.is_empty());
    }

    // -----------------------------------------------------------------------
    // Skill tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_skills_by_character() {
        let loader = seed_skills_db();
        let skills = loader.load_skills("anby_demara").expect("load skills");

        assert_eq!(skills.len(), 2);

        let normal = skills
            .iter()
            .find(|s| s.action_id == "Attack_Normal_1")
            .unwrap();
        assert_eq!(normal.action_type, SkillType::Normal);
        assert_eq!(normal.daze_multiplier, 0.4);
        assert_eq!(normal.damage_multipliers.len(), 2);
        assert_eq!(normal.damage_multipliers[0].frame, 8);
        assert_eq!(normal.damage_multipliers[0].multiplier, 0.5);
        assert_eq!(normal.damage_multipliers[1].frame, 16);
        assert_eq!(normal.damage_multipliers[1].multiplier, 0.7);
        assert_eq!(normal.hit_frames, vec![8, 16]);
        assert!(!normal.is_snapshot);

        let ex = skills.iter().find(|s| s.action_id == "Skill_Ex_1").unwrap();
        assert_eq!(ex.action_type, SkillType::Special);
        assert_eq!(ex.daze_multiplier, 2.5);
        assert_eq!(ex.energy_cost, 40.0);
        assert_eq!(ex.damage_multipliers.len(), 1);
        assert_eq!(ex.damage_multipliers[0].frame, 15);
        assert_eq!(ex.damage_multipliers[0].multiplier, 3.0);
        assert_eq!(ex.hit_frames, vec![15]);
    }

    #[test]
    fn test_load_skills_nonexistent_character() {
        let loader = seed_skills_db();
        let skills = loader.load_skills("nonexistent").expect("load skills");
        assert!(skills.is_empty());
    }

    #[test]
    fn test_load_all_skills() {
        let loader = seed_skills_db();
        let all = loader.load_all_skills().expect("load all skills");
        assert_eq!(all.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Equipment tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_equipment_all_types() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();

        // W-Engine
        conn.execute(
            "INSERT INTO w_engines (id, name, level, ascension, atk, crit_rate, passive_effects)
             VALUES ('we_sharp_storm', 'Sharp Storm', 60, 6, 680.0, 0.24, '[\"passive_crit_dmg_20\"]')",
            [],
        ).unwrap();

        // Drive disc
        conn.execute(
            "INSERT INTO drive_discs (id, slot, level, set_id,
             main_stat_name, main_stat_value,
             sub_stat_1_name, sub_stat_1_value)
             VALUES ('dd_thunder_1', 1, 15, 'set_thunder_metal',
             'hp', 2200.0,
             'atk', 80.0)",
            [],
        )
        .unwrap();

        // Disc set
        conn.execute(
            "INSERT INTO disc_sets (set_id, name,
             two_piece_description, two_piece_buff_id,
             four_piece_description, four_piece_buff_id)
             VALUES ('set_thunder_metal', 'Thunder Metal',
             '+10% Electric DMG', 'buff_electric_dmg_10',
             'ATK +20%', 'buff_thunder_atk_20')",
            [],
        )
        .unwrap();

        let loader = DataLoader { conn };
        let eq = loader.load_equipment().expect("load equipment");

        assert_eq!(eq.w_engines.len(), 1);
        assert_eq!(eq.w_engines[0].id, "we_sharp_storm");
        assert_eq!(eq.w_engines[0].base_stats.atk, 680.0);
        assert_eq!(eq.w_engines[0].passive_effects.len(), 1);

        assert_eq!(eq.drive_discs.len(), 1);
        assert_eq!(eq.drive_discs[0].slot, 1);
        assert_eq!(eq.drive_discs[0].main_stat.value, 2200.0);
        assert_eq!(eq.drive_discs[0].sub_stats.len(), 1);

        assert_eq!(eq.disc_sets.len(), 1);
        assert_eq!(eq.disc_sets[0].set_id, "set_thunder_metal");
        assert!(eq.disc_sets[0].two_piece_bonus.is_some());
        assert!(eq.disc_sets[0].four_piece_bonus.is_some());
    }

    #[test]
    fn test_load_equipment_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();
        let loader = DataLoader { conn };
        let eq = loader.load_equipment().expect("load equipment");
        assert!(eq.w_engines.is_empty());
        assert!(eq.drive_discs.is_empty());
        assert!(eq.disc_sets.is_empty());
    }

    // -----------------------------------------------------------------------
    // APL tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_apl() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO apl (apl_id, name, tracks)
             VALUES ('sample_apl', 'Sample APL',
             '[{\"track_id\":\"track_01\",\"char_id\":\"anby_demara\",\"actions\":[{\"action_id\":\"Attack_Normal_1\",\"at\":0},{\"action_id\":\"Ultimate_1\",\"at\":300}]}]')",
            [],
        ).unwrap();

        let loader = DataLoader { conn };
        let apl = loader.load_apl("sample_apl").expect("load APL");
        assert_eq!(apl.tracks.len(), 1);
        assert_eq!(apl.tracks[0].track_id, "track_01");
        assert_eq!(apl.tracks[0].actions.len(), 2);
        assert_eq!(apl.tracks[0].actions[0].action_id, "Attack_Normal_1");
    }

    #[test]
    fn test_load_apl_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();
        let loader = DataLoader { conn };
        let result = loader.load_apl("nonexistent");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // from_json_dir integration test
    // -----------------------------------------------------------------------

    #[test]
    fn test_from_json_dir_characters() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        let db_path = tmp.path().join("zsim.db");

        write_json(
            &data_dir,
            "characters",
            "anby_demara.json",
            r#"{
                "char_id": "anby_demara",
                "name": "Anby Demara",
                "faction": "Gentle_House",
                "specialty": "Stun",
                "element": "Electric",
                "level": 60,
                "ascension": 6,
                "base_stats": {
                    "hp": 9500.0, "atk": 1100.0, "def": 550.0,
                    "impact": 120.0, "crit_rate": 0.15, "crit_dmg": 0.8,
                    "pen_ratio": 0.1, "pen_fixed": 40.0,
                    "anomaly_mastery": 80.0, "anomaly_proficiency": 95.0,
                    "energy_regen": 1.2, "energy_gen_rate": 0.3
                },
                "action_dict": ["Attack_Normal_1", "Skill_Ex_1"],
                "constellations": [true, false, false, false, false, false]
            }"#,
        );

        let loader = DataLoader::from_json_dir(&db_path, &data_dir).expect("from_json_dir");
        let characters = loader.load_characters().expect("load characters");
        assert_eq!(characters.len(), 1);
        assert_eq!(characters[0].name, "Anby Demara");
        assert_eq!(characters[0].base_stats.atk, 1100.0);
    }

    #[test]
    fn test_from_json_dir_empty_dirs() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        let db_path = tmp.path().join("zsim.db");

        // No data files — should succeed with empty tables
        fs::create_dir_all(data_dir.join("characters")).unwrap();
        fs::create_dir_all(data_dir.join("skills")).unwrap();
        fs::create_dir_all(data_dir.join("equipment")).unwrap();
        fs::create_dir_all(data_dir.join("enemies")).unwrap();
        fs::create_dir_all(data_dir.join("apl")).unwrap();

        let loader = DataLoader::from_json_dir(&db_path, &data_dir).expect("from_json_dir");
        assert!(loader.load_characters().unwrap().is_empty());
        assert!(loader.load_enemies().unwrap().is_empty());
        assert!(loader.load_all_skills().unwrap().is_empty());
    }

    #[test]
    fn test_from_json_dir_and_load_back() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        let db_path = tmp.path().join("zsim.db");

        write_json(
            &data_dir,
            "characters",
            "anby_demara.json",
            r#"{
            "char_id": "anby_demara",
            "name": "Anby Demara",
            "faction": "Gentle_House",
            "specialty": "Stun",
            "element": "Electric",
            "level": 60, "ascension": 6,
            "base_stats": {"hp": 9500.0, "atk": 1100.0, "def": 550.0,
                "impact": 120.0, "crit_rate": 0.15, "crit_dmg": 0.8,
                "pen_ratio": 0.1, "pen_fixed": 40.0,
                "anomaly_mastery": 80.0, "anomaly_proficiency": 95.0,
                "energy_regen": 1.2, "energy_gen_rate": 0.3},
            "action_dict": ["Attack_Normal_1"],
            "constellations": [true,false,false,false,false,false]
        }"#,
        );

        write_json(
            &data_dir,
            "enemies",
            "boss_dullahan.json",
            r#"{
            "enemy_id": "boss_dullahan",
            "enemy_type": "Boss",
            "hp": 150000.0,
            "def": 600.0,
            "base_res": 0.15,
            "daze_max": 200.0,
            "resistances": {"Ice": 0.4},
            "weaknesses": ["Fire"]
        }"#,
        );

        let loader = DataLoader::from_json_dir(&db_path, &data_dir).expect("from_json_dir");

        let characters = loader.load_characters().unwrap();
        assert_eq!(characters.len(), 1);
        assert_eq!(characters[0].char_id, "anby_demara");

        let enemies = loader.load_enemies().unwrap();
        assert_eq!(enemies.len(), 1);
        assert_eq!(enemies[0].enemy_id, "boss_dullahan");
        assert_eq!(enemies[0].resistance_for(&ElementTag::Ice), 0.4);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_characters_sorted() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('z_last', 'Z Last', 'Other', 'Attack', 'Fire')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('a_first', 'A First', 'Other', 'Support', 'Ice')",
            [],
        )
        .unwrap();

        let loader = DataLoader { conn };
        let chars = loader.load_characters().unwrap();
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].char_id, "a_first");
        assert_eq!(chars[1].char_id, "z_last");
    }

    #[test]
    fn test_skills_with_multipliers_ordered() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('c1', 'C1', 'Other', 'Attack', 'Fire')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO skills (char_id, action_id, action_type)
             VALUES ('c1', 'atk_normal', 'Normal')",
            [],
        )
        .unwrap();

        let skill_id: i64 = conn
            .query_row("SELECT id FROM skills", [], |row| row.get(0))
            .unwrap();

        // Insert out of order
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier) VALUES (?1, 3, 30, 1.2)",
            params![skill_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier) VALUES (?1, 1, 10, 0.5)",
            params![skill_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO skill_multipliers (skill_id, segment_index, frame, multiplier) VALUES (?1, 2, 20, 0.8)",
            params![skill_id],
        ).unwrap();

        let loader = DataLoader { conn };
        let skills = loader.load_skills("c1").unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].damage_multipliers.len(), 3);
        assert_eq!(skills[0].damage_multipliers[0].frame, 10);
        assert_eq!(skills[0].damage_multipliers[1].frame, 20);
        assert_eq!(skills[0].damage_multipliers[2].frame, 30);
        assert_eq!(skills[0].hit_frames, vec![10, 20, 30]);
    }

    #[test]
    fn test_new_opens_existing_db() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        // Create and seed
        {
            let conn = Connection::open(&db_path).unwrap();
            DataLoader::init_schema(&conn).unwrap();
            conn.execute(
                "INSERT INTO characters (char_id, name, faction, specialty, element)
                 VALUES ('test', 'Test', 'Other', 'Attack', 'Physical')",
                [],
            )
            .unwrap();
        }

        // Reopen with DataLoader::new
        let loader = DataLoader::new(&db_path).unwrap();
        let chars = loader.load_characters().unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].char_id, "test");
    }

    #[test]
    fn test_empty_database_returns_empty_vectors() {
        let conn = Connection::open_in_memory().unwrap();
        DataLoader::init_schema(&conn).unwrap();
        let loader = DataLoader { conn };

        assert!(loader.load_characters().unwrap().is_empty());
        assert!(loader.load_enemies().unwrap().is_empty());
        assert!(loader.load_all_skills().unwrap().is_empty());
        assert_eq!(loader.load_equipment().unwrap().w_engines.len(), 0);
        assert!(loader.load_apl("nonexistent").is_err());
    }

    #[test]
    fn test_new_returns_error_for_missing_file() {
        let result = DataLoader::new(Path::new("/nonexistent/path/db.sqlite"));
        assert!(result.is_err());
    }
}
