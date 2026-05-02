use crate::calculation::rng::RNGManager;
use crate::entities::enums::ElementTag;

/// 所有 6 个计算模块共享的输入参数。
///
/// 每个字段都有明确的默认值，因此对于只设置被测模块相关字段的测试，
/// 部分初始化也能正常工作。
#[derive(Debug, Clone)]
pub struct CalcInput {
    // ---- 普通 / 异常伤害 ----
    pub atk: f64,
    pub multiplier: f64,
    pub dmg_bonus: f64,

    // ---- 暴击 ----
    pub crit_rate: f64,
    pub crit_dmg: f64,

    // ---- 穿透与防御 ----
    pub pen: f64,
    pub pen_ratio: f64,
    pub def: f64,

    // ---- 敌人减伤 ----
    /// 最终抗性乘数（0.0 = 免疫，1.0 = 无抗性）。
    pub res_factor: f64,
    pub dmg_reduction: f64,

    /// 伤害类型的元素属性。
    pub element: ElementTag,

    // ---- 异常 ----
    pub anomaly_mastery: f64,
    pub anomaly_proficiency: f64,
    pub anomaly_multiplier: f64,
    pub anomaly_dmg_bonus: f64,
    /// 敌人当前的异常累积值（本次攻击前）。
    pub anomaly_gauge_current: f64,
    /// 异常触发所需的阈值。
    pub anomaly_threshold: f64,

    // ---- 击晕 ----
    pub impact: f64,
    pub daze_multiplier: f64,
    pub daze_bonus: f64,
    pub daze_resistance: f64,

    // ---- 紊乱 ----
    pub old_gauge: f64,
    pub new_gauge: f64,
    pub disorder_multiplier: f64,
}

/// 与特定计算模块无关的字段的默认值。
impl Default for CalcInput {
    fn default() -> Self {
        Self {
            atk: 0.0,
            multiplier: 0.0,
            dmg_bonus: 0.0,
            crit_rate: 0.0,
            crit_dmg: 0.0,
            pen: 0.0,
            pen_ratio: 0.0,
            def: 0.0,
            res_factor: 1.0,
            dmg_reduction: 0.0,
            element: ElementTag::Physical,
            anomaly_mastery: 0.0,
            anomaly_proficiency: 0.0,
            anomaly_multiplier: 0.0,
            anomaly_dmg_bonus: 0.0,
            anomaly_gauge_current: 0.0,
            anomaly_threshold: 100.0,
            impact: 0.0,
            daze_multiplier: 0.0,
            daze_bonus: 0.0,
            daze_resistance: 0.0,
            old_gauge: 0.0,
            new_gauge: 0.0,
            disorder_multiplier: 0.0,
        }
    }
}

/// 任何计算模块产生的输出。
///
/// 与调用模块无关的字段保持其默认值（零/false）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CalcOutput {
    /// 最终造成的伤害（普通、异常或紊乱）。
    pub damage: f64,
    /// 此次攻击是否为暴击。
    pub is_crit: bool,
    /// 对敌人造成的击晕（daze）伤害。
    pub stun_damage: f64,
    /// 此次攻击增加的异常累积值。
    pub anomaly_gauge: f64,
    /// 异常累积值是否已达到阈值。
    pub triggered_anomaly: bool,
}

// ---------------------------------------------------------------------------
// 1. RegularMul – 普通伤害公式
// ---------------------------------------------------------------------------
//
//   伤害 = 攻击力 × 倍率 × (1 + 伤害加成) × 暴击系数 × 防御系数
//          × 抗性系数 × (1 - 伤害减免)
//
//   暴击系数 = 1.0              （非暴击）
//               1.0 + 暴击伤害   （暴击）
//   防御系数 = 1.0 / (1.0 + 有效防御 / 1000)
//   有效防御 = 防御 × (1 - 穿透比例) - 固定穿透

pub struct RegularMul;

impl RegularMul {
    pub fn calc(&self, input: &CalcInput, rng: &mut RNGManager) -> CalcOutput {
        let is_crit = rng.gen_crit(input.crit_rate);
        let crit_factor = if is_crit { 1.0 + input.crit_dmg } else { 1.0 };

        let effective_def = (input.def * (1.0 - input.pen_ratio) - input.pen).max(0.0);
        let def_factor = 1.0 / (1.0 + effective_def / 1000.0);

        let damage = input.atk
            * input.multiplier
            * (1.0 + input.dmg_bonus)
            * crit_factor
            * def_factor
            * input.res_factor
            * (1.0 - input.dmg_reduction);

        CalcOutput {
            damage,
            is_crit,
            stun_damage: 0.0,
            anomaly_gauge: 0.0,
            triggered_anomaly: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 2. AnomalyMul – 异常伤害公式
// ---------------------------------------------------------------------------
//
//   伤害 = 攻击力 × 异常倍率 × 精通系数 × (1 + 异常伤害加成)
//
//   精通系数 = 1.0 + 异常精通 / 100

pub struct AnomalyMul;

impl AnomalyMul {
    pub fn calc(&self, input: &CalcInput, _rng: &mut RNGManager) -> CalcOutput {
        let prof_factor = 1.0 + input.anomaly_proficiency / 100.0;
        let damage =
            input.atk * input.anomaly_multiplier * prof_factor * (1.0 + input.anomaly_dmg_bonus);

        CalcOutput {
            damage,
            is_crit: false,
            stun_damage: 0.0,
            anomaly_gauge: 0.0,
            triggered_anomaly: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 3. StunMul – 击晕（daze）伤害公式
// ---------------------------------------------------------------------------
//
//   击晕值 = 冲击力 × 眩晕倍率 × (1 + 眩晕加成) × (1 - 眩晕抗性)

pub struct StunMul;

impl StunMul {
    pub fn calc(&self, input: &CalcInput, _rng: &mut RNGManager) -> CalcOutput {
        let stun_damage = input.impact
            * input.daze_multiplier
            * (1.0 + input.daze_bonus)
            * (1.0 - input.daze_resistance);

        CalcOutput {
            damage: 0.0,
            is_crit: false,
            stun_damage,
            anomaly_gauge: 0.0,
            triggered_anomaly: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 4. CalAnomaly – 异常累积
// ---------------------------------------------------------------------------
//
//   累积值 = 异常掌控 × 倍率
//   是否触发 = (当前累积值 + 新增累积值) >= 阈值

pub struct CalAnomaly;

impl CalAnomaly {
    pub fn calc(&self, input: &CalcInput, _rng: &mut RNGManager) -> CalcOutput {
        let gauge_added = input.anomaly_mastery * input.multiplier;
        let total_gauge = input.anomaly_gauge_current + gauge_added;
        let triggered = total_gauge >= input.anomaly_threshold;

        CalcOutput {
            damage: 0.0,
            is_crit: false,
            stun_damage: 0.0,
            anomaly_gauge: gauge_added,
            triggered_anomaly: triggered,
        }
    }
}

// ---------------------------------------------------------------------------
// 5. CalDisorder – 紊乱伤害
// ---------------------------------------------------------------------------
//
//   伤害 = (旧累积值 + 新累积值) × 紊乱倍率

pub struct CalDisorder;

impl CalDisorder {
    pub fn calc(&self, input: &CalcInput, _rng: &mut RNGManager) -> CalcOutput {
        let damage = (input.old_gauge + input.new_gauge) * input.disorder_multiplier;

        CalcOutput {
            damage,
            is_crit: false,
            stun_damage: 0.0,
            anomaly_gauge: 0.0,
            triggered_anomaly: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 6. CalPolarityDisorder – 极性紊乱伤害
// ---------------------------------------------------------------------------
//
//   伤害 = (旧累积值 + 新累积值) × 紊乱倍率 × 极性系数
//
// 极性紊乱发生在对已处于活跃状态的同元素异常再次施加该元素异常时。
// 1.5 倍系数为代表性取值（实际数值可能因角色配置而异）。

pub struct CalPolarityDisorder;

impl CalPolarityDisorder {
    pub fn calc(&self, input: &CalcInput, _rng: &mut RNGManager) -> CalcOutput {
        let damage = (input.old_gauge + input.new_gauge) * input.disorder_multiplier * 1.5;

        CalcOutput {
            damage,
            is_crit: false,
            stun_damage: 0.0,
            anomaly_gauge: 0.0,
            triggered_anomaly: false,
        }
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- RegularMul 测试 ------------------------------------------------

    /// 使用简单值的基础非暴击攻击。
    #[test]
    fn test_regular_mul_basic() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            atk: 1000.0,
            multiplier: 2.0,
            dmg_bonus: 0.3,
            crit_rate: 0.0, // force non-crit
            def: 500.0,
            res_factor: 1.0,
            ..Default::default()
        };
        // effective_def = 500, def_factor = 1/1.5 ≈ 0.6666667
        // 伤害 = 1000 × 2 × 1.3 × 1 × 0.6666667 × 1 × 1 ≈ 1733.333...
        let out = RegularMul.calc(&input, &mut rng);
        let expected = 1000.0 * 2.0 * 1.3 * (1.0 / 1.5);
        assert!(!out.is_crit);
        assert!((out.damage - expected).abs() < 1e-9);
    }

    /// 暴击应应用暴击伤害倍率。
    #[test]
    fn test_regular_mul_crit() {
        let mut rng = RNGManager::new(42);
        let input = CalcInput {
            atk: 1000.0,
            multiplier: 1.0,
            dmg_bonus: 0.0,
            crit_rate: 1.0, // force crit
            crit_dmg: 1.5,
            def: 0.0,
            res_factor: 1.0,
            ..Default::default()
        };
        let out = RegularMul.calc(&input, &mut rng);
        assert!(out.is_crit);
        assert!((out.damage - 2500.0).abs() < 1e-9); // 1000 × 2.5 = 2500
    }

    /// 穿透减少有效防御。
    #[test]
    fn test_regular_mul_pen() {
        let mut rng = RNGManager::new(1);
        // 50% 穿透比例下：effective_def = 500 * 0.5 = 250
        // def_factor = 1/1.25 = 0.8
        let input = CalcInput {
            atk: 1000.0,
            multiplier: 1.0,
            dmg_bonus: 0.0,
            crit_rate: 0.0,
            def: 500.0,
            pen_ratio: 0.5,
            res_factor: 1.0,
            ..Default::default()
        };
        let out = RegularMul.calc(&input, &mut rng);
        let expected = 1000.0 * (1.0 / 1.25);
        assert!((out.damage - expected).abs() < 1e-9);
    }

    /// 抗性和伤害减免都会减少最终伤害。
    #[test]
    fn test_regular_mul_resistance_and_dr() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            atk: 1000.0,
            multiplier: 1.0,
            dmg_bonus: 0.0,
            crit_rate: 0.0,
            def: 0.0,
            res_factor: 0.5,    // 50% 抗性
            dmg_reduction: 0.2, // 20% 伤害减免
            ..Default::default()
        };
        let out = RegularMul.calc(&input, &mut rng);
        assert!((out.damage - 400.0).abs() < 1e-9); // 1000 × 0.5 × 0.8
    }

    /// 攻击力为零时造成零伤害。
    #[test]
    fn test_regular_mul_zero_atk() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            atk: 0.0,
            multiplier: 2.0,
            ..Default::default()
        };
        let out = RegularMul.calc(&input, &mut rng);
        assert!(!out.is_crit);
        assert_eq!(out.damage, 0.0);
    }

    /// 相同种子、相同输入 → 相同伤害（RNG 是确定性的）。
    #[test]
    fn test_regular_mul_deterministic() {
        let mut rng_a = RNGManager::new(42);
        let mut rng_b = RNGManager::new(42);
        let input = CalcInput {
            atk: 1200.0,
            multiplier: 1.5,
            dmg_bonus: 0.2,
            crit_rate: 0.3,
            crit_dmg: 1.0,
            def: 400.0,
            res_factor: 1.0,
            ..Default::default()
        };
        let a = RegularMul.calc(&input, &mut rng_a);
        let b = RegularMul.calc(&input, &mut rng_b);
        assert_eq!(a.damage, b.damage);
        assert_eq!(a.is_crit, b.is_crit);
    }

    // ---- AnomalyMul 测试 ------------------------------------------------

    /// 基础异常伤害。
    #[test]
    fn test_anomaly_mul_basic() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            atk: 2000.0,
            anomaly_multiplier: 5.0,
            anomaly_proficiency: 100.0, // 精通系数 = 2.0
            anomaly_dmg_bonus: 0.3,
            ..Default::default()
        };
        let out = AnomalyMul.calc(&input, &mut rng);
        let expected = 2000.0 * 5.0 * 2.0 * 1.3; // 26000
        assert!((out.damage - expected).abs() < 1e-9);
        assert!(!out.is_crit); // 异常从不暴击
    }

    /// 异常精通缩放。
    #[test]
    fn test_anomaly_mul_proficiency_scaling() {
        let mut rng = RNGManager::new(1);
        // 精通为 0 时，精通系数 = 1.0
        let input = CalcInput {
            atk: 1000.0,
            anomaly_multiplier: 4.0,
            anomaly_proficiency: 0.0,
            ..Default::default()
        };
        let out = AnomalyMul.calc(&input, &mut rng);
        assert!((out.damage - 4000.0).abs() < 1e-9);

        // 精通为 200 时，精通系数 = 3.0
        let input2 = CalcInput {
            atk: 1000.0,
            anomaly_multiplier: 4.0,
            anomaly_proficiency: 200.0,
            ..Default::default()
        };
        let out2 = AnomalyMul.calc(&input2, &mut rng);
        assert!((out2.damage - 12000.0).abs() < 1e-9);
    }

    // ---- StunMul 测试 ---------------------------------------------------

    /// 基础击晕伤害。
    #[test]
    fn test_stun_mul_basic() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            impact: 100.0,
            daze_multiplier: 2.0,
            daze_bonus: 0.25,
            daze_resistance: 0.0,
            ..Default::default()
        };
        let out = StunMul.calc(&input, &mut rng);
        assert!((out.stun_damage - 250.0).abs() < 1e-9); // 100 × 2 × 1.25
    }

    /// 眩晕抗性减少击晕伤害。
    #[test]
    fn test_stun_mul_resistance() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            impact: 100.0,
            daze_multiplier: 1.0,
            daze_resistance: 0.4,
            ..Default::default()
        };
        let out = StunMul.calc(&input, &mut rng);
        assert!((out.stun_damage - 60.0).abs() < 1e-9); // 100 × 1 × 0.6
    }

    /// 冲击力为零时造成零击晕。
    #[test]
    fn test_stun_mul_zero_impact() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            impact: 0.0,
            daze_multiplier: 2.0,
            ..Default::default()
        };
        let out = StunMul.calc(&input, &mut rng);
        assert_eq!(out.stun_damage, 0.0);
    }

    // ---- CalAnomaly 测试 ------------------------------------------------

    /// 基础异常累积值累积。
    #[test]
    fn test_cal_anomaly_basic() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            anomaly_mastery: 80.0,
            multiplier: 1.0,
            anomaly_gauge_current: 0.0,
            anomaly_threshold: 100.0,
            ..Default::default()
        };
        let out = CalAnomaly.calc(&input, &mut rng);
        assert!((out.anomaly_gauge - 80.0).abs() < 1e-9);
        assert!(!out.triggered_anomaly);
    }

    /// 累积值达到阈值 → triggered_anomaly = true。
    #[test]
    fn test_cal_anomaly_triggers_at_threshold() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            anomaly_mastery: 80.0,
            multiplier: 1.0,
            anomaly_gauge_current: 30.0, // 30 + 80 = 110 且 110 >= 100
            anomaly_threshold: 100.0,
            ..Default::default()
        };
        let out = CalAnomaly.calc(&input, &mut rng);
        assert!(out.triggered_anomaly);
    }

    /// 恰好达到阈值时触发异常。
    #[test]
    fn test_cal_anomaly_exactly_at_threshold() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            anomaly_mastery: 50.0,
            multiplier: 1.0,
            anomaly_gauge_current: 50.0, // 50 + 50 = 100 且 100 >= 100
            anomaly_threshold: 100.0,
            ..Default::default()
        };
        let out = CalAnomaly.calc(&input, &mut rng);
        assert!(out.triggered_anomaly);
    }

    // ---- CalDisorder 测试 -----------------------------------------------

    /// 基础紊乱伤害。
    #[test]
    fn test_cal_disorder_basic() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            old_gauge: 60.0,
            new_gauge: 80.0,
            disorder_multiplier: 5.0,
            ..Default::default()
        };
        let out = CalDisorder.calc(&input, &mut rng);
        assert!((out.damage - 700.0).abs() < 1e-9); // (60+80) × 5
    }

    /// 累积值为零时造成零紊乱伤害。
    #[test]
    fn test_cal_disorder_zero_gauges() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            disorder_multiplier: 5.0,
            ..Default::default()
        };
        let out = CalDisorder.calc(&input, &mut rng);
        assert_eq!(out.damage, 0.0);
    }

    // ---- CalPolarityDisorder 测试 ---------------------------------------

    /// 极性紊乱额外应用 1.5 倍乘数。
    #[test]
    fn test_cal_polarity_disorder_basic() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            old_gauge: 60.0,
            new_gauge: 80.0,
            disorder_multiplier: 5.0,
            ..Default::default()
        };
        let out = CalPolarityDisorder.calc(&input, &mut rng);
        // (60+80) × 5 × 1.5 = 1050
        assert!((out.damage - 1050.0).abs() < 1e-9);
    }

    // ---- 跨模块一致性 ---------------------------------------------------

    /// 紊乱 vs 极性紊乱：极性应为普通紊乱的 1.5 倍。
    #[test]
    fn test_polarity_vs_regular_disorder_ratio() {
        let mut rng_a = RNGManager::new(1);
        let mut rng_b = RNGManager::new(1);
        let input = CalcInput {
            old_gauge: 75.0,
            new_gauge: 90.0,
            disorder_multiplier: 4.0,
            ..Default::default()
        };
        let regular = CalDisorder.calc(&input, &mut rng_a);
        let polarity = CalPolarityDisorder.calc(&input, &mut rng_b);
        assert!((polarity.damage / regular.damage - 1.5).abs() < 1e-9);
    }

    /// RegularMul 在 0% 暴击率下产生固定伤害（无 RNG 依赖）。
    #[test]
    fn test_regular_mul_zero_crit_deterministic() {
        // 两个不同的 RNG 种子，但 0% 暴击率 → 相同结果。
        let mut rng_a = RNGManager::new(1);
        let mut rng_b = RNGManager::new(999);
        let input = CalcInput {
            atk: 1000.0,
            multiplier: 1.0,
            dmg_bonus: 0.0,
            crit_rate: 0.0,
            def: 0.0,
            res_factor: 1.0,
            ..Default::default()
        };
        let a = RegularMul.calc(&input, &mut rng_a);
        let b = RegularMul.calc(&input, &mut rng_b);
        assert_eq!(a.damage, b.damage);
        assert!(!a.is_crit);
        assert!(!b.is_crit);
    }

    /// 精度：所有计算均使用 f64，应与手动计算的参考值在 1e-9 范围内。
    #[test]
    fn test_precision_1e9() {
        let mut rng = RNGManager::new(1);

        // RegularMul：精心选择的值，避免浮点数陷阱。
        let reg = CalcInput {
            atk: 1234.0,
            multiplier: 3.5,
            dmg_bonus: 0.15,
            crit_rate: 1.0,
            crit_dmg: 0.8,
            pen: 50.0,
            pen_ratio: 0.2,
            def: 600.0,
            res_factor: 0.8,
            dmg_reduction: 0.1,
            ..Default::default()
        };
        // effective_def = 600 * 0.8 - 50 = 430
        // def_factor = 1 / 1.43 = 0.6993006993...
        // 伤害 = 1234 * 3.5 * 1.15 * 1.8 * 0.6993006993 * 0.8 * 0.9
        let effective_def = f64::max(600.0 * 0.8 - 50.0, 0.0);
        let df = 1.0 / (1.0 + effective_def / 1000.0);
        let expected = 1234.0 * 3.5 * 1.15 * 1.8 * df * 0.8 * 0.9;
        let out = RegularMul.calc(&reg, &mut rng);
        assert!((out.damage - expected).abs() < 1e-9);

        // StunMul 测试
        let stun = CalcInput {
            impact: 95.0,
            daze_multiplier: 2.5,
            daze_bonus: 0.3,
            daze_resistance: 0.15,
            ..Default::default()
        };
        let expected_stun = 95.0 * 2.5 * 1.3 * 0.85;
        let out_stun = StunMul.calc(&stun, &mut rng);
        assert!((out_stun.stun_damage - expected_stun).abs() < 1e-9);

        // AnomalyMul 测试
        let anom = CalcInput {
            atk: 1500.0,
            anomaly_multiplier: 6.0,
            anomaly_proficiency: 150.0,
            anomaly_dmg_bonus: 0.4,
            ..Default::default()
        };
        let expected_anom = 1500.0 * 6.0 * (1.0 + 150.0 / 100.0) * 1.4;
        let out_anom = AnomalyMul.calc(&anom, &mut rng);
        assert!((out_anom.damage - expected_anom).abs() < 1e-9);
    }
}
