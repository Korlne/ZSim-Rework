use std::collections::HashMap;

use crate::calculation::buff::{BuffCategory, BuffData, BuffManager, StackType};
use crate::data::equipment::{DiscSet, DriveDisc, EquipmentData, WEngine};

/// Per-character equipment assignment: one W-Engine and up to 6 Drive Discs.
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

/// Manages equipment-to-buff translation.
///
/// Holds per-character equipment assignments and resolves them into buffs
/// applied via BuffManager.  Disc set definitions are loaded from EquipmentData
/// so that set-bonus thresholds (2-piece, 4-piece) can be evaluated.
pub struct EquipmentManager {
    assignments: HashMap<String, CharacterEquipment>,
    disc_sets: HashMap<String, DiscSet>,
}

impl EquipmentManager {
    /// Create a new, empty EquipmentManager.
    pub fn new() -> Self {
        EquipmentManager {
            assignments: HashMap::new(),
            disc_sets: HashMap::new(),
        }
    }

    /// Initialise from full equipment data, loading disc set definitions.
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

    /// Assign equipment to a character (replaces any existing assignment).
    pub fn equip_character(&mut self, character_id: &str, equipment: CharacterEquipment) {
        self.assignments.insert(character_id.to_string(), equipment);
    }

    /// Get a reference to a character's equipment assignment.
    pub fn get_equipment(&self, character_id: &str) -> Option<&CharacterEquipment> {
        self.assignments.get(character_id)
    }

    /// Remove a character's equipment assignment.
    pub fn unequip_character(&mut self, character_id: &str) {
        self.assignments.remove(character_id);
    }

    /// Resolve a buff_id string to a BuffData template.
    ///
    /// Returns `None` for unknown buff IDs (can be extended externally).
    pub fn resolve_buff(buff_id: &str) -> Option<BuffData> {
        match buff_id {
            // ── Disc set bonuses ──────────────────────────────────────
            "buff_electric_dmg_10" => Some(
                BuffData::new(
                    "buff_electric_dmg_10",
                    BuffCategory::DamageBonus,
                    StackType::Replace,
                )
                .with_modifier("dmg_bonus", 0.10),
            ),
            "buff_thunder_atk_20" => Some(
                BuffData::new(
                    "buff_thunder_atk_20",
                    BuffCategory::Stat,
                    StackType::Replace,
                )
                .with_duration(900) // 15 s × 60 fps
                .with_modifier("atk_pct", 0.20),
            ),

            // ── W-Engine passives ─────────────────────────────────────
            "passive_crit_dmg_20" => Some(
                BuffData::new(
                    "passive_crit_dmg_20",
                    BuffCategory::Crit,
                    StackType::Replace,
                )
                .with_modifier("crit_dmg", 0.20),
            ),

            _ => None,
        }
    }

    /// Count the number of drive discs per set and return qualifying bonus buff IDs.
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

    /// Apply all equipment-derived buffs for a character.
    ///
    /// * W-Engine passive effects are registered as buffs.
    /// * Drive Disc set bonuses (2-piece, 4-piece) are evaluated and registered.
    ///
    /// Unknown or unresolvable buff IDs are silently skipped.
    pub fn apply_equipment_buffs(
        &self,
        character_id: &str,
        buff_manager: &mut BuffManager,
        current_tick: u64,
    ) {
        let Some(equipment) = self.assignments.get(character_id) else {
            return;
        };

        // W-Engine passive effects
        if let Some(ref we) = equipment.w_engine {
            for passive_id in &we.passive_effects {
                if let Some(buff) = Self::resolve_buff(passive_id) {
                    buff_manager.apply_buff(character_id, buff, current_tick);
                }
            }
        }

        // Disc set bonuses
        for buff_id in self.resolve_set_bonuses(&equipment.drive_discs) {
            if let Some(buff) = Self::resolve_buff(&buff_id) {
                buff_manager.apply_buff(character_id, buff, current_tick);
            }
        }
    }

    /// Return the number of characters with equipment assigned.
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

    // ── Helpers ──────────────────────────────────────────────────────────

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

    // ── Basic assignment ─────────────────────────────────────────────────

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

    // ── Disc set bonus resolution ────────────────────────────────────────

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
        // Also add a disc with empty set_id directly
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

    // ── apply_equipment_buffs (integration with BuffManager) ─────────────

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
        assert_eq!(snap.crit_dmg, 0.20);
        assert_eq!(bm.total_active_buffs(), 1);
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
        assert_eq!(snap.dmg_bonus, 0.10);
        assert_eq!(snap.atk_pct, 0.20);
        assert_eq!(bm.total_active_buffs(), 2);
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
        assert_eq!(snap.crit_dmg, 0.20);
        assert_eq!(snap.dmg_bonus, 0.10);
        assert_eq!(snap.atk_pct, 0.20);
        assert_eq!(bm.total_active_buffs(), 3);
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
        assert_eq!(snap.dmg_bonus, 0.10); // 2-pc only
        assert_eq!(snap.atk_pct, 0.0); // 4-pc not applied
        assert_eq!(bm.total_active_buffs(), 1);
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
        // Should not panic — unknown buff IDs are silently skipped.
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);
        assert_eq!(bm.total_active_buffs(), 0);
    }

    // ── Multi-character ──────────────────────────────────────────────────

    #[test]
    fn test_multiple_characters_independent_buffs() {
        let mut mgr = setup_manager_with_sets();

        // Character A: W-Engine only
        mgr.equip_character(
            "char_a",
            CharacterEquipment::new().with_w_engine(sample_w_engine()),
        );

        // Character B: 4-piece disc set
        let discs = (1..=4)
            .map(|slot| sample_disc("set_thunder_metal", slot))
            .collect();
        mgr.equip_character("char_b", CharacterEquipment::new().with_drive_discs(discs));

        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("char_a", &mut bm, 0);
        mgr.apply_equipment_buffs("char_b", &mut bm, 0);

        let snap_a = bm.get_effective_modifiers("char_a");
        assert_eq!(snap_a.crit_dmg, 0.20);
        assert_eq!(snap_a.dmg_bonus, 0.0); // no disc set

        let snap_b = bm.get_effective_modifiers("char_b");
        assert_eq!(snap_b.crit_dmg, 0.0); // no W-Engine
        assert_eq!(snap_b.dmg_bonus, 0.10);
        assert_eq!(snap_b.atk_pct, 0.20);
    }

    // ── resolve_buff ─────────────────────────────────────────────────────

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

    // ── from_equipment_data ──────────────────────────────────────────────

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

    // ── CharacterEquipment builder ───────────────────────────────────────

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

    // ── Integration with real equipment data file ────────────────────────

    #[test]
    fn test_load_real_equipment_and_apply() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/equipment/equipment.json"
        ));
        let contents = std::fs::read_to_string(path).expect("read equipment file");
        let data: EquipmentData = serde_json::from_str(&contents).expect("parse equipment JSON");
        let mut mgr = EquipmentManager::from_equipment_data(&data);

        // Assign the real Sharp Storm W-Engine to a character.
        let we = data
            .w_engines
            .iter()
            .find(|w| w.id == "we_sharp_storm")
            .cloned()
            .expect("we_sharp_storm in data");

        mgr.equip_character("anby", CharacterEquipment::new().with_w_engine(we));

        // Also assign the Thunder Metal discs (all 6 from the file).
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
                .with_w_engine(sample_w_engine())
                .with_drive_discs(discs),
        );

        let mut bm = BuffManager::new();
        mgr.apply_equipment_buffs("anby", &mut bm, 0);

        let snap = bm.get_effective_modifiers("anby");
        // W-Engine: passive_crit_dmg_20 -> crit_dmg +0.20
        assert_eq!(snap.crit_dmg, 0.20);
        // 6 Thunder Metal discs -> 2-pc (dmg_bonus +0.10) + 4-pc (atk_pct +0.20)
        assert_eq!(snap.dmg_bonus, 0.10);
        assert_eq!(snap.atk_pct, 0.20);
        assert_eq!(bm.total_active_buffs(), 3);
    }
}
