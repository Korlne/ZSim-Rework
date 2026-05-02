use serde::{Deserialize, Serialize};

/// 角色和装备的 12 项战斗属性。
/// Serde 别名保持与 Python 原始 JSON 字段名的向后兼容性。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BaseStats {
    #[serde(alias = "atk", default)]
    pub atk: f64,
    #[serde(alias = "def", alias = "def_val", default)]
    pub def: f64,
    #[serde(alias = "hp", default)]
    pub hp: f64,
    #[serde(alias = "crit_rate", default)]
    pub crit_rate: f64,
    #[serde(alias = "crit_dmg", default)]
    pub crit_dmg: f64,
    #[serde(alias = "pen_fixed", default)]
    pub pen: f64,
    #[serde(alias = "pen_ratio", default)]
    pub pen_ratio: f64,
    #[serde(alias = "anomaly_mastery", default)]
    pub anomaly_mastery: f64,
    #[serde(alias = "anomaly_proficiency", default)]
    pub anomaly_proficiency: f64,
    #[serde(alias = "impact", default)]
    pub impact: f64,
    #[serde(alias = "energy_regen", default)]
    pub energy_regen: f64,
    #[serde(alias = "dmg_bonus", alias = "energy_gen_rate", default)]
    pub dmg_bonus: f64,
}

impl BaseStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_atk(mut self, v: f64) -> Self {
        self.atk = v.max(0.0);
        self
    }

    pub fn with_def(mut self, v: f64) -> Self {
        self.def = v.max(0.0);
        self
    }

    pub fn with_hp(mut self, v: f64) -> Self {
        self.hp = v.max(0.0);
        self
    }

    pub fn with_crit_rate(mut self, v: f64) -> Self {
        self.crit_rate = v.max(0.0);
        self
    }

    pub fn with_crit_dmg(mut self, v: f64) -> Self {
        self.crit_dmg = v.max(0.0);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_all_zeros() {
        let stats = BaseStats::default();
        assert_eq!(stats.atk, 0.0);
        assert_eq!(stats.def, 0.0);
        assert_eq!(stats.hp, 0.0);
        assert_eq!(stats.crit_rate, 0.0);
        assert_eq!(stats.crit_dmg, 0.0);
        assert_eq!(stats.pen, 0.0);
        assert_eq!(stats.pen_ratio, 0.0);
        assert_eq!(stats.anomaly_mastery, 0.0);
        assert_eq!(stats.anomaly_proficiency, 0.0);
        assert_eq!(stats.impact, 0.0);
        assert_eq!(stats.energy_regen, 0.0);
        assert_eq!(stats.dmg_bonus, 0.0);
    }

    #[test]
    fn test_builder_methods_clamp_negatives() {
        let stats = BaseStats::new()
            .with_atk(-50.0)
            .with_crit_rate(-0.5)
            .with_hp(-10.0);
        assert_eq!(stats.atk, 0.0);
        assert_eq!(stats.crit_rate, 0.0);
        assert_eq!(stats.hp, 0.0);
    }

    #[test]
    fn test_deserialize_python_style_json() {
        // Python BaseStats 使用：hp、atk、def_val、impact、anomaly_proficiency、
        // anomaly_mastery、crit_rate、crit_dmg、pen_ratio、pen_fixed、
        // energy_regen、energy_gen_rate
        let json = r#"{
            "hp": 8000.0,
            "atk": 1200.0,
            "def": 600.0,
            "impact": 95.0,
            "anomaly_proficiency": 105.0,
            "anomaly_mastery": 92.0,
            "crit_rate": 0.25,
            "crit_dmg": 1.2,
            "pen_ratio": 0.15,
            "pen_fixed": 50.0,
            "energy_regen": 1.5,
            "energy_gen_rate": 0.2
        }"#;
        let stats: BaseStats = serde_json::from_str(json).expect("deserialize");
        assert_eq!(stats.hp, 8000.0);
        assert_eq!(stats.atk, 1200.0);
        assert_eq!(stats.def, 600.0);
        assert_eq!(stats.crit_rate, 0.25);
        assert_eq!(stats.crit_dmg, 1.2);
        assert_eq!(stats.pen, 50.0);
        assert_eq!(stats.pen_ratio, 0.15);
        assert_eq!(stats.anomaly_mastery, 92.0);
        assert_eq!(stats.anomaly_proficiency, 105.0);
        assert_eq!(stats.impact, 95.0);
        assert_eq!(stats.energy_regen, 1.5);
        assert_eq!(stats.dmg_bonus, 0.2);
    }

    #[test]
    fn test_deserialize_with_def_val_alias() {
        let json = r#"{"def_val": 500.0}"#;
        let stats: BaseStats = serde_json::from_str(json).expect("deserialize");
        assert_eq!(stats.def, 500.0);
    }

    #[test]
    fn test_serialize_roundtrip() {
        let stats = BaseStats {
            atk: 1500.0,
            def: 700.0,
            hp: 10000.0,
            crit_rate: 0.3,
            crit_dmg: 1.5,
            pen: 80.0,
            pen_ratio: 0.2,
            anomaly_mastery: 100.0,
            anomaly_proficiency: 110.0,
            impact: 90.0,
            energy_regen: 1.8,
            dmg_bonus: 0.5,
        };
        let json = serde_json::to_string(&stats).expect("serialize");
        let roundtripped: BaseStats = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(stats.atk, roundtripped.atk);
        assert_eq!(stats.def, roundtripped.def);
        assert_eq!(stats.hp, roundtripped.hp);
        assert_eq!(stats.crit_rate, roundtripped.crit_rate);
        assert_eq!(stats.dmg_bonus, roundtripped.dmg_bonus);
    }

    #[test]
    fn test_partial_json_defaults_missing() {
        let json = r#"{"atk": 500.0, "hp": 3000.0}"#;
        let stats: BaseStats = serde_json::from_str(json).expect("deserialize");
        assert_eq!(stats.atk, 500.0);
        assert_eq!(stats.hp, 3000.0);
        assert_eq!(stats.def, 0.0); // 默认值
        assert_eq!(stats.crit_rate, 0.0);
    }
}
