use crate::calculation::rng::RNGManager;
use crate::entities::enums::ElementTag;

/// Input parameters shared across all 6 calculation modules.
///
/// Every field has a clear default so partial initialization works for
/// tests that only set the fields relevant to the module under test.
#[derive(Debug, Clone)]
pub struct CalcInput {
    // ---- Regular / Anomaly damage ----
    pub atk: f64,
    pub multiplier: f64,
    pub dmg_bonus: f64,

    // ---- Crit ----
    pub crit_rate: f64,
    pub crit_dmg: f64,

    // ---- Penetration & Defense ----
    pub pen: f64,
    pub pen_ratio: f64,
    pub def: f64,

    // ---- Enemy mitigation ----
    /// Final resistance multiplier (0.0 = immune, 1.0 = no resistance).
    pub res_factor: f64,
    pub dmg_reduction: f64,

    /// Element for damage typing.
    pub element: ElementTag,

    // ---- Anomaly ----
    pub anomaly_mastery: f64,
    pub anomaly_proficiency: f64,
    pub anomaly_multiplier: f64,
    pub anomaly_dmg_bonus: f64,
    /// Current buildup gauge on the enemy (before this hit).
    pub anomaly_gauge_current: f64,
    /// Threshold at which the anomaly triggers.
    pub anomaly_threshold: f64,

    // ---- Stun ----
    pub impact: f64,
    pub daze_multiplier: f64,
    pub daze_bonus: f64,
    pub daze_resistance: f64,

    // ---- Disorder ----
    pub old_gauge: f64,
    pub new_gauge: f64,
    pub disorder_multiplier: f64,
}

/// Default values for fields not relevant to a particular calc module.
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

/// Output produced by any calc module.
///
/// Fields not relevant to the calling module are left at their default (zero/false).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CalcOutput {
    /// Final damage dealt (regular, anomaly, or disorder).
    pub damage: f64,
    /// Whether the hit was a critical strike.
    pub is_crit: bool,
    /// Stun (daze) damage applied to the enemy.
    pub stun_damage: f64,
    /// Anomaly buildup gauge added by this hit.
    pub anomaly_gauge: f64,
    /// Whether the anomaly gauge reached the threshold.
    pub triggered_anomaly: bool,
}

// ---------------------------------------------------------------------------
// 1. RegularMul – regular damage formula
// ---------------------------------------------------------------------------
//
//   DMG = ATK × multiplier × (1 + DMG_BONUS) × crit_factor × def_factor
//         × res_factor × (1 - dmg_reduction)
//
//   crit_factor = 1.0          (non-crit)
//                 1.0 + CRIT_DMG (crit)
//   def_factor  = 1.0 / (1.0 + effective_DEF / 1000)
//   effective_DEF = DEF × (1 - PEN_RATIO) - PEN

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
// 2. AnomalyMul – anomaly damage formula
// ---------------------------------------------------------------------------
//
//   DMG = ATK × anomaly_multiplier × proficiency_factor × (1 + anomaly_dmg_bonus)
//
//   proficiency_factor = 1.0 + ANOMALY_PROFICIENCY / 100

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
// 3. StunMul – stun (daze) damage formula
// ---------------------------------------------------------------------------
//
//   STUN = IMPACT × daze_multiplier × (1 + daze_bonus) × (1 - daze_resistance)

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
// 4. CalAnomaly – anomaly accumulation
// ---------------------------------------------------------------------------
//
//   gauge_added = ANOMALY_MASTERY × multiplier
//   triggered   = (gauge_current + gauge_added) >= threshold

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
// 5. CalDisorder – disorder damage
// ---------------------------------------------------------------------------
//
//   DMG = (old_gauge + new_gauge) × disorder_multiplier

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
// 6. CalPolarityDisorder – polarity disorder damage
// ---------------------------------------------------------------------------
//
//   DMG = (old_gauge + new_gauge) × disorder_multiplier × polarity_factor
//
// Polarity disorder occurs when applying an anomaly of one element on top of
// the same element's anomaly already active.  The 1.5× multiplier is a
// representative value (actual tuning may vary by character kit).

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
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- RegularMul tests ------------------------------------------------

    /// Basic non-crit hit with simple values.
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
        // dmg = 1000 × 2 × 1.3 × 1 × 0.6666667 × 1 × 1 ≈ 1733.333...
        let out = RegularMul.calc(&input, &mut rng);
        let expected = 1000.0 * 2.0 * 1.3 * (1.0 / 1.5);
        assert!(!out.is_crit);
        assert!((out.damage - expected).abs() < 1e-9);
    }

    /// Crit hit should apply crit_dmg multiplier.
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
        assert!((out.damage - 2500.0).abs() < 1e-9); // 1000 × 2.5
    }

    /// Penetration reduces effective defense.
    #[test]
    fn test_regular_mul_pen() {
        let mut rng = RNGManager::new(1);
        // With 50% PEN_RATIO: effective_def = 500 * 0.5 = 250
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

    /// Resistance and damage reduction both reduce final damage.
    #[test]
    fn test_regular_mul_resistance_and_dr() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            atk: 1000.0,
            multiplier: 1.0,
            dmg_bonus: 0.0,
            crit_rate: 0.0,
            def: 0.0,
            res_factor: 0.5,    // 50% resistance
            dmg_reduction: 0.2, // 20% DR
            ..Default::default()
        };
        let out = RegularMul.calc(&input, &mut rng);
        assert!((out.damage - 400.0).abs() < 1e-9); // 1000 × 0.5 × 0.8
    }

    /// Zero ATK produces zero damage.
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

    /// Same seed, same input → same damage (RNG is deterministic).
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

    // ---- AnomalyMul tests ------------------------------------------------

    /// Basic anomaly damage.
    #[test]
    fn test_anomaly_mul_basic() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            atk: 2000.0,
            anomaly_multiplier: 5.0,
            anomaly_proficiency: 100.0, // prof_factor = 2.0
            anomaly_dmg_bonus: 0.3,
            ..Default::default()
        };
        let out = AnomalyMul.calc(&input, &mut rng);
        let expected = 2000.0 * 5.0 * 2.0 * 1.3; // 26000
        assert!((out.damage - expected).abs() < 1e-9);
        assert!(!out.is_crit); // anomaly never crits
    }

    /// Anomaly proficiency scaling.
    #[test]
    fn test_anomaly_mul_proficiency_scaling() {
        let mut rng = RNGManager::new(1);
        // At 0 proficiency, prof_factor = 1.0
        let input = CalcInput {
            atk: 1000.0,
            anomaly_multiplier: 4.0,
            anomaly_proficiency: 0.0,
            ..Default::default()
        };
        let out = AnomalyMul.calc(&input, &mut rng);
        assert!((out.damage - 4000.0).abs() < 1e-9);

        // At 200 proficiency, prof_factor = 3.0
        let input2 = CalcInput {
            atk: 1000.0,
            anomaly_multiplier: 4.0,
            anomaly_proficiency: 200.0,
            ..Default::default()
        };
        let out2 = AnomalyMul.calc(&input2, &mut rng);
        assert!((out2.damage - 12000.0).abs() < 1e-9);
    }

    // ---- StunMul tests ---------------------------------------------------

    /// Basic stun damage.
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

    /// Daze resistance reduces stun damage.
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

    /// Zero impact produces zero stun.
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

    // ---- CalAnomaly tests ------------------------------------------------

    /// Basic anomaly gauge accumulation.
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

    /// Gauge reaches threshold → triggered_anomaly = true.
    #[test]
    fn test_cal_anomaly_triggers_at_threshold() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            anomaly_mastery: 80.0,
            multiplier: 1.0,
            anomaly_gauge_current: 30.0, // 30 + 80 = 110 >= 100
            anomaly_threshold: 100.0,
            ..Default::default()
        };
        let out = CalAnomaly.calc(&input, &mut rng);
        assert!(out.triggered_anomaly);
    }

    /// Exactly at threshold triggers anomaly.
    #[test]
    fn test_cal_anomaly_exactly_at_threshold() {
        let mut rng = RNGManager::new(1);
        let input = CalcInput {
            anomaly_mastery: 50.0,
            multiplier: 1.0,
            anomaly_gauge_current: 50.0, // 50 + 50 = 100 >= 100
            anomaly_threshold: 100.0,
            ..Default::default()
        };
        let out = CalAnomaly.calc(&input, &mut rng);
        assert!(out.triggered_anomaly);
    }

    // ---- CalDisorder tests -----------------------------------------------

    /// Basic disorder damage.
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

    /// Zero gauges produce zero disorder damage.
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

    // ---- CalPolarityDisorder tests ---------------------------------------

    /// Polarity disorder applies a 1.5× multiplier on top.
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

    // ---- Cross-module consistency ----------------------------------------

    /// Disorder vs PolarityDisorder: polarity should be 1.5× regular disorder.
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

    /// RegularMul with 0% crit produces a fixed damage (no RNG dependency).
    #[test]
    fn test_regular_mul_zero_crit_deterministic() {
        // Two different RNG seeds, but 0% crit → same result.
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

    /// Precision: all calculations use f64 and should be within 1e-9 of
    /// hand-computed reference values.
    #[test]
    fn test_precision_1e9() {
        let mut rng = RNGManager::new(1);

        // RegularMul: carefully chosen values that avoid floating-point pitfalls.
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
        // dmg = 1234 * 3.5 * 1.15 * 1.8 * 0.6993006993 * 0.8 * 0.9
        let effective_def = f64::max(600.0 * 0.8 - 50.0, 0.0);
        let df = 1.0 / (1.0 + effective_def / 1000.0);
        let expected = 1234.0 * 3.5 * 1.15 * 1.8 * df * 0.8 * 0.9;
        let out = RegularMul.calc(&reg, &mut rng);
        assert!((out.damage - expected).abs() < 1e-9);

        // StunMul
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

        // AnomalyMul
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
