use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::enums::{ElementTag, EnemyType};

/// 敌人战斗状态，包含 HP、异常条、异常积蓄和抗性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnemyState {
    pub enemy_id: String,
    #[serde(default)]
    pub enemy_type: EnemyType,
    #[serde(default)]
    pub hp: f64,
    #[serde(alias = "def", default)]
    pub def_val: f64,
    #[serde(default)]
    pub base_res: f64,
    /// 当前的异常条累积量。
    #[serde(alias = "daze_current", default)]
    pub stun_gauge: f64,
    /// 触发异常前的最大异常条。
    #[serde(alias = "daze_max", default = "default_daze_max")]
    pub stun_max: f64,
    /// 敌人当前是否处于异常状态。
    #[serde(default)]
    pub is_stunned: bool,
    /// 每个元素的异常积蓄进度条。
    #[serde(default)]
    pub anomaly_buildup: HashMap<ElementTag, f64>,
    /// 每个元素的伤害抗性倍率（0.0 = 免疫，1.0 = 全额伤害）。
    #[serde(default)]
    pub resistances: HashMap<ElementTag, f64>,
    /// 该敌人弱点的元素。
    #[serde(default)]
    pub weaknesses: HashSet<ElementTag>,
}

fn default_daze_max() -> f64 {
    100.0
}

impl EnemyState {
    pub fn new(enemy_id: impl Into<String>, enemy_type: EnemyType, daze_max: f64) -> Self {
        Self {
            enemy_id: enemy_id.into(),
            enemy_type,
            hp: 0.0,
            def_val: 0.0,
            base_res: 0.0,
            stun_gauge: 0.0,
            stun_max: daze_max,
            is_stunned: false,
            anomaly_buildup: HashMap::new(),
            resistances: HashMap::new(),
            weaknesses: HashSet::new(),
        }
    }

    pub fn chain_attack_limit(&self) -> u32 {
        self.enemy_type.chain_attack_limit()
    }

    pub fn add_daze(&mut self, value: f64) {
        if !self.is_stunned {
            self.stun_gauge += value;
            if self.stun_gauge >= self.stun_max {
                self.is_stunned = true;
            }
        }
    }

    pub fn accumulate_anomaly(&mut self, element: ElementTag, amount: f64) {
        let gauge = self.anomaly_buildup.entry(element).or_insert(0.0);
        *gauge += amount;
    }

    pub fn resistance_for(&self, element: &ElementTag) -> f64 {
        self.resistances.get(element).copied().unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_enemy() -> EnemyState {
        EnemyState::new("boss_01", EnemyType::Boss, 100.0)
    }

    #[test]
    fn test_stun_trigger() {
        let mut enemy = sample_enemy();
        assert!(!enemy.is_stunned);
        enemy.add_daze(100.0);
        assert!(enemy.is_stunned);
        // 处于异常状态时，异常条不应继续累积
        enemy.add_daze(50.0);
        assert_eq!(enemy.stun_gauge, 100.0);
    }

    #[test]
    fn test_stun_no_trigger_below_max() {
        let mut enemy = sample_enemy();
        enemy.add_daze(50.0);
        assert!(!enemy.is_stunned);
        assert_eq!(enemy.stun_gauge, 50.0);
    }

    #[test]
    fn test_chain_limits() {
        assert_eq!(
            EnemyState::new("n", EnemyType::Normal, 100.0).chain_attack_limit(),
            1
        );
        assert_eq!(
            EnemyState::new("e", EnemyType::Elite, 100.0).chain_attack_limit(),
            2
        );
        assert_eq!(sample_enemy().chain_attack_limit(), 3);
    }

    #[test]
    fn test_anomaly_accumulation() {
        let mut enemy = sample_enemy();
        enemy.accumulate_anomaly(ElementTag::Fire, 30.0);
        enemy.accumulate_anomaly(ElementTag::Fire, 20.0);
        assert_eq!(*enemy.anomaly_buildup.get(&ElementTag::Fire).unwrap(), 50.0);
        // 不同元素拥有独立的积蓄条
        enemy.accumulate_anomaly(ElementTag::Ice, 25.0);
        assert_eq!(*enemy.anomaly_buildup.get(&ElementTag::Ice).unwrap(), 25.0);
    }

    #[test]
    fn test_resistances_default() {
        let enemy = sample_enemy();
        assert_eq!(enemy.resistance_for(&ElementTag::Fire), 1.0);
    }

    #[test]
    fn test_resistances_custom() {
        let mut enemy = sample_enemy();
        enemy.resistances.insert(ElementTag::Ice, 0.5);
        enemy.resistances.insert(ElementTag::Fire, 0.0);
        assert_eq!(enemy.resistance_for(&ElementTag::Ice), 0.5);
        assert_eq!(enemy.resistance_for(&ElementTag::Fire), 0.0);
        assert_eq!(enemy.resistance_for(&ElementTag::Physical), 1.0);
    }

    #[test]
    fn test_deserialize_from_data_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/enemies/boss_dullahan.json"
        );
        let json = std::fs::read_to_string(path).expect("read enemy JSON file");
        let enemy: EnemyState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(enemy.enemy_id, "boss_dullahan");
        assert_eq!(enemy.enemy_type, EnemyType::Boss);
        assert_eq!(enemy.hp, 150000.0);
        assert_eq!(enemy.def_val, 600.0);
        assert_eq!(enemy.base_res, 0.15);
        assert_eq!(enemy.stun_gauge, 0.0);
        assert_eq!(enemy.stun_max, 200.0);
        assert_eq!(enemy.resistance_for(&ElementTag::Ice), 0.4);
        assert_eq!(enemy.resistance_for(&ElementTag::Ether), 0.6);
        assert_eq!(enemy.resistance_for(&ElementTag::Fire), 1.0);
        assert!(enemy.weaknesses.contains(&ElementTag::Fire));
        assert!(enemy.weaknesses.contains(&ElementTag::Physical));
        assert_eq!(enemy.chain_attack_limit(), 3);
    }

    #[test]
    fn test_deserialize_with_def_alias() {
        // Python EnemyState 使用 alias="def" 作为防御值字段
        let json = r#"{
            "enemy_id": "test_enemy",
            "enemy_type": "Elite",
            "hp": 50000.0,
            "def": 400.0,
            "daze_current": 30.0,
            "daze_max": 120.0,
            "resistances": {"Ice": 0.5, "Fire": 0.0},
            "weaknesses": ["Electric"]
        }"#;
        let enemy: EnemyState = serde_json::from_str(json).expect("deserialize");
        assert_eq!(enemy.enemy_id, "test_enemy");
        assert_eq!(enemy.enemy_type, EnemyType::Elite);
        assert_eq!(enemy.hp, 50000.0);
        assert_eq!(enemy.def_val, 400.0);
        assert_eq!(enemy.stun_gauge, 30.0);
        assert_eq!(enemy.stun_max, 120.0);
        assert_eq!(enemy.resistance_for(&ElementTag::Ice), 0.5);
        assert!(enemy.weaknesses.contains(&ElementTag::Electric));
    }

    #[test]
    fn test_deserialize_with_def_val() {
        let json = r#"{"enemy_id": "e", "def_val": 300.0}"#;
        let enemy: EnemyState = serde_json::from_str(json).expect("deserialize");
        assert_eq!(enemy.def_val, 300.0);
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut enemy = sample_enemy();
        enemy.hp = 80000.0;
        enemy.def_val = 500.0;
        enemy.stun_max = 150.0;
        enemy.resistances.insert(ElementTag::Fire, 0.2);
        enemy.weaknesses.insert(ElementTag::Ice);

        let json = serde_json::to_string(&enemy).expect("serialize");
        let back: EnemyState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.enemy_id, enemy.enemy_id);
        assert_eq!(back.hp, 80000.0);
        assert_eq!(back.def_val, 500.0);
        assert_eq!(back.stun_max, 150.0);
        assert_eq!(back.resistance_for(&ElementTag::Fire), 0.2);
        assert!(back.weaknesses.contains(&ElementTag::Ice));
    }
}
