use std::collections::HashMap;
use std::fmt;

use crate::combat::skill::SkillData;
use crate::combat::team::TeamManager;
use crate::entities::character::Character;
use crate::entities::enemy::EnemyState;
use crate::entities::enums::{SkillType, SpecialtyTag};

/// Categories of resources that can fail validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceType {
    Energy,
    Hp,
    Decibel,
    ChainPoint,
    SkillCooldown,
    SwitchCooldown,
    AnomalyResistance,
    RoleRequirement,
}

/// Structured error returned when a resource validation check fails.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub action_id: String,
    pub missing_resource: ResourceType,
    pub current_value: f64,
    pub required_value: f64,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "action '{}' missing {:?}: current={}, required={}",
            self.action_id, self.missing_resource, self.current_value, self.required_value
        )
    }
}

impl std::error::Error for ValidationError {}

/// Pre-execution resource validator that screens skill execution eligibility.
///
/// Tracks skill cooldowns and validates 8 resource dimensions before
/// allowing an action to proceed.
#[derive(Debug, Clone)]
pub struct ResourceValidator {
    /// Maps action_id -> tick when that skill's cooldown expires.
    cooldowns: HashMap<String, u64>,
}

impl ResourceValidator {
    /// Create a new validator with no tracked cooldowns.
    pub fn new() -> Self {
        Self {
            cooldowns: HashMap::new(),
        }
    }

    // ------------------------------------------------------------------
    // Cooldown management
    // ------------------------------------------------------------------

    /// Record a cooldown for an action. No-ops if `cooldown_ticks == 0`.
    pub fn set_cooldown(&mut self, action_id: &str, cooldown_ticks: u64, current_tick: u64) {
        if cooldown_ticks > 0 {
            self.cooldowns
                .insert(action_id.to_string(), current_tick + cooldown_ticks);
        }
    }

    /// Manually clear a cooldown (e.g., on cooldown reset or buff).
    pub fn clear_cooldown(&mut self, action_id: &str) {
        self.cooldowns.remove(action_id);
    }

    /// Get remaining cooldown ticks for an action (0 if ready or never set).
    pub fn remaining_cooldown(&self, action_id: &str, current_tick: u64) -> u64 {
        self.cooldowns
            .get(action_id)
            .map(|&expiry| expiry.saturating_sub(current_tick))
            .unwrap_or(0)
    }

    // ------------------------------------------------------------------
    // Dimension 1: Energy
    // ------------------------------------------------------------------

    /// Validate the character has enough energy for the skill cost.
    pub fn validate_energy(
        &self,
        character: &Character,
        skill: &SkillData,
    ) -> Result<(), ValidationError> {
        if skill.energy_cost <= 0.0 {
            return Ok(());
        }
        if character.resources.energy >= skill.energy_cost {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: skill.action_id.clone(),
                missing_resource: ResourceType::Energy,
                current_value: character.resources.energy,
                required_value: skill.energy_cost,
            })
        }
    }

    // ------------------------------------------------------------------
    // Dimension 2: Cooldown
    // ------------------------------------------------------------------

    /// Validate the skill is off cooldown at the current tick.
    pub fn validate_cooldown(
        &self,
        action_id: &str,
        current_tick: u64,
    ) -> Result<(), ValidationError> {
        let remaining = self.remaining_cooldown(action_id, current_tick);
        if remaining == 0 {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::SkillCooldown,
                current_value: remaining as f64,
                required_value: 0.0,
            })
        }
    }

    // ------------------------------------------------------------------
    // Dimension 3: HP
    // ------------------------------------------------------------------

    /// Validate the character has enough HP to pay the skill's HP cost.
    ///
    /// Uses strict greater-than for HP — 0 HP means dead, and costs must
    /// leave the character alive.
    pub fn validate_hp(
        &self,
        character: &Character,
        skill: &SkillData,
    ) -> Result<(), ValidationError> {
        if skill.hp_cost <= 0.0 {
            return Ok(());
        }
        if character.current_stats.hp > skill.hp_cost {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: skill.action_id.clone(),
                missing_resource: ResourceType::Hp,
                current_value: character.current_stats.hp,
                required_value: skill.hp_cost,
            })
        }
    }

    // ------------------------------------------------------------------
    // Dimension 4: Decibel
    // ------------------------------------------------------------------

    /// Validate the on-field character has enough decibel for the skill cost.
    pub fn validate_decibel(
        &self,
        team: &TeamManager,
        skill: &SkillData,
    ) -> Result<(), ValidationError> {
        if skill.decibel_cost <= 0.0 {
            return Ok(());
        }
        let on_field_decibel = team.decibel_of(team.on_field_index()).unwrap_or(0.0);
        if on_field_decibel >= skill.decibel_cost {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: skill.action_id.clone(),
                missing_resource: ResourceType::Decibel,
                current_value: on_field_decibel,
                required_value: skill.decibel_cost,
            })
        }
    }

    // ------------------------------------------------------------------
    // Dimension 5: Chain point
    // ------------------------------------------------------------------

    /// Validate the on-field character has at least 1 chain point.
    pub fn validate_chain_point(
        &self,
        action_id: &str,
        team: &TeamManager,
    ) -> Result<(), ValidationError> {
        let points = team.chain_points();
        if points >= 1 {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::ChainPoint,
                current_value: points as f64,
                required_value: 1.0,
            })
        }
    }

    // ------------------------------------------------------------------
    // Dimension 6: Switch cooldown
    // ------------------------------------------------------------------

    /// Validate the team switch is not on cooldown.
    pub fn validate_switch_cooldown(
        &self,
        action_id: &str,
        team: &TeamManager,
    ) -> Result<(), ValidationError> {
        if !team.is_switch_on_cooldown() {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::SwitchCooldown,
                current_value: team.switch_cooldown() as f64,
                required_value: 0.0,
            })
        }
    }

    // ------------------------------------------------------------------
    // Dimension 7: Anomaly state (enemy immunity check)
    // ------------------------------------------------------------------

    /// Validate the enemy is not immune to the character's element.
    ///
    /// A resistance of 0.0 means full immunity — the character cannot
    /// trigger anomalies on this enemy.
    pub fn validate_anomaly_state(
        &self,
        character: &Character,
        enemy: &EnemyState,
    ) -> Result<(), ValidationError> {
        let resistance = enemy.resistance_for(&character.element);
        if resistance > 0.0 {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: "anomaly".to_string(),
                missing_resource: ResourceType::AnomalyResistance,
                current_value: resistance,
                required_value: f64::EPSILON,
            })
        }
    }

    // ------------------------------------------------------------------
    // Dimension 8: Role (specialty requirement)
    // ------------------------------------------------------------------

    /// Define which specialties are allowed for restricted skill types.
    ///
    /// `None` means the skill type has no role restriction.
    fn required_specialties(action_type: &SkillType) -> Option<&'static [SpecialtyTag]> {
        match action_type {
            // Coordinated attacks are only available to off-field roles
            SkillType::Coordinated => Some(&[
                SpecialtyTag::Support,
                SpecialtyTag::Anomaly,
                SpecialtyTag::Rupture,
            ]),
            // All other skill types have no role restriction
            _ => None,
        }
    }

    /// Validate the character's specialty is valid for the skill type.
    ///
    /// Most skill types have no restriction. Restricted types
    /// (e.g., Coordinated) check against an allowlist of specialties.
    pub fn validate_role(
        &self,
        character: &Character,
        skill: &SkillData,
    ) -> Result<(), ValidationError> {
        if let Some(required) = Self::required_specialties(&skill.action_type) {
            if required.contains(&character.specialty) {
                Ok(())
            } else {
                Err(ValidationError {
                    action_id: skill.action_id.clone(),
                    missing_resource: ResourceType::RoleRequirement,
                    current_value: 0.0,
                    required_value: 1.0,
                })
            }
        } else {
            Ok(())
        }
    }

    // ------------------------------------------------------------------
    // Aggregated: validate_all
    // ------------------------------------------------------------------

    /// Run all applicable validations and collect every failure.
    ///
    /// Unlike individual validators, this does **not** short-circuit —
    /// it evaluates all 8 dimensions and returns every error found.
    ///
    /// **Conditional checks:**
    /// - Chain point: only checked when `skill.action_type == Chain`.
    /// - Switch cooldown: only checked for `Assist` / `QuickAssist`.
    /// - Anomaly state: only checked when `enemy` is `Some`.
    ///
    /// Returns an empty `Vec` when all checks pass.
    pub fn validate_all(
        &self,
        character: &Character,
        skill: &SkillData,
        team: &TeamManager,
        current_tick: u64,
        enemy: Option<&EnemyState>,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // 1. Energy
        if let Err(e) = self.validate_energy(character, skill) {
            errors.push(e);
        }

        // 2. Skill cooldown
        if let Err(e) = self.validate_cooldown(&skill.action_id, current_tick) {
            errors.push(e);
        }

        // 3. HP
        if let Err(e) = self.validate_hp(character, skill) {
            errors.push(e);
        }

        // 4. Decibel
        if let Err(e) = self.validate_decibel(team, skill) {
            errors.push(e);
        }

        // 5. Chain point (relevant for Chain-type skills)
        if skill.action_type == SkillType::Chain {
            if let Err(e) = self.validate_chain_point(&skill.action_id, team) {
                errors.push(e);
            }
        }

        // 6. Switch cooldown (relevant for switch-triggering actions)
        if matches!(
            skill.action_type,
            SkillType::Assist | SkillType::QuickAssist
        ) {
            if let Err(e) = self.validate_switch_cooldown(&skill.action_id, team) {
                errors.push(e);
            }
        }

        // 7. Anomaly state (requires enemy context)
        if let Some(enemy) = enemy {
            if let Err(e) = self.validate_anomaly_state(character, enemy) {
                errors.push(e);
            }
        }

        // 8. Role
        if let Err(e) = self.validate_role(character, skill) {
            errors.push(e);
        }

        errors
    }
}

impl Default for ResourceValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::enemy::EnemyState;
    use crate::entities::enums::{ElementTag, EnemyType, FactionTag, SkillType, SpecialtyTag};
    use crate::entities::models::BaseStats;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn make_character(
        char_id: &str,
        specialty: SpecialtyTag,
        element: ElementTag,
        energy: f64,
        hp: f64,
    ) -> Character {
        let mut c = Character::new(
            char_id,
            FactionTag::GentleHouse,
            specialty,
            element,
            BaseStats::default().with_hp(hp),
        );
        c.resources.energy = energy;
        c.current_stats.hp = hp;
        c
    }

    fn make_team() -> TeamManager {
        TeamManager::new(vec![
            make_character(
                "char_0",
                SpecialtyTag::Attack,
                ElementTag::Physical,
                100.0,
                8000.0,
            ),
            make_character(
                "char_1",
                SpecialtyTag::Support,
                ElementTag::Ether,
                120.0,
                6000.0,
            ),
            make_character(
                "char_2",
                SpecialtyTag::Anomaly,
                ElementTag::Fire,
                80.0,
                5000.0,
            ),
        ])
    }

    fn make_skill(
        action_id: &str,
        action_type: SkillType,
        energy_cost: f64,
        hp_cost: f64,
        decibel_cost: f64,
        cooldown_ticks: u64,
    ) -> SkillData {
        SkillData {
            action_id: action_id.to_string(),
            action_type,
            damage_multipliers: vec![],
            daze_multiplier: 0.0,
            hit_frames: vec![],
            invincible_frames: vec![],
            interruptible_frame: 0,
            is_snapshot: false,
            charge_branches: vec![],
            prerequisite_action_id: None,
            hp_cost,
            energy_cost,
            decibel_cost,
            cooldown_ticks,
            animation_frames: 30,
        }
    }

    fn make_enemy() -> EnemyState {
        let mut enemy = EnemyState::new("test_enemy", EnemyType::Elite, 100.0);
        enemy.resistances.insert(ElementTag::Fire, 0.5);
        enemy.resistances.insert(ElementTag::Electric, 0.0); // immune
        enemy
    }

    // ------------------------------------------------------------------
    // Cooldown management (5 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_new_validator_no_cooldowns() {
        let v = ResourceValidator::new();
        assert_eq!(v.remaining_cooldown("any_action", 0), 0);
    }

    #[test]
    fn test_set_cooldown_and_remaining() {
        let mut v = ResourceValidator::new();
        v.set_cooldown("skill_a", 100, 50);
        assert_eq!(v.remaining_cooldown("skill_a", 50), 100);
        assert_eq!(v.remaining_cooldown("skill_a", 120), 30);
    }

    #[test]
    fn test_cooldown_expired_returns_zero() {
        let mut v = ResourceValidator::new();
        v.set_cooldown("skill_a", 30, 0);
        assert_eq!(v.remaining_cooldown("skill_a", 0), 30);
        assert_eq!(v.remaining_cooldown("skill_a", 30), 0);
        assert_eq!(v.remaining_cooldown("skill_a", 100), 0);
    }

    #[test]
    fn test_clear_cooldown() {
        let mut v = ResourceValidator::new();
        v.set_cooldown("skill_a", 100, 0);
        v.clear_cooldown("skill_a");
        assert_eq!(v.remaining_cooldown("skill_a", 0), 0);
    }

    #[test]
    fn test_zero_cooldown_not_set() {
        let mut v = ResourceValidator::new();
        v.set_cooldown("skill_a", 0, 0);
        assert_eq!(v.remaining_cooldown("skill_a", 0), 0);
    }

    // ------------------------------------------------------------------
    // validate_energy (2 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_energy_sufficient() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            50.0,
            8000.0,
        );
        let s = make_skill("ult", SkillType::Ultimate, 40.0, 0.0, 0.0, 0);
        assert!(v.validate_energy(&c, &s).is_ok());
    }

    #[test]
    fn test_energy_insufficient() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            10.0,
            8000.0,
        );
        let s = make_skill("ult", SkillType::Ultimate, 40.0, 0.0, 0.0, 0);
        let err = v.validate_energy(&c, &s).unwrap_err();
        assert_eq!(err.action_id, "ult");
        assert_eq!(err.missing_resource, ResourceType::Energy);
        assert!((err.current_value - 10.0).abs() < 1e-9);
        assert!((err.required_value - 40.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // validate_hp (3 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_hp_sufficient() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            0.0,
            8000.0,
        );
        let s = make_skill("hp_cost_skill", SkillType::Special, 0.0, 500.0, 0.0, 0);
        assert!(v.validate_hp(&c, &s).is_ok());
    }

    #[test]
    fn test_hp_insufficient() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            0.0,
            300.0,
        );
        let s = make_skill("hp_cost_skill", SkillType::Special, 0.0, 500.0, 0.0, 0);
        let err = v.validate_hp(&c, &s).unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::Hp);
        assert!((err.current_value - 300.0).abs() < 1e-9);
        assert!((err.required_value - 500.0).abs() < 1e-9);
    }

    #[test]
    fn test_hp_zero_cost_always_ok() {
        let v = ResourceValidator::new();
        let c = make_character("test", SpecialtyTag::Attack, ElementTag::Physical, 0.0, 0.0);
        let s = make_skill("free_skill", SkillType::Normal, 0.0, 0.0, 0.0, 0);
        assert!(v.validate_hp(&c, &s).is_ok());
    }

    // ------------------------------------------------------------------
    // validate_decibel (2 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_decibel_sufficient() {
        let v = ResourceValidator::new();
        let mut team = make_team();
        team.characters[0].resources.decibel = 2000.0;
        let s = make_skill("ult", SkillType::Ultimate, 0.0, 0.0, 1500.0, 0);
        assert!(v.validate_decibel(&team, &s).is_ok());
    }

    #[test]
    fn test_decibel_insufficient() {
        let v = ResourceValidator::new();
        let team = make_team();
        let s = make_skill("ult", SkillType::Ultimate, 0.0, 0.0, 1500.0, 0);
        let err = v.validate_decibel(&team, &s).unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::Decibel);
        assert!((err.current_value - 0.0).abs() < 1e-9);
        assert!((err.required_value - 1500.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // validate_chain_point (2 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_chain_point_sufficient() {
        let v = ResourceValidator::new();
        let mut team = make_team();
        team.characters[0].resources.chain_points = 2;
        assert!(v.validate_chain_point("chain_attack", &team).is_ok());
    }

    #[test]
    fn test_chain_point_insufficient() {
        let v = ResourceValidator::new();
        let team = make_team();
        let err = v.validate_chain_point("chain_attack", &team).unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::ChainPoint);
        assert!((err.current_value - 0.0).abs() < 1e-9);
        assert!((err.required_value - 1.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // validate_switch_cooldown (2 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_switch_cooldown_ready() {
        let v = ResourceValidator::new();
        let team = make_team();
        assert!(v.validate_switch_cooldown("assist", &team).is_ok());
    }

    #[test]
    fn test_switch_cooldown_active() {
        let v = ResourceValidator::new();
        let mut team = make_team();
        team.switch_cooldown_remaining = 15;
        let err = v.validate_switch_cooldown("assist", &team).unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::SwitchCooldown);
        assert!((err.current_value - 15.0).abs() < 1e-9);
        assert!((err.required_value - 0.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // validate_anomaly_state (2 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_anomaly_state_normal_resistance() {
        let v = ResourceValidator::new();
        let c = make_character("test", SpecialtyTag::Anomaly, ElementTag::Fire, 0.0, 8000.0);
        let enemy = make_enemy();
        assert!(v.validate_anomaly_state(&c, &enemy).is_ok());
    }

    #[test]
    fn test_anomaly_state_immune() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Anomaly,
            ElementTag::Electric,
            0.0,
            8000.0,
        );
        let enemy = make_enemy();
        let err = v.validate_anomaly_state(&c, &enemy).unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::AnomalyResistance);
        assert!((err.current_value - 0.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // validate_cooldown (2 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_cooldown_ready() {
        let v = ResourceValidator::new();
        assert!(v.validate_cooldown("any_skill", 0).is_ok());
    }

    #[test]
    fn test_cooldown_not_ready() {
        let mut v = ResourceValidator::new();
        v.set_cooldown("skill_a", 30, 0);
        let err = v.validate_cooldown("skill_a", 10).unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::SkillCooldown);
        assert!((err.current_value - 20.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // validate_role (3 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_role_allowed_for_coordinated() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Support,
            ElementTag::Physical,
            0.0,
            8000.0,
        );
        let s = make_skill("coord", SkillType::Coordinated, 0.0, 0.0, 0.0, 0);
        assert!(v.validate_role(&c, &s).is_ok());
    }

    #[test]
    fn test_role_not_allowed_for_coordinated() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            0.0,
            8000.0,
        );
        let s = make_skill("coord", SkillType::Coordinated, 0.0, 0.0, 0.0, 0);
        let err = v.validate_role(&c, &s).unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::RoleRequirement);
    }

    #[test]
    fn test_role_no_restriction_passes() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            0.0,
            8000.0,
        );
        let s = make_skill("normal_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0);
        assert!(v.validate_role(&c, &s).is_ok());
    }

    // ------------------------------------------------------------------
    // validate_all (9 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_all_no_errors() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            100.0,
            8000.0,
        );
        let s = make_skill("normal_atk", SkillType::Normal, 10.0, 0.0, 0.0, 0);
        let mut team = make_team();
        team.characters[0].resources.energy = 100.0;
        let enemy = make_enemy();
        let errors = v.validate_all(&c, &s, &team, 0, Some(&enemy));
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_validate_all_energy_failure() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            5.0,
            8000.0,
        );
        let s = make_skill("ult", SkillType::Ultimate, 40.0, 0.0, 0.0, 0);
        let team = make_team();
        let errors = v.validate_all(&c, &s, &team, 0, None);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, ResourceType::Energy);
    }

    #[test]
    fn test_validate_all_multiple_errors() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            5.0,
            300.0,
        );
        let s = make_skill("costly_skill", SkillType::Special, 40.0, 500.0, 0.0, 0);
        let team = make_team();
        let errors = v.validate_all(&c, &s, &team, 0, None);
        assert_eq!(errors.len(), 2);
        let types: Vec<_> = errors.iter().map(|e| &e.missing_resource).collect();
        assert!(types.contains(&&ResourceType::Energy));
        assert!(types.contains(&&ResourceType::Hp));
    }

    #[test]
    fn test_validate_all_skip_anomaly_without_enemy() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Anomaly,
            ElementTag::Electric,
            0.0,
            8000.0,
        );
        let s = make_skill("normal_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0);
        let team = make_team();
        let errors = v.validate_all(&c, &s, &team, 0, None);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_all_anomaly_immune_with_enemy() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Anomaly,
            ElementTag::Electric,
            0.0,
            8000.0,
        );
        let s = make_skill("normal_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0);
        let team = make_team();
        let enemy = make_enemy();
        let errors = v.validate_all(&c, &s, &team, 0, Some(&enemy));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, ResourceType::AnomalyResistance);
    }

    #[test]
    fn test_validate_all_chain_point_check() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            0.0,
            8000.0,
        );
        let s = make_skill("chain_atk", SkillType::Chain, 0.0, 0.0, 0.0, 0);
        let team = make_team();
        let errors = v.validate_all(&c, &s, &team, 0, None);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, ResourceType::ChainPoint);
    }

    #[test]
    fn test_validate_all_switch_cooldown_check() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            0.0,
            8000.0,
        );
        let s = make_skill("quick_assist", SkillType::QuickAssist, 0.0, 0.0, 0.0, 0);
        let mut team = make_team();
        team.switch_cooldown_remaining = 10;
        let errors = v.validate_all(&c, &s, &team, 0, None);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, ResourceType::SwitchCooldown);
    }

    #[test]
    fn test_validate_all_non_chain_skips_chain_check() {
        let v = ResourceValidator::new();
        let c = make_character(
            "test",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            0.0,
            8000.0,
        );
        let s = make_skill("normal_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0);
        let team = make_team();
        let errors = v.validate_all(&c, &s, &team, 0, None);
        assert!(errors.is_empty());
    }

    // ------------------------------------------------------------------
    // Integration: full flow with cooldown lifecycle (2 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_full_validation_before_skill_execution() {
        let v = ResourceValidator::new();
        let character = make_character(
            "agent",
            SpecialtyTag::Attack,
            ElementTag::Physical,
            50.0,
            8000.0,
        );
        let skill = make_skill("Ex_Special", SkillType::Special, 30.0, 200.0, 0.0, 15);
        let mut team = make_team();
        team.characters[0].resources.energy = 50.0;
        let enemy = make_enemy();

        // All checks should pass
        let errors = v.validate_all(&character, &skill, &team, 0, Some(&enemy));
        assert!(
            errors.is_empty(),
            "expected all validations to pass, got: {:?}",
            errors
        );

        // Execute skill: set cooldown
        let mut v = v;
        v.set_cooldown(&skill.action_id, skill.cooldown_ticks, 0);

        // Immediate re-use should fail cooldown
        let errors = v.validate_all(&character, &skill, &team, 5, Some(&enemy));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, ResourceType::SkillCooldown);

        // After cooldown expires, should pass again
        let errors = v.validate_all(&character, &skill, &team, 15, Some(&enemy));
        assert!(
            errors.is_empty(),
            "expected cooldown to have expired, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_validate_all_zero_cost_skill() {
        let v = ResourceValidator::new();
        let c = make_character("agent", SpecialtyTag::Stun, ElementTag::Electric, 0.0, 0.0);
        let s = make_skill("basic_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0);
        let team = make_team();
        let errors = v.validate_all(&c, &s, &team, 0, None);
        assert!(errors.is_empty());
    }

    // ------------------------------------------------------------------
    // Error display & trait impl (2 tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError {
            action_id: "test_skill".to_string(),
            missing_resource: ResourceType::Energy,
            current_value: 10.0,
            required_value: 40.0,
        };
        let msg = err.to_string();
        assert!(msg.contains("test_skill"));
        assert!(msg.contains("Energy"));
    }

    #[test]
    fn test_validation_error_implements_std_error() {
        let err = ValidationError {
            action_id: "test".to_string(),
            missing_resource: ResourceType::Energy,
            current_value: 5.0,
            required_value: 20.0,
        };
        let err_ref: &dyn std::error::Error = &err;
        assert!(err_ref.downcast_ref::<ValidationError>().is_some());
    }

    // ------------------------------------------------------------------
    // Default impl
    // ------------------------------------------------------------------

    #[test]
    fn test_default_creates_empty() {
        let v: ResourceValidator = Default::default();
        assert_eq!(v.remaining_cooldown("anything", 0), 0);
    }
}
