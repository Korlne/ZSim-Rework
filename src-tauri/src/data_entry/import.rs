use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use zsim_core::combat::skill::SkillData;
use zsim_core::data::apl::APLData;
use zsim_core::data::equipment::EquipmentData;
use zsim_core::entities::character::Character;
use zsim_core::entities::enemy::EnemyState;

// ---------------------------------------------------------------------------
// 结果类型
// ---------------------------------------------------------------------------

/// 单个数据类型的导入结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub success: usize,
    pub errors: Vec<String>,
}

impl ImportResult {
    fn new() -> Self {
        Self {
            success: 0,
            errors: Vec::new(),
        }
    }

    fn add_success(&mut self) {
        self.success += 1;
    }

    fn add_error(&mut self, msg: String) {
        self.errors.push(msg);
    }

    pub fn total(&self) -> usize {
        self.success + self.errors.len()
    }
}

// 用于将 enum 序列化为 serde rename 字符串的辅助函数。
fn enum_to_string<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

// 将 HashSet<String> 序列化为 JSON 数组字符串。
fn set_to_json_string(set: &std::collections::HashSet<String>) -> String {
    let mut items: Vec<&String> = set.iter().collect();
    items.sort();
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
}

// ---------------------------------------------------------------------------
// 角色导入
// ---------------------------------------------------------------------------

/// 从 `{data_dir}/characters/*.json` 导入所有角色。
pub fn import_characters(conn: &Connection, data_dir: &Path) -> ImportResult {
    let mut result = ImportResult::new();
    let dir = data_dir.join("characters");

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            result.add_error(format!("Cannot read characters directory '{}': {e}", dir.display()));
            return result;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(format!("Failed to read {}: {e}", path.display()));
                continue;
            }
        };

        let character: Character = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(format!("Failed to parse {}: {e}", path.display()));
                continue;
            }
        };

        let faction_str = enum_to_string(&character.faction);
        let specialty_str = enum_to_string(&character.specialty);
        let element_str = enum_to_string(&character.element);
        let constellations_json =
            serde_json::to_string(&character.constellations).unwrap_or_else(|_| "[false,false,false,false,false,false]".to_string());
        let potentials_json =
            serde_json::to_string(&character.potentials).unwrap_or_else(|_| "[false,false,false,false,false,false]".to_string());
        let action_dict_json = set_to_json_string(&character.action_dict);

        if let Err(e) = conn.execute(
             "INSERT OR REPLACE INTO characters
              (char_id, name, faction, specialty, element, level, ascension,
               hp, atk, def, impact, crit_rate, crit_dmg, pen_ratio, pen_fixed,
               anomaly_mastery, anomaly_proficiency, energy_regen, energy_gen_rate,
               constellations, potentials, action_dict, updated_at)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                      ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                      ?16, ?17, ?18, ?19,
                      ?20, ?21, ?22, datetime('now'))",
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
                potentials_json,
                action_dict_json,
            ],
        ) {
            result.add_error(format!("DB error importing {}: {e}", character.char_id));
        } else {
            result.add_success();
        }
    }

    result
}

// ---------------------------------------------------------------------------
// 技能导入
// ---------------------------------------------------------------------------

/// 从 `{data_dir}/skills/*.json` 导入所有技能及倍率段。
pub fn import_skills(conn: &Connection, data_dir: &Path) -> ImportResult {
    let mut result = ImportResult::new();
    let dir = data_dir.join("skills");

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            result.add_error(format!("Cannot read skills directory '{}': {e}", dir.display()));
            return result;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(format!("Failed to read {}: {e}", path.display()));
                continue;
            }
        };

        // 技能文件是 SkillData 数组。
        let skills: Vec<SkillData> = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                result.add_error(format!("Failed to parse {}: {e}", path.display()));
                continue;
            }
        };

        // 从文件名推断 char_id（去掉 .json 后缀）。
        let char_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        for skill in &skills {
            let action_type_str = enum_to_string(&skill.action_type);
            let is_snapshot = if skill.is_snapshot { 1 } else { 0 };
            let prereq = skill.prerequisite_action_id.as_deref();

            let result_str = conn.execute(
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
                    skill.interruptible_frame,
                    is_snapshot,
                    prereq,
                ],
            );

            match result_str {
                Ok(_) => {
                    // 获取或创建 skill_id (INSERT OR REPLACE 后可能改变 id，所以我们查询对应的行)。
                    let skill_id: i64 = match conn.query_row(
                        "SELECT id FROM skills WHERE char_id = ?1 AND action_id = ?2",
                        params![char_id, skill.action_id],
                        |row| row.get(0),
                    ) {
                        Ok(id) => id,
                        Err(e) => {
                            result.add_error(format!(
                                "Cannot find skill_id for {}/{}: {e}",
                                char_id, skill.action_id
                            ));
                            continue;
                        }
                    };

                    // 删除该技能已有的倍率段（处理重新导入），然后插入新的。
                    if let Err(e) = conn.execute(
                        "DELETE FROM skill_multipliers WHERE skill_id = ?1",
                        params![skill_id],
                    ) {
                        result.add_error(format!(
                            "Failed to clear multipliers for skill {}: {e}",
                            skill_id
                        ));
                        continue;
                    }

                    for (idx, hf) in skill.damage_multipliers.iter().enumerate() {
                        if let Err(e) = conn.execute(
                            "INSERT INTO skill_multipliers
                             (skill_id, segment_index, frame, multiplier, decay_coeff)
                             VALUES (?1, ?2, ?3, ?4, 1.0)",
                            params![skill_id, idx as i64 + 1, hf.frame, hf.multiplier],
                        ) {
                            result.add_error(format!(
                                "Failed to insert multiplier for skill {} (seg {}): {e}",
                                skill.action_id, idx
                            ));
                        }
                    }

                    result.add_success();
                }
                Err(e) => {
                    result.add_error(format!(
                        "DB error importing skill {}/{}: {e}",
                        char_id, skill.action_id
                    ));
                }
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// 装备导入
// ---------------------------------------------------------------------------

/// 从 `{data_dir}/equipment/equipment.json` 导入音擎、驱动盘和套装。
pub fn import_equipment(conn: &Connection, data_dir: &Path) -> ImportResult {
    let mut result = ImportResult::new();
    let path = data_dir.join("equipment").join("equipment.json");

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            result.add_error(format!("Failed to read {}: {e}", path.display()));
            return result;
        }
    };

    let equipment: EquipmentData = match serde_json::from_str(&content) {
        Ok(e) => e,
        Err(e) => {
            result.add_error(format!("Failed to parse {}: {e}", path.display()));
            return result;
        }
    };

    // 导入音擎
    for we in &equipment.w_engines {
        let passive_json =
            serde_json::to_string(&we.passive_effects).unwrap_or_else(|_| "[]".to_string());

        if let Err(e) = conn.execute(
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
        ) {
            result.add_error(format!("DB error importing w_engine {}: {e}", we.id));
        } else {
            result.add_success();
        }
    }

    // 导入驱动盘
    for dd in &equipment.drive_discs {
        // 将 sub_stats 展开到 4 个副属性列
        let sub = &dd.sub_stats;
        let (sn1, sv1) = sub.get(0).map(|s| (s.stat_name.as_str(), s.value)).unwrap_or(("", 0.0));
        let (sn2, sv2) = sub.get(1).map(|s| (s.stat_name.as_str(), s.value)).unwrap_or(("", 0.0));
        let (sn3, sv3) = sub.get(2).map(|s| (s.stat_name.as_str(), s.value)).unwrap_or(("", 0.0));
        let (sn4, sv4) = sub.get(3).map(|s| (s.stat_name.as_str(), s.value)).unwrap_or(("", 0.0));

        if let Err(e) = conn.execute(
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
                sn1, sv1,
                sn2, sv2,
                sn3, sv3,
                sn4, sv4,
            ],
        ) {
            result.add_error(format!("DB error importing drive_disc {}: {e}", dd.id));
        } else {
            result.add_success();
        }
    }

    // 导入套装
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

        if let Err(e) = conn.execute(
            "INSERT OR REPLACE INTO disc_sets
             (set_id, name,
              two_piece_description, two_piece_buff_id,
              four_piece_description, four_piece_buff_id,
              updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            params![ds.set_id, ds.name, tp_desc, tp_buff, fp_desc, fp_buff],
        ) {
            result.add_error(format!("DB error importing disc_set {}: {e}", ds.set_id));
        } else {
            result.add_success();
        }
    }

    result
}

// ---------------------------------------------------------------------------
// 敌人导入
// ---------------------------------------------------------------------------

/// 从 `{data_dir}/enemies/*.json` 导入所有敌人。
pub fn import_enemies(conn: &Connection, data_dir: &Path) -> ImportResult {
    let mut result = ImportResult::new();
    let dir = data_dir.join("enemies");

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            result.add_error(format!("Cannot read enemies directory '{}': {e}", dir.display()));
            return result;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(format!("Failed to read {}: {e}", path.display()));
                continue;
            }
        };

        let enemy: EnemyState = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(e) => {
                result.add_error(format!("Failed to parse {}: {e}", path.display()));
                continue;
            }
        };

        // 按 enemy_id 取文件名，回退到 struct 中的 enemy_id。
        let enemy_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&enemy.enemy_id)
            .to_string();

        let enemy_type_str = enum_to_string(&enemy.enemy_type);
        let resistances_json =
            serde_json::to_string(&enemy.resistances).unwrap_or_else(|_| "{}".to_string());
        let weaknesses_json =
            serde_json::to_string(&enemy.weaknesses).unwrap_or_else(|_| "[]".to_string());
        let anomaly_buildup_json =
            serde_json::to_string(&enemy.anomaly_buildup).unwrap_or_else(|_| "{}".to_string());

        // 敌人的 level 字段在 JSON 中不存在，默认使用 60。
        let enemy_level: i64 = 60;

        if let Err(e) = conn.execute(
            "INSERT OR REPLACE INTO enemies
             (enemy_id, enemy_type, level, hp, def, base_res, daze_max,
              resistances, weaknesses, anomaly_buildup, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))",
            params![
                enemy_id,
                enemy_type_str,
                enemy_level,
                enemy.hp,
                enemy.def_val,
                enemy.base_res,
                enemy.stun_max,
                resistances_json,
                weaknesses_json,
                anomaly_buildup_json,
            ],
        ) {
            result.add_error(format!("DB error importing enemy {enemy_id}: {e}"));
        } else {
            result.add_success();
        }
    }

    result
}

// ---------------------------------------------------------------------------
// APL 导入
// ---------------------------------------------------------------------------

/// 从 `{data_dir}/apl/*.json` 导入所有 APL 排轴数据。
pub fn import_apl(conn: &Connection, data_dir: &Path) -> ImportResult {
    let mut result = ImportResult::new();
    let dir = data_dir.join("apl");

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            result.add_error(format!("Cannot read apl directory '{}': {e}", dir.display()));
            return result;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(format!("Failed to read {}: {e}", path.display()));
                continue;
            }
        };

        let apl: APLData = match serde_json::from_str(&content) {
            Ok(a) => a,
            Err(e) => {
                result.add_error(format!("Failed to parse {}: {e}", path.display()));
                continue;
            }
        };

        let apl_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let tracks_json = serde_json::to_string(&apl.tracks).unwrap_or_else(|_| "[]".to_string());

        if let Err(e) = conn.execute(
            "INSERT OR REPLACE INTO apl (apl_id, name, tracks, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            params![apl_id, apl_id, tracks_json],
        ) {
            result.add_error(format!("DB error importing APL {apl_id}: {e}"));
        } else {
            result.add_success();
        }
    }

    result
}

// ---------------------------------------------------------------------------
// 批量导入
// ---------------------------------------------------------------------------

/// 导入所有种类的数据。返回 Vec<(类型名, ImportResult)>。
pub fn import_all(conn: &Connection, data_dir: &Path) -> Vec<(&'static str, ImportResult)> {
    vec![
        ("characters", import_characters(conn, data_dir)),
        ("skills", import_skills(conn, data_dir)),
        ("equipment", import_equipment(conn, data_dir)),
        ("enemies", import_enemies(conn, data_dir)),
        ("apl", import_apl(conn, data_dir)),
    ]
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_entry::db::init_db;
    use std::io::Write;
    use tempfile::TempDir;

    /// 写入一个 JSON 文件到临时目录的子目录中，并确保父目录存在。
    fn write_json(dir: &Path, subdir: &str, filename: &str, content: &str) {
        let d = dir.join(subdir);
        fs::create_dir_all(&d).expect("create subdir");
        let mut f = fs::File::create(d.join(filename)).expect("create file");
        f.write_all(content.as_bytes()).expect("write file");
    }

    #[test]
    fn test_import_characters_empty_dir() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();
        // Create the subdirectory but leave it empty
        fs::create_dir_all(tmp.path().join("characters")).unwrap();
        let result = import_characters(&conn, tmp.path());
        assert_eq!(result.success, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_import_characters_single() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();

        write_json(
            tmp.path(),
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

        let result = import_characters(&conn, tmp.path());
        assert_eq!(result.success, 1, "one character imported");
        assert!(result.errors.is_empty(), "no errors: {:?}", result.errors);

        // 验证数据库内容
        let (name, faction, hp, atk): (String, String, f64, f64) = conn
            .query_row(
                "SELECT name, faction, hp, atk FROM characters WHERE char_id = 'anby_demara'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(name, "Anby Demara");
        assert_eq!(faction, "Gentle_House");
        assert_eq!(hp, 9500.0);
        assert_eq!(atk, 1100.0);

        // verify pen_fixed maps to pen column
        let pen: f64 = conn
            .query_row(
                "SELECT pen_fixed FROM characters WHERE char_id = 'anby_demara'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pen, 40.0);

        // verify action_dict as JSON
        let ad: String = conn
            .query_row(
                "SELECT action_dict FROM characters WHERE char_id = 'anby_demara'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(ad.contains("Attack_Normal_1"));
        assert!(ad.contains("Skill_Ex_1"));
    }

    #[test]
    fn test_import_characters_malformed_file_skipped() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();

        write_json(tmp.path(), "characters", "bad.json", "this is not json");

        let result = import_characters(&conn, tmp.path());
        assert_eq!(result.success, 0);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("bad.json"));
    }

    #[test]
    fn test_import_skills_empty_dir() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();
        // Create the subdirectory but leave it empty
        fs::create_dir_all(tmp.path().join("skills")).unwrap();
        let result = import_skills(&conn, tmp.path());
        assert_eq!(result.success, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_import_skills_with_multipliers() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();

        // 需要先有角色（外键约束）
        conn.execute(
            "INSERT INTO characters (char_id, name, faction, specialty, element)
             VALUES ('anby_demara', 'Anby', 'Gentle_House', 'Stun', 'Electric')",
            [],
        )
        .unwrap();

        write_json(
            tmp.path(),
            "skills",
            "anby_demara.json",
            r#"[
                {
                    "action_id": "Attack_Normal_1",
                    "action_type": "Normal",
                    "damage_multipliers": [
                        {"frame": 8, "multiplier": 0.5},
                        {"frame": 16, "multiplier": 0.7}
                    ],
                    "daze_multiplier": 0.4,
                    "hit_frames": [8, 16],
                    "interruptible_frame": 25,
                    "is_snapshot": false,
                    "charge_branches": [],
                    "prerequisite_action_id": null,
                    "hp_cost": 0.0,
                    "energy_cost": 0.0,
                    "decibel_cost": 0.0,
                    "cooldown_ticks": 0,
                    "animation_frames": 30
                },
                {
                    "action_id": "Skill_Ex_1",
                    "action_type": "Special",
                    "damage_multipliers": [
                        {"frame": 15, "multiplier": 3.0}
                    ],
                    "daze_multiplier": 2.5,
                    "hit_frames": [15],
                    "interruptible_frame": 45,
                    "is_snapshot": false,
                    "charge_branches": [],
                    "prerequisite_action_id": null,
                    "hp_cost": 0.0,
                    "energy_cost": 40.0,
                    "decibel_cost": 0.0,
                    "cooldown_ticks": 8,
                    "animation_frames": 50
                }
            ]"#,
        );

        let result = import_skills(&conn, tmp.path());
        assert_eq!(result.success, 2, "two skills imported");
        assert!(result.errors.is_empty(), "no errors: {:?}", result.errors);

        // 验证技能数据
        let skill_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skills WHERE char_id = 'anby_demara'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(skill_count, 2);

        // 验证倍率段
        let mult_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skill_multipliers sm
                 JOIN skills s ON sm.skill_id = s.id
                 WHERE s.action_id = 'Attack_Normal_1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mult_count, 2);
    }

    #[test]
    fn test_import_equipment_all_types() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();

        write_json(
            tmp.path(),
            "equipment",
            "equipment.json",
            r#"{
                "w_engines": [
                    {
                        "id": "we_sharp_storm",
                        "name": "Sharp Storm",
                        "level": 60,
                        "ascension": 6,
                        "base_stats": {"atk": 680.0, "crit_rate": 0.24},
                        "passive_effects": ["passive_crit_dmg_20"]
                    }
                ],
                "drive_discs": [
                    {
                        "id": "dd_thunder_1",
                        "slot": 1,
                        "level": 15,
                        "main_stat": {"stat_name": "hp", "value": 2200.0},
                        "sub_stats": [
                            {"stat_name": "atk", "value": 80.0}
                        ],
                        "set_id": "set_thunder_metal"
                    }
                ],
                "disc_sets": [
                    {
                        "set_id": "set_thunder_metal",
                        "name": "Thunder Metal",
                        "two_piece_bonus": {
                            "description": "+10% Electric DMG",
                            "buff_id": "buff_electric_dmg_10"
                        },
                        "four_piece_bonus": {
                            "description": "ATK +20%",
                            "buff_id": "buff_thunder_atk_20"
                        }
                    }
                ]
            }"#,
        );

        let result = import_equipment(&conn, tmp.path());
        assert_eq!(result.success, 3, "1 w_engine + 1 drive_disc + 1 disc_set");
        assert!(result.errors.is_empty(), "no errors: {:?}", result.errors);

        // Verify w_engines
        let we_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM w_engines", [], |row| row.get(0))
            .unwrap();
        assert_eq!(we_count, 1);

        // Verify drive_discs
        let dd_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM drive_discs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(dd_count, 1);

        // Verify disc_sets
        let ds_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM disc_sets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(ds_count, 1);
    }

    #[test]
    fn test_import_enemies_single() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();

        write_json(
            tmp.path(),
            "enemies",
            "boss_dullahan.json",
            r#"{
                "enemy_id": "boss_dullahan",
                "enemy_type": "Boss",
                "hp": 150000.0,
                "def": 600.0,
                "base_res": 0.15,
                "daze_current": 0.0,
                "daze_max": 200.0,
                "resistances": {"Ice": 0.4, "Ether": 0.6},
                "weaknesses": ["Fire", "Physical"],
                "anomaly_buildup": {}
            }"#,
        );

        let result = import_enemies(&conn, tmp.path());
        assert_eq!(result.success, 1);
        assert!(result.errors.is_empty());

        // 验证数据库
        let (enemy_type, hp, def_val): (String, f64, f64) = conn
            .query_row(
                "SELECT enemy_type, hp, def FROM enemies WHERE enemy_id = 'boss_dullahan'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(enemy_type, "Boss");
        assert_eq!(hp, 150000.0);
        assert_eq!(def_val, 600.0);
    }

    #[test]
    fn test_import_apl_single() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();

        write_json(
            tmp.path(),
            "apl",
            "sample_apl.json",
            r#"{
                "tracks": [
                    {
                        "track_id": "track_01",
                        "char_id": "anby_demara",
                        "actions": [
                            {"action_id": "Attack_Normal_1", "at": 0},
                            {"action_id": "Ultimate_1", "at": 300}
                        ]
                    }
                ]
            }"#,
        );

        let result = import_apl(&conn, tmp.path());
        assert_eq!(result.success, 1);
        assert!(result.errors.is_empty());

        // 验证 tracks 存储为 JSON
        let tracks: String = conn
            .query_row(
                "SELECT tracks FROM apl WHERE apl_id = 'sample_apl'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(tracks.contains("track_01"));
        assert!(tracks.contains("Attack_Normal_1"));
    }

    #[test]
    fn test_import_all_skips_missing_directory() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();
        // 空的临时目录 — 应该优雅处理缺少每个子目录的情况
        let results = import_all(&conn, tmp.path());
        for (name, r) in &results {
            assert_eq!(r.success, 0, "{name} should have 0 success");
            // 每个没有对应子目录的类型都应该有错误
            assert!(!r.errors.is_empty(), "{name} should have an error for missing directory");
        }
    }

    #[test]
    fn test_import_characters_reimport_updates() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();

        write_json(
            tmp.path(),
            "characters",
            "test_char.json",
            r#"{
                "char_id": "test_char",
                "name": "Original",
                "faction": "Gentle_House",
                "specialty": "Attack",
                "element": "Fire",
                "level": 50,
                "ascension": 4,
                "base_stats": {"hp": 5000.0, "atk": 800.0, "def": 300.0,
                    "impact": 90.0, "crit_rate": 0.1, "crit_dmg": 0.5,
                    "pen_ratio": 0.0, "pen_fixed": 0.0,
                    "anomaly_mastery": 60.0, "anomaly_proficiency": 70.0,
                    "energy_regen": 1.0, "energy_gen_rate": 0.2},
                "action_dict": [],
                "constellations": [false, false, false, false, false, false]
            }"#,
        );

        import_characters(&conn, tmp.path());

        // 重新导入并修改
        write_json(
            tmp.path(),
            "characters",
            "test_char.json",
            r#"{
                "char_id": "test_char",
                "name": "Updated",
                "faction": "Belobog_Heavy_Ind",
                "specialty": "Defense",
                "element": "Ice",
                "level": 60,
                "ascension": 6,
                "base_stats": {"hp": 10000.0, "atk": 1200.0, "def": 600.0,
                    "impact": 110.0, "crit_rate": 0.15, "crit_dmg": 0.8,
                    "pen_ratio": 0.1, "pen_fixed": 40.0,
                    "anomaly_mastery": 80.0, "anomaly_proficiency": 90.0,
                    "energy_regen": 1.2, "energy_gen_rate": 0.3},
                "action_dict": ["New_Action"],
                "constellations": [true, false, false, false, false, false]
            }"#,
        );

        let result = import_characters(&conn, tmp.path());
        assert_eq!(result.success, 1);

        // 验证数据已更新
        let (name, level): (String, i64) = conn
            .query_row(
                "SELECT name, level FROM characters WHERE char_id = 'test_char'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "Updated");
        assert_eq!(level, 60);

        // 验证只有一个角色
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM characters", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_import_skills_without_character_graceful() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let tmp = TempDir::new().unwrap();

        // 创建技能文件，但无对应的角色记录（外键约束会失败）
        write_json(
            tmp.path(),
            "skills",
            "ghost_char.json",
            r#"[
                {
                    "action_id": "Attack_1",
                    "action_type": "Normal",
                    "damage_multipliers": [],
                    "daze_multiplier": 0.0,
                    "hit_frames": [],
                    "interruptible_frame": 0,
                    "is_snapshot": false,
                    "charge_branches": [],
                    "prerequisite_action_id": null,
                    "hp_cost": 0.0,
                    "energy_cost": 0.0,
                    "decibel_cost": 0.0,
                    "cooldown_ticks": 0,
                    "animation_frames": 1
                }
            ]"#,
        );

        // 应有外键约束错误，但不应崩溃
        let result = import_skills(&conn, tmp.path());
        // FOREIGN KEY 失败，所以 success=0，errors=1
        assert_eq!(result.success, 0);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("FOREIGN KEY") || result.errors[0].contains("constraint"));
    }
}
