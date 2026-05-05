use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::enums::{CharacterState, ElementTag, FactionTag, SpecialtyTag};
use super::models::BaseStats;

/// 每个角色的资源池（能量、分贝等）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceSet {
    #[serde(default)]
    pub energy: f64,
    #[serde(default)]
    pub decibel: f64,
    #[serde(default)]
    pub chain_points: u32,
}

/// 一个可玩角色，包含身份、属性、资源和状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub char_id: String,
    #[serde(default)]
    pub name: String,
    pub faction: FactionTag,
    pub specialty: SpecialtyTag,
    pub element: ElementTag,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub ascension: u32,
    pub base_stats: BaseStats,
    #[serde(default)]
    pub current_stats: BaseStats,
    #[serde(default)]
    pub resources: ResourceSet,
    #[serde(default)]
    pub state: CharacterState,
    #[serde(default)]
    pub action_dict: HashSet<String>,
    #[serde(default = "default_constellations")]
    pub constellations: [bool; 6],
    #[serde(default = "default_constellations")]
    pub potentials: [bool; 6],
    #[serde(default)]
    pub realtime_modifiers: HashMap<String, f64>,
}

fn default_constellations() -> [bool; 6] {
    [false; 6]
}

impl Character {
    pub fn new(
        char_id: impl Into<String>,
        faction: FactionTag,
        specialty: SpecialtyTag,
        element: ElementTag,
        base_stats: BaseStats,
    ) -> Self {
        Self {
            char_id: char_id.into(),
            name: String::new(),
            faction,
            specialty,
            element,
            level: 1,
            ascension: 0,
            current_stats: base_stats.clone(),
            base_stats,
            resources: ResourceSet::default(),
            state: CharacterState::Standby,
            action_dict: HashSet::new(),
            constellations: [false; 6],
            potentials: [false; 6],
            realtime_modifiers: HashMap::new(),
        }
    }

    pub fn change_state(&mut self, new_state: CharacterState) {
        self.state = new_state;
    }

    pub fn validate_action(&self, action_id: &str) -> bool {
        self.action_dict.contains(action_id)
    }

    pub fn get_realtime_stat(&self, stat_name: &str) -> f64 {
        let base_val = match stat_name {
            "atk" => self.base_stats.atk,
            "def" => self.base_stats.def,
            "hp" => self.base_stats.hp,
            "crit_rate" => self.base_stats.crit_rate,
            "crit_dmg" => self.base_stats.crit_dmg,
            "pen" => self.base_stats.pen,
            "pen_ratio" => self.base_stats.pen_ratio,
            "anomaly_mastery" => self.base_stats.anomaly_mastery,
            "anomaly_proficiency" => self.base_stats.anomaly_proficiency,
            "impact" => self.base_stats.impact,
            "energy_regen" => self.base_stats.energy_regen,
            "dmg_bonus" => self.base_stats.dmg_bonus,
            _ => 0.0,
        };
        let modifier = self
            .realtime_modifiers
            .get(stat_name)
            .copied()
            .unwrap_or(0.0);
        base_val + modifier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_character() -> Character {
        let stats = BaseStats::default()
            .with_atk(1000.0)
            .with_hp(8000.0)
            .with_crit_rate(0.3)
            .with_crit_dmg(1.5);
        let mut c = Character::new(
            "agent_01",
            FactionTag::GentleHouse,
            SpecialtyTag::Attack,
            ElementTag::Physical,
            stats,
        );
        c.action_dict.insert("Attack_Normal_1".to_string());
        c
    }

    #[test]
    fn test_character_creation() {
        let c = sample_character();
        assert_eq!(c.char_id, "agent_01");
        assert_eq!(c.faction, FactionTag::GentleHouse);
        assert_eq!(c.state, CharacterState::Standby);
        assert_eq!(c.level, 1);
        assert_eq!(c.base_stats.atk, 1000.0);
        assert_eq!(c.current_stats.atk, 1000.0);
    }

    #[test]
    fn test_validate_action() {
        let c = sample_character();
        assert!(c.validate_action("Attack_Normal_1"));
        assert!(!c.validate_action("Attack_Normal_2"));
    }

    #[test]
    fn test_change_state() {
        let mut c = sample_character();
        c.change_state(CharacterState::Active);
        assert_eq!(c.state, CharacterState::Active);
    }

    #[test]
    fn test_get_realtime_stat_with_modifier() {
        let mut c = sample_character();
        c.realtime_modifiers.insert("atk".to_string(), 200.0);
        assert_eq!(c.get_realtime_stat("atk"), 1200.0);
        assert_eq!(c.get_realtime_stat("hp"), 8000.0);
    }

    #[test]
    fn test_serde_roundtrip() {
        let c = sample_character();
        let json = serde_json::to_string(&c).expect("serialize");
        let back: Character = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(c.char_id, back.char_id);
        assert_eq!(c.faction, back.faction);
        assert_eq!(c.base_stats.atk, back.base_stats.atk);
    }

    #[test]
    fn test_deserialize_from_data_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/characters/anby_demara.json"
        );
        let json = std::fs::read_to_string(path).expect("read character JSON file");
        let c: Character = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(c.char_id, "anby_demara");
        assert_eq!(c.name, "Anby Demara");
        assert_eq!(c.faction, FactionTag::GentleHouse);
        assert_eq!(c.specialty, SpecialtyTag::Stun);
        assert_eq!(c.element, ElementTag::Electric);
        assert_eq!(c.level, 60);
        assert_eq!(c.ascension, 6);
        assert_eq!(c.base_stats.hp, 9500.0);
        assert_eq!(c.base_stats.atk, 1100.0);
        assert_eq!(c.base_stats.def, 550.0);
        assert_eq!(c.base_stats.impact, 120.0);
        assert_eq!(c.base_stats.crit_rate, 0.15);
        assert_eq!(c.base_stats.crit_dmg, 0.8);
        assert_eq!(c.base_stats.pen_ratio, 0.1);
        assert_eq!(c.base_stats.pen, 40.0);
        assert_eq!(c.base_stats.anomaly_mastery, 80.0);
        assert_eq!(c.base_stats.anomaly_proficiency, 95.0);
        assert_eq!(c.base_stats.energy_regen, 1.2);
        assert_eq!(c.base_stats.dmg_bonus, 0.3);
        assert!(c.constellations[0]);
        assert_eq!(c.action_dict.len(), 6);
        assert!(c.validate_action("Attack_Normal_1"));
        assert!(c.validate_action("Ultimate_1"));
        assert!(c.validate_action("Dodge_1"));
    }

    #[test]
    fn test_deserialize_minimal_json() {
        // Minimal character JSON as might come from data/characters/
        let json = r#"{
            "char_id": "anby_demara",
            "name": "Anby Demara",
            "faction": "Gentle_House",
            "specialty": "Stun",
            "element": "Electric",
            "level": 60,
            "ascension": 6,
            "base_stats": {
                "hp": 9500.0,
                "atk": 1100.0,
                "def": 550.0,
                "impact": 120.0,
                "crit_rate": 0.15,
                "crit_dmg": 0.8,
                "pen_ratio": 0.1,
                "pen_fixed": 40.0,
                "anomaly_mastery": 80.0,
                "anomaly_proficiency": 95.0,
                "energy_regen": 1.2,
                "energy_gen_rate": 0.3
            },
            "action_dict": ["Attack_Normal_1", "Attack_Normal_2", "Skill_Ex_1"],
            "constellations": [true, false, false, false, false, false]
        }"#;
        let c: Character = serde_json::from_str(json).expect("deserialize");
        assert_eq!(c.char_id, "anby_demara");
        assert_eq!(c.name, "Anby Demara");
        assert_eq!(c.specialty, SpecialtyTag::Stun);
        assert_eq!(c.element, ElementTag::Electric);
        assert_eq!(c.level, 60);
        assert_eq!(c.ascension, 6);
        assert_eq!(c.base_stats.hp, 9500.0);
        assert_eq!(c.base_stats.def, 550.0);
        assert!(c.constellations[0]);
        assert!(!c.constellations[1]);
        assert_eq!(c.action_dict.len(), 3);
        assert!(c.validate_action("Attack_Normal_1"));
        assert!(c.validate_action("Skill_Ex_1"));
    }
}
