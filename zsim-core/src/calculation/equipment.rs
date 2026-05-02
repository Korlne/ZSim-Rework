use std::collections::HashMap;

use crate::calculation::buff::{BuffCategory, BuffData, BuffManager, StackType};
use crate::data::equipment::{DiscSet, DriveDisc, EquipmentData, WEngine};

/// 分配给永久装备属性增益的持续时间（覆盖最大模拟时长）。
pub const EQUIPMENT_BUFF_DURATION: u64 = 18_000;

/// 每个角色的装备分配：一把音擎（W-Engine）和最多 6 个驱动盘（Drive Disc）。
#[derive(Debug, Clone)]
pub struct CharacterEquipment {
    pub w_engine: Option<WEngine>,
    pub drive_discs: Vec<DriveDisc>,
}

impl CharacterEquipment {
    pub fn new() -> Self {
        CharacterEquipment {
            w_engine: None,
            drive_discs: Vec::new(),
        }
    }

    pub fn with_w_engine(mut self, we: WEngine) -> Self {
        self.w_engine = Some(we);
        self
    }

    pub fn with_drive_discs(mut self, discs: Vec<DriveDisc>) -> Self {
        self.drive_discs = discs;
        self
    }
}

impl Default for CharacterEquipment {
    fn default() -> Self {
        Self::new()
    }
}

/// 管理装备到增益的转换。
///
/// 持有每个角色的装备分配，并将其解析为通过 BuffManager 施加的增益。
/// 盘片套装定义从 EquipmentData 加载，以便评估套装效果阈值（2 件套、4 件套）。
pub struct EquipmentManager {
    assignments: HashMap<String, CharacterEquipment>,
    disc_sets: HashMap<String, DiscSet>,
}

impl EquipmentManager {
    /// 创建一个新的空 EquipmentManager。
    pub fn new() -> Self {
        EquipmentManager {
            assignments: HashMap::new(),
            disc_sets: HashMap::new(),
        }
    }

    /// 从完整的装备数据初始化，加载盘片套装定义。
    pub fn from_equipment_data(data: &EquipmentData) -> Self {
        let disc_sets = data
            .disc_sets
            .iter()
            .map(|ds| (ds.set_id.clone(), ds.clone()))
            .collect();
        EquipmentManager {
            assignments: HashMap::new(),
            disc_sets,
        }
    }

    /// 为角色分配装备（替换任何现有分配）。
    pub fn equip_character(&mut self, character_id: &str, equipment: CharacterEquipment) {
        self.assignments.insert(character_id.to_string(), equipment);
    }

    /// 获取角色装备分配的引用。
    pub fn get_equipment(&self, character_id: &str) -> Option<&CharacterEquipment> {
        self.assignments.get(character_id)
    }

    /// 移除角色的装备分配。
    pub fn unequip_character(&mut self, character_id: &str) {
        self.assignments.remove(character_id);
    }

    /// 将 buff_id 字符串解析为 BuffData 模板。
    ///
    /// 对于未知的 buff ID 返回 `None`（可在外部扩展）。
    pub fn resolve_buff(buff_id: &str) -> Option<BuffData> {
        match buff_id {
            // ── 盘片套装加成 ───────────────────────────────────────────
            "buff_electric_dmg_10" => Some(
                BuffData::new(
                    "buff_electric_dmg_10",
                    BuffCategory::DamageBonus,
                    StackType::Replace,
                )
                .with_duration(EQUIPMENT_BUFF_DURATION)
                .with_modifier("dmg_bonus", 0.10),
            ),
            "buff_thunder_atk_20" => Some(
                BuffData::new(
                    "buff_thunder_atk_20",
                    BuffCategory::Stat,
                    StackType::Replace,
                )
                .with_duration(900) // 15 秒 × 60 fps
                .with_modifier("atk_pct", 0.20),
            ),

            // ── 音擎被动效果 ───────────────────────────────────────────
            "passive_crit_dmg_20" => Some(
                BuffData::new(
                    "passive_crit_dmg_20",
                    BuffCategory::Crit,
                    StackType::Replace,
                )
                .with_duration(EQUIPMENT_BUFF_DURATION)
                .with_modifier("crit_dmg", 0.20),
            ),

            _ => None,
        }
    }

    /// 统计每个套装的驱动盘数量，并返回符合条件的加成增益 ID。
    fn resolve_set_bonuses(&self, drive_discs: &[DriveDisc]) -> Vec<String> {
        let mut set_counts: HashMap<String, usize> = HashMap::new();
        for disc in drive_discs {
            if !disc.set_id.is_empty() {
                *set_counts.entry(disc.set_id.clone()).or_default() += 1;
            }
        }

        let mut buffs = Vec::new();
        for (set_id, count) in &set_counts {
            if let Some(disc_set) = self.disc_sets.get(set_id) {
                if *count >= 2 {
                    if let Some(ref bonus) = disc_set.two_piece_bonus {
                        if !bonus.buff_id.is_empty() {
                            buffs.push(bonus.buff_id.clone());
                        }
                    }
                }
                if *count >= 4 {
                    if let Some(ref bonus) = disc_set.four_piece_bonus {
                        if !bonus.buff_id.is_empty() {
                            buffs.push(bonus.buff_id.clone());
                        }
                    }
                }
            }
        }
        buffs
    }

    /// 将装备 JSON 中的属性名称映射到 BuffManager 的修改器名称。
    ///
    /// 装备使用短名称如 `"atk"` 或 `"pen_fixed"`，而增益系统期望 `"atk_flat"` / `"pen"`。
    pub fn map_stat_name(name: &str) -> &str {
        match name {
            "hp" => "hp_flat",
            "atk" => "atk_flat",
            "def" => "def_flat",
            "pen_fixed" => "pen",
            other => other,
        }
    }

    /// 从音擎的基础属性构建一个包含多个修改器的单一增益。
    /// 跳过零值属性以保持增益的紧凑性。
    fn build_wengine_stats_buff(we: &WEngine) -> Option<BuffData> {
        let buff_id = format!("eq_we_{}_stats", we.id);
        let mut buff = BuffData::new(&buff_id, BuffCategory::Stat, StackType::Replace)
            .with_duration(EQUIPMENT_BUFF_DURATION);

        let stats = &we.base_stats;
        let mut count = 0;
        macro_rules! add_if_nonzero {
            ($field:ident, $name:expr) => {
                if stats.$field != 0.0 {
                    buff = buff.with_modifier($name, stats.$field);
                    count += 1;
                }
            };
        }

        add_if_nonzero!(atk, "atk_flat");
        add_if_nonzero!(hp, "hp_flat");
        add_if_nonzero!(def, "def_flat");
        add_if_nonzero!(crit_rate, "crit_rate");
        add_if_nonzero!(crit_dmg, "crit_dmg");
        add_if_nonzero!(pen, "pen");
        add_if_nonzero!(pen_ratio, "pen_ratio");
        add_if_nonzero!(anomaly_mastery, "anomaly_mastery");
        add_if_nonzero!(anomaly_proficiency, "anomaly_proficiency");
        add_if_nonzero!(impact, "impact");
        add_if_nonzero!(energy_regen, "energy_regen");
        add_if_nonzero!(dmg_bonus, "dmg_bonus");

        if count > 0 {
            Some(buff)
        } else {
            None
        }
    }

    /// 为角色施加所有源自装备的增益。
    ///
    /// * 音擎的基础属性注册为单个多修改器增益。
    /// * 音擎的被动效果按 ID 注册为增益。
    /// * 驱动盘的主属性和副属性注册为属性增益。
    /// * 盘片套装加成（2 件套、4 件套）被评估并注册。
    ///
    /// 未知或无法解析的增益 ID 会被静默跳过。
    pub fn apply_equipment_buffs(
        &self,
        character_id: &str,
        buff_manager: &mut BuffManager,
        current_tick: u64,
    ) {
        let Some(equipment) = self.assignments.get(character_id) else {
            return;
        };

        // ── 音擎 ───────────────────────────────────────────────────────
        if let Some(ref we) = equipment.w_engine {
            // 基础属性 → 包含所有非零属性修改器的单个增益
            if let Some(stats_buff) = Self::build_wengine_stats_buff(we) {
                buff_manager.apply_buff(character_id, stats_buff, current_tick);
            }

            // 被动效果
            for passive_id in &we.passive_effects {
                if let Some(buff) = Self::resolve_buff(passive_id) {
                    buff_manager.apply_buff(character_id, buff, current_tick);
                }
            }
        }

        // ── 驱动盘 ─────────────────────────────────────────────────────
        for disc in &equipment.drive_discs {
            // 主属性 → 单个修改器的增益
            let main_buff_id = format!("eq_dd_{}_main", disc.id);
            let mapped_main = Self::map_stat_name(&disc.main_stat.stat_name);
            let main_buff = BuffData::new(&main_buff_id, BuffCategory::Stat, StackType::Replace)
                .with_duration(EQUIPMENT_BUFF_DURATION)
                .with_modifier(mapped_main, disc.main_stat.value);
            buff_manager.apply_buff(character_id, main_buff, current_tick);

            // 副属性 → 每个副属性条目一个独立增益
            for sub in &disc.sub_stats {
                let mapped_sub = Self::map_stat_name(&sub.stat_name);
                let sub_buff_id = format!("eq_dd_{}_sub_{}", disc.id, sub.stat_name);
                let sub_buff =
                    BuffData::new(&sub_buff_id, BuffCategory::Stat, StackType::Independent)
                        .with_duration(EQUIPMENT_BUFF_DURATION)
                        .with_modifier(mapped_sub, sub.value);
                buff_manager.apply_buff(character_id, sub_buff, current_tick);
            }
        }

        // ── 盘片套装加成 ───────────────────────────────────────────────
        for buff_id in self.resolve_set_bonuses(&equipment.drive_discs) {
            if let Some(buff) = Self::resolve_buff(&buff_id) {
                buff_manager.apply_buff(character_id, buff, current_tick);
            }
        }
    }

    /// 返回已分配装备的角色数量。
    pub fn total_equipped(&self) -> usize {
        self.assignments.len()
    }
}

impl Default for EquipmentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::equipment::{DiscBonus, StatEntry};

    // ── 辅助函数 ─────────────────────────────────────────────────────────

    fn sample_w_engine() -> WEngine {
        WEngine {
            id: "we_sharp_storm".into(),
            name: "Sharp Storm".into(),
            level: 60,
            ascension: 6,
            base_stats: crate::entities::models::BaseStats::default().with_atk(680.0),
            passive_effects: vec!["passive_crit_dmg_20".into()],
        }
    }

    fn sample_disc(set_id: &str, slot: u8) -> DriveDisc {
        DriveDisc {
            id: format!("dd_{}_{}", set_id, slot),
            slot,
            level: 15,
            main_stat: StatEntry {
                stat_name: "hp".into(),
                value: 1000.0,
            },
            sub_stats: vec![],
            set_id: set_id.to_string(),
        }
    }

    fn thunder_metal_set() -> DiscSet {
        DiscSet {
            set_id: "set_thunder_metal".into(),
            name: "Thunder Metal".into(),
            two_piece_bonus: Some(DiscBonus {
                description: "+10% Electric DMG".into(),
                buff_id: "buff_electric_dmg_10".into(),
            }),
            four_piece_bonus: Some(DiscBonus {
                description: "ATK +20% on Electric DMG".into(),
                buff_id: "buff_thunder_atk_20".into(),
            }),
        }
    }

    fn setup_manager_with_sets() -> EquipmentManager {
        let data = EquipmentData {
            w_engines: vec![sample_w_engine()],
            drive_discs: vec![],
            disc_sets: vec![thunder_metal_set()],
        };
        EquipmentManager::from_equipment_data(&data)
    }

    // ── 基础分配 ─────────────────────────────────────────────────────────

    #[test]
    fn test_new_manager_is_empty() {
        let mgr = EquipmentManager::new();
        assert_eq!(mgr.total_equipped(), 0);
    }

    #[test]
    fn test_equip_and_get_character() {
        let mut mgr = EquipmentManager::new();
        let eq = CharacterEquipment::new().with_w_engine(sample_w_engine());
        mgr.equip_character("char_a", eq);

        assert_eq!(mgr.total_equipped(), 1);
        let fetched = mgr.get_equipment("char_a").expect("should be equipped");
        assert!(fetched.w_engine.is_some());
        assert_eq!(fetched.w_engine.as_ref().unwrap().id, "we_sharp_storm");
    }

    #[test]
    fn test_equip_replaces_previous() {
        let mut mgr = EquipmentManager::new();
        mgr.equip_character("char_a", CharacterEquipment::new());
        mgr.equip_character(
            "char_a",
            CharacterEquipment::new().with_w_engine(sample_w_engine()),
        );
        assert_eq!(mgr.total_equipped(), 1);
        assert!(mgr.get_equipment("char_a").unwrap().w_engine.is_some());
    }

    #[test]
    fn test_unequip_character() {
        let mut mgr = EquipmentManager::new();
        mgr.equip_character("char_a", CharacterEquipment::new());
        assert_eq!(mgr.total_equipped(), 1);
        mgr.unequip_character("char_a");
        assert_eq!(mgr.total_equipped(), 0);
        assert!(mgr.get_equipment("char_a").is_none());
    }

    #[test]
    fn test_get_unknown_character_returns_none() {
        let mgr = EquipmentManager::new();
        assert!(mgr.get_equipment("nobody").is_none());
    }

    // ── 盘片套装加成解析 ─────────────────────────────────────────────────

    #[test]
    fn test_no_discs_no_bonuses() {
        let mgr = setup_manager_with_sets();
        let buffs = mgr.resolve_set_bonuses(&[]);
        assert!(buffs.is_empty());
    }

    #[test]
    fn test_one_disc_does_not_trigger_bonus() {
        let mgr = setup_manager_with_sets();
        let discs = vec![sample_disc("set_thunder_metal", 1)];
        let buffs = mgr.resolve_set_bonuses(&discs);
        assert!(buffs.is_empty());
    }

    #[test]
    fn test_two_discs_triggers_two_piece() {
        let mgr = setup_manager_with_sets();
        let discs = vec![
            sample_disc("set_thunder_metal", 1),
            sample_disc("set_thunder_metal", 2),
        ];
        let buffs = mgr.resolve_set_bonuses(&discs);
        assert!(buffs.contains(&"buff_electric_dmg_10".to_string()));
        assert!(!buffs.contains(&"buff_thunder_atk_20".to_string()));
    }

    #[test]
    fn test_four_discs_triggers_both_bonuses() {
        let mgr = setup_manager_with_sets();
        let discs = (1..=4)
            .map(|slot| sample_disc("set_thunder_metal", slot))
            .collect::<Vec<_>>();
        let buffs = mgr.resolve_set_bonuses(&discs);
        assert_eq!(buffs.len(), 2);
        assert!(buffs.contains(&"buff_electric_dmg_10".to_string()));
        assert!(buffs.contains(&"buff_thunder_atk_20".to_string()));
    }

    #[test]
    fn test_six_discs_same_set_still_triggers_both() {
        let mgr = setup_manager_with_sets();
        let discs = (1..=6)
            .map(|slot| sample_disc("set_thunder_metal", slot))
            .collect::<Vec<_>>();
        let buffs = mgr.resolve_set_bonuses(&discs);
        assert_eq!(buffs.len(), 2);
    }

    #[test]
    fn test_unknown_set_id_produces_no_bonuses() {
        let mgr = setup_manager_with_sets();
        let discs = (1..=4)
            .map(|slot| sample_disc("set_unknown", slot))
            .collect::<Vec<_>>();
        let buffs = mgr.resolve_set_bonuses(&discs);
        assert!(buffs.is_empty());
    }

    #[test]
    fn test_discs_with_empty_set_id_ignored() {
        let mut mgr = setup_manager_with_sets();
        // 额外添加一个 set_id 为空的盘片
        mgr.disc_sets.insert(
            "set_no_bonus".into(),
            DiscSet {
                set_id: "set_no_bonus".into(),
                name: "No Bonus".into(),
                two_piece_bonus: None,
                four_piece_bonus: None,
            },
        );
        let discs = (1..=4)
            .map(|slot| sample_disc("set_no_bonus", slot))
            .collect::<Vec<_>>();
        let buffs = mgr.resolve_set_bonuses(&discs);
        assert!(buffs.is_empty());
    }

    #[test]
    fn test_disc_set_with_empty_buff_id() {
        let mut mgr = setup_manager_with_sets();
        mgr.disc_sets.insert(
            "set_empty_buff".into(),
            DiscSet {
                set_id: "set_empty_buff".into(),
                name: "Empty Buff".into(),
                two_piece_bonus: Some(DiscBonus {
                    description: "empty".into(),
                    buff_id: "".into(),
                }),
                four_piece_bonus: None,
            },
        );
        let discs = (1..=2)
            .map(|slot| sample_disc("set_empty_buff", slot))
            .collect::<Vec<_>>();
        let buffs = mgr.resolve_set_bonuses(&discs);
        assert!(buffs.is_empty());
    }

    // ── 应用装备增益（与 BuffManager 集成）───────────────────────────────

    #[test]
    fn test_apply_no_equipment_does_nothing() {
        let mgr = setup_manager_with_sets();
        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);
        assert_eq!(bm.total_active_buffs(), 0);
    }

    #[test]
    fn test_apply_w_engine_buffs() {
        let mut mgr = setup_manager_with_sets();
        mgr.equip_character(
            "char_a",
            CharacterEquipment::new().with_w_engine(sample_w_engine()),
        );

        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);

        let snap = bm.get_effective_modifiers("char_a");
        // 音擎基础属性 → atk_flat = 680.0
        assert_eq!(snap.atk_flat, 680.0);
        // 音擎被动 → crit_dmg +0.20
        assert_eq!(snap.crit_dmg, 0.20);
        // 2 个增益：音擎属性 + 音擎被动
        assert_eq!(bm.total_active_buffs(), 2);
    }

    #[test]
    fn test_apply_disc_set_bonuses() {
        let mut mgr = setup_manager_with_sets();
        let discs = (1..=4)
            .map(|slot| sample_disc("set_thunder_metal", slot))
            .collect();
        mgr.equip_character("char_a", CharacterEquipment::new().with_drive_discs(discs));

        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);

        let snap = bm.get_effective_modifiers("char_a");
        // 4 个盘片 × 每个主属性 hp=1000
        assert_eq!(snap.hp_flat, 4000.0);
        // 2 件套套装加成
        assert_eq!(snap.dmg_bonus, 0.10);
        // 4 件套套装加成
        assert_eq!(snap.atk_pct, 0.20);
        // 4 个主属性增益 + 2 个套装加成增益
        assert_eq!(bm.total_active_buffs(), 6);
    }

    #[test]
    fn test_apply_w_engine_and_disc_buffs_together() {
        let mut mgr = setup_manager_with_sets();
        let discs = (1..=4)
            .map(|slot| sample_disc("set_thunder_metal", slot))
            .collect();
        mgr.equip_character(
            "char_a",
            CharacterEquipment::new()
                .with_w_engine(sample_w_engine())
                .with_drive_discs(discs),
        );

        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);

        let snap = bm.get_effective_modifiers("char_a");
        // 音擎基础属性 → atk_flat = 680.0
        assert_eq!(snap.atk_flat, 680.0);
        // 音擎被动 → crit_dmg +0.20
        assert_eq!(snap.crit_dmg, 0.20);
        // 4 个盘片 × 主属性 hp=1000
        assert_eq!(snap.hp_flat, 4000.0);
        // 2 件套套装加成
        assert_eq!(snap.dmg_bonus, 0.10);
        // 4 件套套装加成
        assert_eq!(snap.atk_pct, 0.20);
        // 1 音擎属性 + 1 音擎被动 + 4 主属性 + 2 套装加成 = 8
        assert_eq!(bm.total_active_buffs(), 8);
    }

    #[test]
    fn test_apply_incomplete_set_does_not_trigger_four_piece() {
        let mut mgr = setup_manager_with_sets();
        let discs = (1..=2)
            .map(|slot| sample_disc("set_thunder_metal", slot))
            .collect();
        mgr.equip_character("char_a", CharacterEquipment::new().with_drive_discs(discs));

        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);

        let snap = bm.get_effective_modifiers("char_a");
        // 2 个盘片 × 每个主属性 hp=1000
        assert_eq!(snap.hp_flat, 2000.0);
        // 仅 2 件套
        assert_eq!(snap.dmg_bonus, 0.10);
        // 4 件套未生效
        assert_eq!(snap.atk_pct, 0.0);
        // 2 个主属性增益 + 1 个套装加成增益 = 3
        assert_eq!(bm.total_active_buffs(), 3);
    }

    #[test]
    fn test_unknown_buff_id_skipped_gracefully() {
        let mut mgr = EquipmentManager::new();
        mgr.disc_sets.insert(
            "set_unknown_buff".into(),
            DiscSet {
                set_id: "set_unknown_buff".into(),
                name: "Unknown".into(),
                two_piece_bonus: Some(DiscBonus {
                    description: "?".into(),
                    buff_id: "nonexistent_buff_id".into(),
                }),
                four_piece_bonus: None,
            },
        );
        let discs = (1..=2)
            .map(|slot| sample_disc("set_unknown_buff", slot))
            .collect();
        mgr.equip_character("char_a", CharacterEquipment::new().with_drive_discs(discs));

        let mut bm = BuffManager::new();
        // 不应 panic — 未知的增益 ID 会被静默跳过。
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);
        // 2 个盘片 × 主属性增益已应用，未知套装增益被跳过
        assert_eq!(bm.total_active_buffs(), 2);
    }

    // ── 多角色 ───────────────────────────────────────────────────────────

    #[test]
    fn test_multiple_characters_independent_buffs() {
        let mut mgr = setup_manager_with_sets();

        // 角色 A：仅音擎
        mgr.equip_character(
            "char_a",
            CharacterEquipment::new().with_w_engine(sample_w_engine()),
        );

        // 角色 B：4 件套盘片
        let discs = (1..=4)
            .map(|slot| sample_disc("set_thunder_metal", slot))
            .collect();
        mgr.equip_character("char_b", CharacterEquipment::new().with_drive_discs(discs));

        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);
        mgr.apply_equipment_buffs("char_b", &mut bm, 0);

        let snap_a = bm.get_effective_modifiers("char_a");
        assert_eq!(snap_a.atk_flat, 680.0); // W-Engine base stats
        assert_eq!(snap_a.crit_dmg, 0.20); // W-Engine passive
        assert_eq!(snap_a.dmg_bonus, 0.0); // no disc set

        let snap_b = bm.get_effective_modifiers("char_b");
        assert_eq!(snap_b.crit_dmg, 0.0); // no W-Engine
        assert_eq!(snap_b.hp_flat, 4000.0); // 4 discs × hp=1000
        assert_eq!(snap_b.dmg_bonus, 0.10); // 2-pc set
        assert_eq!(snap_b.atk_pct, 0.20); // 4-pc set
    }

    // ── 解析增益 ─────────────────────────────────────────────────────────

    #[test]
    fn test_resolve_known_buffs() {
        assert!(EquipmentManager::resolve_buff("buff_electric_dmg_10").is_some());
        assert!(EquipmentManager::resolve_buff("buff_thunder_atk_20").is_some());
        assert!(EquipmentManager::resolve_buff("passive_crit_dmg_20").is_some());
    }

    #[test]
    fn test_resolve_unknown_buff_returns_none() {
        assert!(EquipmentManager::resolve_buff("nonexistent").is_none());
        assert!(EquipmentManager::resolve_buff("").is_none());
    }

    // ── 从装备数据初始化 ─────────────────────────────────────────────────

    #[test]
    fn test_from_equipment_data_loads_disc_sets() {
        let data = EquipmentData {
            w_engines: vec![],
            drive_discs: vec![],
            disc_sets: vec![thunder_metal_set()],
        };
        let mgr = EquipmentManager::from_equipment_data(&data);
        assert!(mgr.disc_sets.contains_key("set_thunder_metal"));
    }

    #[test]
    fn test_from_equipment_data_empty() {
        let data = EquipmentData::default();
        let mgr = EquipmentManager::from_equipment_data(&data);
        assert!(mgr.disc_sets.is_empty());
    }

    // ── CharacterEquipment 构建器 ────────────────────────────────────────

    #[test]
    fn test_character_equipment_default() {
        let eq = CharacterEquipment::default();
        assert!(eq.w_engine.is_none());
        assert!(eq.drive_discs.is_empty());
    }

    #[test]
    fn test_character_equipment_with_w_engine() {
        let eq = CharacterEquipment::new().with_w_engine(sample_w_engine());
        assert!(eq.w_engine.is_some());
        assert_eq!(eq.w_engine.unwrap().id, "we_sharp_storm");
    }

    // ── 属性名称映射 ───────────────────────────────────────────────────────

    #[test]
    fn test_map_stat_name_mappings() {
        assert_eq!(EquipmentManager::map_stat_name("hp"), "hp_flat");
        assert_eq!(EquipmentManager::map_stat_name("atk"), "atk_flat");
        assert_eq!(EquipmentManager::map_stat_name("def"), "def_flat");
        assert_eq!(EquipmentManager::map_stat_name("pen_fixed"), "pen");
    }

    #[test]
    fn test_map_stat_name_passthrough() {
        // 不需要映射的名称原样返回。
        assert_eq!(EquipmentManager::map_stat_name("crit_rate"), "crit_rate");
        assert_eq!(EquipmentManager::map_stat_name("crit_dmg"), "crit_dmg");
        assert_eq!(
            EquipmentManager::map_stat_name("anomaly_mastery"),
            "anomaly_mastery"
        );
        assert_eq!(
            EquipmentManager::map_stat_name("energy_regen"),
            "energy_regen"
        );
        assert_eq!(EquipmentManager::map_stat_name(""), "");
    }

    // ── 构建音擎属性增益 ──────────────────────────────────────────────────

    #[test]
    fn test_build_wengine_stats_buff_all_zeros() {
        let we = WEngine {
            id: "we_empty".into(),
            name: "Empty".into(),
            level: 1,
            ascension: 0,
            base_stats: crate::entities::models::BaseStats::default(),
            passive_effects: vec![],
        };
        let result = EquipmentManager::build_wengine_stats_buff(&we);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_wengine_stats_buff_with_stats() {
        let we = WEngine {
            id: "we_test".into(),
            name: "Test".into(),
            level: 60,
            ascension: 6,
            base_stats: crate::entities::models::BaseStats::default()
                .with_atk(500.0)
                .with_crit_rate(0.10)
                .with_crit_dmg(0.50),
            passive_effects: vec![],
        };
        let buff =
            EquipmentManager::build_wengine_stats_buff(&we).expect("should build buff with stats");
        assert_eq!(buff.buff_id, "eq_we_we_test_stats");
        assert_eq!(buff.modifiers.len(), 3);
        let mod_names: Vec<&str> = buff
            .modifiers
            .iter()
            .map(|m| m.stat_name.as_str())
            .collect();
        assert!(mod_names.contains(&"atk_flat"));
        assert!(mod_names.contains(&"crit_rate"));
        assert!(mod_names.contains(&"crit_dmg"));
    }

    // ── 与真实装备数据文件的集成测试 ────────────────────────────────────

    #[test]
    fn test_load_real_equipment_and_apply() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/equipment/equipment.json"
        ));
        let contents = std::fs::read_to_string(path).expect("read equipment file");
        let data: EquipmentData = serde_json::from_str(&contents).expect("parse equipment JSON");
        let mut mgr = EquipmentManager::from_equipment_data(&data);

        // 使用真实 Sharp Storm 音擎（atk=680, crit_rate=0.24）。
        let we = data
            .w_engines
            .iter()
            .find(|w| w.id == "we_sharp_storm")
            .cloned()
            .expect("we_sharp_storm in data");

        // 使用全部 6 个雷霆金属盘片。
        let discs = data
            .drive_discs
            .iter()
            .filter(|d| d.set_id == "set_thunder_metal")
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(discs.len(), 6);

        mgr.equip_character(
            "anby",
            CharacterEquipment::new()
                .with_w_engine(we)
                .with_drive_discs(discs),
        );

        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("anby", &mut bm, 0);

        let snap = bm.get_effective_modifiers("anby");
        // 真实 Sharp Storm 音擎：基础 atk=680，加上盘片主/副属性的攻击力贡献
        assert_eq!(snap.atk_flat, 1396.0);
        // 音擎被动 + 盘片副属性
        assert_eq!(snap.crit_dmg, 0.57);
        // 6 个 Thunder Metal 盘片 → 2 件套（+10% 电属性伤害）+ 4 件套（攻击力 +20%）
        assert_eq!(snap.dmg_bonus, 0.10);
        assert_eq!(snap.atk_pct, 0.20);
    }
}
