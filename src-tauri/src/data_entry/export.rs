use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};

use zsim_core::combat::skill::{HitFrame, SkillData};
use zsim_core::data::apl::APLData;
use zsim_core::data::equipment::{DiscBonus, DiscSet, DriveDisc, EquipmentData, StatEntry, WEngine};
use zsim_core::entities::character::Character;
use zsim_core::entities::enemy::EnemyState;
use zsim_core::entities::enums::{ElementTag, EnemyType, FactionTag, SkillType, SpecialtyTag};
use zsim_core::entities::models::BaseStats;

/// Parse a serde-rename string back into an enum.
fn parse_enum<T>(value: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let quoted = format!("\"{}\"", value);
    serde_json::from_str(&quoted).map_err(|e| format!("Failed to parse enum '{value}': {e}"))
}

/// Export all data from SQLite to JSON files in the given data directory.
/// Returns a summary of exported items per type.
pub fn export_all(conn: &Connection, data_dir: &Path) -> Result<String, String> {
    let mut summary = serde_json::Map::new();

    // Ensure output directories exist
    for subdir in &["characters", "skills", "equipment", "enemies", "apl"] {
        let dir = data_dir.join(subdir);
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create {dir:?}: {e}"))?;
    }

    // Export characters
    let char_count = export_characters(conn, &data_dir.join("characters"))?;
    summary.insert("characters".to_string(), serde_json::json!(char_count));

    // Export skills
    let skill_count = export_skills(conn, &data_dir.join("skills"))?;
    summary.insert("skills".to_string(), serde_json::json!(skill_count));

    // Export equipment
    export_equipment(conn, &data_dir.join("equipment"))?;
    summary.insert("equipment".to_string(), serde_json::json!(true));

    // Export enemies
    let enemy_count = export_enemies(conn, &data_dir.join("enemies"))?;
    summary.insert("enemies".to_string(), serde_json::json!(enemy_count));

    // Export APL
    let apl_count = export_apl(conn, &data_dir.join("apl"))?;
    summary.insert("apl".to_string(), serde_json::json!(apl_count));

    serde_json::to_string(&summary).map_err(|e| format!("Failed to serialize summary: {e}"))
}

// ── Character Export ──────────────────────────────────────

fn export_characters(conn: &Connection, dir: &Path) -> Result<usize, String> {
    let mut stmt = conn
        .prepare(
            "SELECT char_id, name, faction, specialty, element, level, ascension,
                    hp, atk, def, impact, crit_rate, crit_dmg,
                    pen_ratio, pen_fixed, anomaly_mastery, anomaly_proficiency,
                    energy_regen, energy_gen_rate, constellations, potentials, action_dict
             FROM characters",
        )
        .map_err(|e| format!("Failed to prepare characters query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i32>(5)?,
                row.get::<_, i32>(6)?,
                row.get::<_, f64>(7)?,
                row.get::<_, f64>(8)?,
                row.get::<_, f64>(9)?,
                row.get::<_, f64>(10)?,
                row.get::<_, f64>(11)?,
                row.get::<_, f64>(12)?,
                row.get::<_, f64>(13)?,
                row.get::<_, f64>(14)?,
                row.get::<_, f64>(15)?,
                row.get::<_, f64>(16)?,
                row.get::<_, f64>(17)?,
                row.get::<_, f64>(18)?,
                row.get::<_, String>(19)?,
                row.get::<_, String>(20)?,
                row.get::<_, String>(21)?,
            ))
        })
        .map_err(|e| format!("Failed to query characters: {e}"))?;

    let mut count = 0usize;
    for row in rows {
        let (
            char_id, name, faction_str, specialty_str, element_str,
            level, ascension, hp, atk, def, impact, crit_rate, crit_dmg,
            pen_ratio, pen_fixed, anomaly_mastery, anomaly_proficiency,
            energy_regen, energy_gen_rate, constellations_json, potentials_json, action_dict_json,
        ) = row.map_err(|e| format!("Failed to read character row: {e}"))?;

        let faction: FactionTag = parse_enum(&faction_str)?;
        let specialty: SpecialtyTag = parse_enum(&specialty_str)?;
        let element: ElementTag = parse_enum(&element_str)?;

        let constellations: [bool; 6] =
            serde_json::from_str(&constellations_json).unwrap_or([false; 6]);
        let potentials: [bool; 6] =
            serde_json::from_str(&potentials_json).unwrap_or([false; 6]);

        let action_dict: HashSet<String> =
            serde_json::from_str(&action_dict_json).unwrap_or_default();

        let base_stats = BaseStats {
            hp,
            atk,
            def,
            impact,
            crit_rate,
            crit_dmg,
            pen_ratio,
            pen: pen_fixed,
            anomaly_mastery,
            anomaly_proficiency,
            energy_regen,
            dmg_bonus: energy_gen_rate,
        };

        let character = Character {
            char_id,
            name,
            faction,
            specialty,
            element,
            level: level as u32,
            ascension: ascension as u32,
            base_stats: base_stats.clone(),
            current_stats: base_stats,
            resources: Default::default(),
            state: Default::default(),
            action_dict,
            constellations,
            potentials,
            realtime_modifiers: HashMap::new(),
        };

        let json = serde_json::to_string_pretty(&character)
            .map_err(|e| format!("Failed to serialize character: {e}"))?;

        let file_path = dir.join(format!("{}.json", character.char_id));
        fs::write(&file_path, &json)
            .map_err(|e| format!("Failed to write {file_path:?}: {e}"))?;
        count += 1;
    }

    Ok(count)
}

// ── Skill Export ──────────────────────────────────────────

fn export_skills(conn: &Connection, dir: &Path) -> Result<usize, String> {
    // Get distinct char_ids
    let mut char_stmt = conn
        .prepare("SELECT DISTINCT char_id FROM skills ORDER BY char_id")
        .map_err(|e| format!("Failed to prepare char_id query: {e}"))?;

    let char_ids: Vec<String> = char_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query char_ids: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let mut total_skills = 0usize;

    // Preload all skill_multipliers
    let mut mult_lookup: HashMap<i64, Vec<HitFrame>> = HashMap::new();
    {
        let mut mult_stmt = conn
            .prepare(
                "SELECT skill_id, frame, multiplier, decay_coeff
                 FROM skill_multipliers ORDER BY skill_id, segment_index",
            )
            .map_err(|e| format!("Failed to prepare multipliers query: {e}"))?;

        let mult_rows = mult_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    HitFrame {
                        frame: row.get::<_, i64>(1)? as u64,
                        multiplier: row.get::<_, f64>(2)?,
                    },
                ))
            })
            .map_err(|e| format!("Failed to query multipliers: {e}"))?;

        for row in mult_rows {
            let (skill_id, hf) = row.map_err(|e| format!("Failed to read multiplier: {e}"))?;
            mult_lookup.entry(skill_id).or_default().push(hf);
        }
    }

    for char_id in &char_ids {
        let mut stmt = conn
            .prepare(
                "SELECT id, action_id, action_type, daze_multiplier,
                        energy_cost, decibel_cost, hp_cost, cooldown_ticks,
                        animation_frames, interruptible_frame, is_snapshot,
                        prerequisite_action_id
                 FROM skills WHERE char_id = ?1 ORDER BY action_id",
            )
            .map_err(|e| format!("Failed to prepare skills query: {e}"))?;

        let rows = stmt
            .query_map(params![char_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, f64>(5)?,
                    row.get::<_, f64>(6)?,
                    row.get::<_, i32>(7)?,
                    row.get::<_, i32>(8)?,
                    row.get::<_, Option<i32>>(9)?,
                    row.get::<_, i32>(10)?,
                    row.get::<_, Option<String>>(11)?,
                ))
            })
            .map_err(|e| format!("Failed to query skills: {e}"))?;

        let mut skills: Vec<SkillData> = Vec::new();
        for row in rows {
            let (
                db_id, action_id, action_type_str, daze_multiplier,
                energy_cost, decibel_cost, hp_cost, cooldown_ticks,
                animation_frames, interruptible_frame, is_snapshot,
                prerequisite_action_id,
            ) = row.map_err(|e| format!("Failed to read skill row: {e}"))?;

            let action_type: SkillType = parse_enum(&action_type_str)?;

            let damage_multipliers = mult_lookup.remove(&db_id).unwrap_or_default();
            let hit_frames: Vec<u64> = damage_multipliers
                .iter()
                .map(|hf| hf.frame)
                .collect();

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

        let json = serde_json::to_string_pretty(&skills)
            .map_err(|e| format!("Failed to serialize skills: {e}"))?;

        let file_path = dir.join(format!("{char_id}.json"));
        fs::write(&file_path, &json)
            .map_err(|e| format!("Failed to write {file_path:?}: {e}"))?;

        total_skills += skills.len();
    }

    Ok(total_skills)
}

// ── Equipment Export ──────────────────────────────────────

fn export_equipment(conn: &Connection, dir: &Path) -> Result<(), String> {
    // Export W-Engines
    let mut w_engines: Vec<WEngine> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, level, ascension,
                        atk, crit_rate, crit_dmg, pen_ratio, energy_regen, impact, anomaly_mastery,
                        passive_effects
                 FROM w_engines",
            )
            .map_err(|e| format!("Failed to prepare w_engines query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, f64>(5)?,
                    row.get::<_, f64>(6)?,
                    row.get::<_, f64>(7)?,
                    row.get::<_, f64>(8)?,
                    row.get::<_, f64>(9)?,
                    row.get::<_, f64>(10)?,
                    row.get::<_, String>(11)?,
                ))
            })
            .map_err(|e| format!("Failed to query w_engines: {e}"))?;

        for row in rows {
            let (
                id, name, level, ascension,
                atk, crit_rate, crit_dmg, pen_ratio, energy_regen, impact, anomaly_mastery,
                passive_effects_json,
            ) = row.map_err(|e| format!("Failed to read w_engine: {e}"))?;

            let passive_effects: Vec<String> =
                serde_json::from_str(&passive_effects_json).unwrap_or_default();

            w_engines.push(WEngine {
                id,
                name,
                level: level as u32,
                ascension: ascension as u32,
                base_stats: BaseStats {
                    atk,
                    crit_rate,
                    crit_dmg,
                    pen_ratio,
                    energy_regen,
                    impact,
                    anomaly_mastery,
                    ..Default::default()
                },
                passive_effects,
            });
        }
    }

    // Export Drive Discs
    let mut drive_discs: Vec<DriveDisc> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, slot, level, set_id,
                        main_stat_name, main_stat_value,
                        sub_stat_1_name, sub_stat_1_value,
                        sub_stat_2_name, sub_stat_2_value,
                        sub_stat_3_name, sub_stat_3_value,
                        sub_stat_4_name, sub_stat_4_value
                 FROM drive_discs",
            )
            .map_err(|e| format!("Failed to prepare drive_discs query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i32>(2)?,
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
            })
            .map_err(|e| format!("Failed to query drive_discs: {e}"))?;

        for row in rows {
            let (
                id, slot, level, set_id,
                ms_name, ms_value,
                ss1_name, ss1_value,
                ss2_name, ss2_value,
                ss3_name, ss3_value,
                ss4_name, ss4_value,
            ) = row.map_err(|e| format!("Failed to read drive_disc: {e}"))?;

            let main_stat = StatEntry {
                stat_name: ms_name,
                value: ms_value,
            };

            let mut sub_stats = Vec::new();
            let sub_pairs = [
                (ss1_name, ss1_value),
                (ss2_name, ss2_value),
                (ss3_name, ss3_value),
                (ss4_name, ss4_value),
            ];
            for (name, value) in sub_pairs {
                if let (Some(n), Some(v)) = (name, value) {
                    sub_stats.push(StatEntry {
                        stat_name: n,
                        value: v,
                    });
                }
            }

            drive_discs.push(DriveDisc {
                id,
                slot: slot as u8,
                level: level as u32,
                set_id,
                main_stat,
                sub_stats,
            });
        }
    }

    // Export Disc Sets
    let mut disc_sets: Vec<DiscSet> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT set_id, name,
                        two_piece_description, two_piece_buff_id,
                        four_piece_description, four_piece_buff_id
                 FROM disc_sets",
            )
            .map_err(|e| format!("Failed to prepare disc_sets query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(|e| format!("Failed to query disc_sets: {e}"))?;

        for row in rows {
            let (set_id, name, tp_desc, tp_buff, fp_desc, fp_buff) =
                row.map_err(|e| format!("Failed to read disc_set: {e}"))?;

            let two_piece_bonus = if tp_desc.is_some() || tp_buff.is_some() {
                Some(DiscBonus {
                    description: tp_desc.clone().unwrap_or_default(),
                    buff_id: tp_buff.clone().unwrap_or_default(),
                })
            } else {
                None
            };

            let four_piece_bonus = if fp_desc.is_some() || fp_buff.is_some() {
                Some(DiscBonus {
                    description: fp_desc.unwrap_or_default(),
                    buff_id: fp_buff.unwrap_or_default(),
                })
            } else {
                None
            };

            disc_sets.push(DiscSet {
                set_id,
                name,
                two_piece_bonus,
                four_piece_bonus,
            });
        }
    }

    let equipment = EquipmentData {
        w_engines,
        drive_discs,
        disc_sets,
    };

    let json = serde_json::to_string_pretty(&equipment)
        .map_err(|e| format!("Failed to serialize equipment: {e}"))?;

    let file_path = dir.join("equipment.json");
    fs::write(&file_path, &json)
        .map_err(|e| format!("Failed to write {file_path:?}: {e}"))?;

    Ok(())
}

// ── Enemy Export ──────────────────────────────────────────

fn export_enemies(conn: &Connection, dir: &Path) -> Result<usize, String> {
    let mut stmt = conn
        .prepare(
            "SELECT enemy_id, enemy_type, level, hp, def, base_res, daze_max,
                    resistances, weaknesses, anomaly_buildup
             FROM enemies",
        )
        .map_err(|e| format!("Failed to prepare enemies query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(|e| format!("Failed to query enemies: {e}"))?;

    let mut count = 0usize;
    for row in rows {
        let (
            enemy_id, enemy_type_str, _level, hp, def_val, base_res, daze_max,
            resistances_json, weaknesses_json, anomaly_buildup_json,
        ) = row.map_err(|e| format!("Failed to read enemy row: {e}"))?;

        let enemy_type: EnemyType = parse_enum(&enemy_type_str)?;

        let resistances: HashMap<ElementTag, f64> =
            serde_json::from_str(&resistances_json).unwrap_or_default();
        let weaknesses: HashSet<ElementTag> =
            serde_json::from_str(&weaknesses_json).unwrap_or_default();
        let anomaly_buildup: HashMap<ElementTag, f64> =
            serde_json::from_str(&anomaly_buildup_json).unwrap_or_default();

        let enemy = EnemyState {
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
        };

        let json = serde_json::to_string_pretty(&enemy)
            .map_err(|e| format!("Failed to serialize enemy: {e}"))?;

        let file_path = dir.join(format!("{}.json", enemy.enemy_id));
        fs::write(&file_path, &json)
            .map_err(|e| format!("Failed to write {file_path:?}: {e}"))?;
        count += 1;
    }

    Ok(count)
}

// ── APL Export ────────────────────────────────────────────

fn export_apl(conn: &Connection, dir: &Path) -> Result<usize, String> {
    let mut stmt = conn
        .prepare("SELECT apl_id, name, tracks FROM apl")
        .map_err(|e| format!("Failed to prepare apl query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("Failed to query apl: {e}"))?;

    let mut count = 0usize;
    for row in rows {
        let (apl_id, _name, tracks_json) =
            row.map_err(|e| format!("Failed to read apl row: {e}"))?;

        let tracks: Vec<zsim_core::data::apl::Track> =
            serde_json::from_str(&tracks_json).unwrap_or_default();

        let apl = APLData { tracks };

        let json = serde_json::to_string_pretty(&apl)
            .map_err(|e| format!("Failed to serialize APL: {e}"))?;

        let file_path = dir.join(format!("{apl_id}.json"));
        fs::write(&file_path, &json)
            .map_err(|e| format!("Failed to write {file_path:?}: {e}"))?;
        count += 1;
    }

    Ok(count)
}
