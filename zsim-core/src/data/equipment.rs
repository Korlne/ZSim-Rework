use serde::{Deserialize, Serialize};

use crate::entities::models::BaseStats;

/// A stat value entry (e.g., main stat or sub-stat on a drive disc).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatEntry {
    pub stat_name: String,
    pub value: f64,
}

/// W-Engine (weapon) data loaded from data/equipment/.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WEngine {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub ascension: u32,
    #[serde(default)]
    pub base_stats: BaseStats,
    /// Passive effect IDs applied when the w-engine is equipped.
    #[serde(default)]
    pub passive_effects: Vec<String>,
}

/// A single drive disc with main and sub stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveDisc {
    pub id: String,
    /// Disc slot position (1-6).
    pub slot: u8,
    #[serde(default)]
    pub level: u32,
    pub main_stat: StatEntry,
    #[serde(default)]
    pub sub_stats: Vec<StatEntry>,
    #[serde(default)]
    pub set_id: String,
}

/// A disc set bonus definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscSet {
    pub set_id: String,
    #[serde(default)]
    pub name: String,
    pub two_piece_bonus: Option<DiscBonus>,
    pub four_piece_bonus: Option<DiscBonus>,
}

/// A disc set bonus effect description (resolved into BuffData at runtime).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscBonus {
    /// Description label for the bonus effect.
    #[serde(default)]
    pub description: String,
    /// Buff identifier applied when the set threshold is met.
    #[serde(default)]
    pub buff_id: String,
}

/// Full equipment data collection loaded from data/equipment/.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EquipmentData {
    #[serde(default)]
    pub w_engines: Vec<WEngine>,
    #[serde(default)]
    pub drive_discs: Vec<DriveDisc>,
    #[serde(default)]
    pub disc_sets: Vec<DiscSet>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_w_engine() {
        let json = r#"{
            "id": "we_sharp_01",
            "name": "Sharp Storm",
            "level": 60,
            "ascension": 6,
            "base_stats": {
                "atk": 680.0,
                "crit_rate": 0.24,
                "crit_dmg": 0.5
            },
            "passive_effects": ["passive_crit_dmg_up"]
        }"#;
        let we: WEngine = serde_json::from_str(json).expect("deserialize");
        assert_eq!(we.id, "we_sharp_01");
        assert_eq!(we.name, "Sharp Storm");
        assert_eq!(we.level, 60);
        assert_eq!(we.ascension, 6);
        assert_eq!(we.base_stats.atk, 680.0);
        assert_eq!(we.base_stats.crit_rate, 0.24);
        assert_eq!(we.passive_effects.len(), 1);
        assert_eq!(we.passive_effects[0], "passive_crit_dmg_up");
    }

    #[test]
    fn test_deserialize_drive_disc() {
        let json = r#"{
            "id": "dd_01_slot4",
            "slot": 4,
            "level": 15,
            "main_stat": {"stat_name": "crit_rate", "value": 0.24},
            "sub_stats": [
                {"stat_name": "atk", "value": 120.0},
                {"stat_name": "crit_dmg", "value": 0.15}
            ],
            "set_id": "set_thunder_01"
        }"#;
        let disc: DriveDisc = serde_json::from_str(json).expect("deserialize");
        assert_eq!(disc.id, "dd_01_slot4");
        assert_eq!(disc.slot, 4);
        assert_eq!(disc.level, 15);
        assert_eq!(disc.main_stat.stat_name, "crit_rate");
        assert_eq!(disc.main_stat.value, 0.24);
        assert_eq!(disc.sub_stats.len(), 2);
        assert_eq!(disc.sub_stats[1].stat_name, "crit_dmg");
        assert_eq!(disc.set_id, "set_thunder_01");
    }

    #[test]
    fn test_deserialize_disc_set() {
        let json = r#"{
            "set_id": "set_thunder_01",
            "name": "Thunder Metal",
            "two_piece_bonus": {
                "description": "+10% Electric DMG",
                "buff_id": "buff_electric_dmg_10"
            },
            "four_piece_bonus": null
        }"#;
        let set: DiscSet = serde_json::from_str(json).expect("deserialize");
        assert_eq!(set.set_id, "set_thunder_01");
        assert_eq!(set.name, "Thunder Metal");
        assert!(set.two_piece_bonus.is_some());
        assert!(set.four_piece_bonus.is_none());
    }

    #[test]
    fn test_deserialize_equipment_data_full() {
        let json = r#"{
            "w_engines": [],
            "drive_discs": [],
            "disc_sets": []
        }"#;
        let data: EquipmentData = serde_json::from_str(json).expect("deserialize");
        assert!(data.w_engines.is_empty());
        assert!(data.drive_discs.is_empty());
        assert!(data.disc_sets.is_empty());
    }

    #[test]
    fn test_serde_roundtrip() {
        let data = EquipmentData {
            w_engines: vec![WEngine {
                id: "we_01".into(),
                name: "Test Engine".into(),
                level: 60,
                ascension: 6,
                base_stats: BaseStats::default().with_atk(500.0),
                passive_effects: vec!["p1".into()],
            }],
            drive_discs: vec![],
            disc_sets: vec![],
        };
        let json = serde_json::to_string(&data).expect("serialize");
        let back: EquipmentData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.w_engines.len(), 1);
        assert_eq!(back.w_engines[0].base_stats.atk, 500.0);
    }
}
