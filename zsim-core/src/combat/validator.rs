use std::fmt;

use crate::calculation::anomaly::AnomalyDisorderManager;
use crate::combat::skill::SkillData;
use crate::combat::team::TeamManager;
use crate::entities::character::Character;
use crate::entities::enums::{ElementTag, SpecialtyTag};

/// Resource types that can be validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Energy,
    Hp,
    Decibel,
    ChainPoint,
    Cooldown,
    SwitchCooldown,
    CharacterState,
    AnomalyState,
    Role,
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceType::Energy => write!(f, "energy"),
            ResourceType::Hp => write!(f, "HP"),
            ResourceType::Decibel => write!(f, "decibel"),
            ResourceType::ChainPoint => write!(f, "chain_point"),
            ResourceType::Cooldown => write!(f, "cooldown"),
            ResourceType::SwitchCooldown => write!(f, "switch_cooldown"),
            ResourceType::CharacterState => write!(f, "character_state"),
            ResourceType::AnomalyState => write!(f, "anomaly_state"),
            ResourceType::Role => write!(f, "role"),
        }
    }
}

/// Structured error returned when a resource validation check fails.
#[derive(Debug, Clone)]
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
            "action '{}' missing resource '{}': current={}, required={}",
            self.action_id, self.missing_resource, self.current_value, self.required_value
        )
    }
}

impl std::error::Error for ValidationError {}

/// Pre-action resource validator: 8 independent checks plus an aggregate.
///
/// Follows the unit-struct pattern used by damage calculation modules.
#[derive(Debug, Clone, Copy)]
pub struct ResourceValidator;

impl ResourceValidator {
    /// Validate that the character has enough energy.
    pub fn validate_energy(
        character: &Character,
        required_energy: f64,
        action_id: &str,
    ) -> Result<(), ValidationError> {
        let current = character.resources.energy;
        if current >= required_energy - 1e-9 {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::Energy,
                current_value: current,
                required_value: required_energy,
            })
        }
    }

    /// Validate that the skill cooldown has elapsed.
    ///
    /// A cooldown of 0 ticks means the skill is always ready (no cooldown).
    pub fn validate_cooldown(
        last_used_tick: u64,
        cooldown_ticks: u64,
        current_tick: u64,
        action_id: &str,
    ) -> Result<(), ValidationError> {
        if cooldown_ticks == 0 {
            return Ok(());
        }
        let ready_tick = last_used_tick.saturating_add(cooldown_ticks);
        if current_tick >= ready_tick {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::Cooldown,
                current_value: current_tick as f64,
                required_value: ready_tick as f64,
            })
        }
    }

    /// Validate that the character has enough HP to pay the cost.
    pub fn validate_hp(
        character: &Character,
        hp_cost: f64,
        action_id: &str,
    ) -> Result<(), ValidationError> {
        let current = character.current_stats.hp;
        if current >= hp_cost - 1e-9 {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::Hp,
                current_value: current,
                required_value: hp_cost,
            })
        }
    }

    /// Validate that the character has enough decibel (ult energy).
    pub fn validate_decibel(
        character: &Character,
        required_decibel: f64,
        action_id: &str,
    ) -> Result<(), ValidationError> {
        let current = character.resources.decibel;
        if current >= required_decibel - 1e-9 {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::Decibel,
                current_value: current,
                required_value: required_decibel,
            })
        }
    }

    /// Validate that the character has enough chain points.
    pub fn validate_chain_point(
        character: &Character,
        required_points: u32,
        action_id: &str,
    ) -> Result<(), ValidationError> {
        let current = character.resources.chain_points;
        if current >= required_points {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::ChainPoint,
                current_value: current as f64,
                required_value: required_points as f64,
            })
        }
    }

    /// Validate that the team's switch is not on cooldown.
    pub fn validate_switch_cooldown(
        team: &TeamManager,
        action_id: &str,
    ) -> Result<(), ValidationError> {
        let remaining = team.switch_cooldown();
        if remaining == 0 {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::SwitchCooldown,
                current_value: remaining as f64,
                required_value: 0.0,
            })
        }
    }

    /// Validate that a specific anomaly is active on the target enemy.
    pub fn validate_anomaly_state(
        anomaly_mgr: &AnomalyDisorderManager,
        enemy_id: &str,
        required_element: &ElementTag,
        action_id: &str,
    ) -> Result<(), ValidationError> {
        if anomaly_mgr.is_active(enemy_id, required_element) {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::AnomalyState,
                current_value: 0.0,
                required_value: 1.0,
            })
        }
    }

    /// Validate that the character has the required specialty/role.
    pub fn validate_role(
        character: &Character,
        required_role: &SpecialtyTag,
        action_id: &str,
    ) -> Result<(), ValidationError> {
        if character.specialty == *required_role {
            Ok(())
        } else {
            Err(ValidationError {
                action_id: action_id.to_string(),
                missing_resource: ResourceType::Role,
                current_value: 0.0,
                required_value: 1.0,
            })
        }
    }

    /// Run all applicable validations and collect errors.
    ///
    /// Only checks resources with non-zero requirements. Optional checks
    /// (anomaly_state, role) are included only when their respective
    /// `Option` parameters are `Some`.
    ///
    /// Returns `Vec<ValidationError>` — empty means all checks passed.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_all(
        character: &Character,
        skill: &SkillData,
        team: &TeamManager,
        anomaly_mgr: &AnomalyDisorderManager,
        enemy_id: &str,
        required_element: Option<&ElementTag>,
        required_role: Option<&SpecialtyTag>,
        required_chain_points: u32,
        last_used_tick: u64,
        current_tick: u64,
        action_id: &str,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Energy
        if skill.energy_cost > 0.0 {
            if let Err(e) = Self::validate_energy(character, skill.energy_cost, action_id) {
                errors.push(e);
            }
        }

        // Cooldown
        if skill.cooldown_ticks > 0 {
            if let Err(e) =
                Self::validate_cooldown(last_used_tick, skill.cooldown_ticks, current_tick, action_id)
            {
                errors.push(e);
            }
        }

        // HP
        if skill.hp_cost > 0.0 {
            if let Err(e) = Self::validate_hp(character, skill.hp_cost, action_id) {
                errors.push(e);
            }
        }

        // Decibel
        if skill.decibel_cost > 0.0 {
            if let Err(e) = Self::validate_decibel(character, skill.decibel_cost, action_id) {
                errors.push(e);
            }
        }

        // Chain points
        if required_chain_points > 0 {
            if let Err(e) = Self::validate_chain_point(character, required_chain_points, action_id) {
                errors.push(e);
            }
        }

        // Switch cooldown
        if let Err(e) = Self::validate_switch_cooldown(team, action_id) {
            errors.push(e);
        }

        // Anomaly state (optional)
        if let Some(element) = required_element {
            if let Err(e) = Self::validate_anomaly_state(anomaly_mgr, enemy_id, element, action_id) {
                errors.push(e);
            }
        }

        // Role (optional)
        if let Some(role) = required_role {
            if let Err(e) = Self::validate_role(character, role, action_id) {
                errors.push(e);
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::enums::{FactionTag, SkillType};
    use crate::entities::models::BaseStats;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn make_character(hp: f64) -> Character {
        let mut c = Character::new(
            "test_char",
            FactionTag::GentleHouse,
            SpecialtyTag::Attack,
            ElementTag::Physical,
            BaseStats::default().with_hp(hp),
        );
        c.current_stats = BaseStats::default().with_hp(hp);
        c
    }

    fn make_skill_with_costs(
        energy: f64,
        hp: f64,
        decibel: f64,
        cooldown: u64,
    ) -> SkillData {
        SkillData {
            action_id: "test_skill".into(),
            action_type: SkillType::Special,
            damage_multipliers: vec![],
            daze_multiplier: 0.0,
            hit_frames: vec![],
            invincible_frames: vec![],
            interruptible_frame: 0,
            is_snapshot: false,
            charge_branches: vec![],
            prerequisite_action_id: None,
            hp_cost: hp,
            energy_cost: energy,
            decibel_cost: decibel,
            cooldown_ticks: cooldown,
            animation_frames: 1,
        }
    }

    fn make_team() -> TeamManager {
        TeamManager::new(vec![
            make_character(8000.0),
            make_character(6000.0),
            make_character(5000.0),
        ])
    }

    fn make_anomaly_mgr() -> AnomalyDisorderManager {
        AnomalyDisorderManager::new()
    }

    // ------------------------------------------------------------------
    // validate_energy
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_energy_sufficient() {
        let mut c = make_character(8000.0);
        c.resources.energy = 50.0;
        assert!(ResourceValidator::validate_energy(&c, 40.0, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_energy_insufficient() {
        let mut c = make_character(8000.0);
        c.resources.energy = 10.0;
        let err = ResourceValidator::validate_energy(&c, 40.0, "skill_1").unwrap_err();
        assert_eq!(err.action_id, "skill_1");
        assert_eq!(err.missing_resource, ResourceType::Energy);
        assert!((err.current_value - 10.0).abs() < 1e-9);
        assert!((err.required_value - 40.0).abs() < 1e-9);
    }

    #[test]
    fn test_validate_energy_exact() {
        let mut c = make_character(8000.0);
        c.resources.energy = 40.0;
        assert!(ResourceValidator::validate_energy(&c, 40.0, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_energy_zero_cost() {
        let mut c = make_character(8000.0);
        c.resources.energy = 0.0;
        assert!(ResourceValidator::validate_energy(&c, 0.0, "skill_1").is_ok());
    }

    // ------------------------------------------------------------------
    // validate_cooldown
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_cooldown_ready() {
        assert!(ResourceValidator::validate_cooldown(10, 30, 50, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_cooldown_not_ready() {
        let err =
            ResourceValidator::validate_cooldown(10, 30, 35, "skill_1").unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::Cooldown);
        assert!((err.current_value - 35.0).abs() < 1e-9);
        assert!((err.required_value - 40.0).abs() < 1e-9);
    }

    #[test]
    fn test_validate_cooldown_exact_ready() {
        assert!(ResourceValidator::validate_cooldown(10, 30, 40, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_cooldown_zero_cooldown() {
        assert!(ResourceValidator::validate_cooldown(50, 0, 50, "skill_1").is_ok());
        assert!(ResourceValidator::validate_cooldown(50, 0, 30, "skill_1").is_ok());
    }

    // ------------------------------------------------------------------
    // validate_hp
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_hp_sufficient() {
        let c = make_character(8000.0);
        assert!(ResourceValidator::validate_hp(&c, 5000.0, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_hp_insufficient() {
        let c = make_character(8000.0);
        let err = ResourceValidator::validate_hp(&c, 9000.0, "skill_1").unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::Hp);
        assert!((err.current_value - 8000.0).abs() < 1e-9);
        assert!((err.required_value - 9000.0).abs() < 1e-9);
    }

    #[test]
    fn test_validate_hp_exact() {
        let c = make_character(8000.0);
        assert!(ResourceValidator::validate_hp(&c, 8000.0, "skill_1").is_ok());
    }

    // ------------------------------------------------------------------
    // validate_decibel
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_decibel_sufficient() {
        let mut c = make_character(8000.0);
        c.resources.decibel = 2000.0;
        assert!(ResourceValidator::validate_decibel(&c, 1500.0, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_decibel_insufficient() {
        let mut c = make_character(8000.0);
        c.resources.decibel = 500.0;
        let err = ResourceValidator::validate_decibel(&c, 1000.0, "skill_1").unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::Decibel);
        assert!((err.current_value - 500.0).abs() < 1e-9);
        assert!((err.required_value - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_validate_decibel_exact() {
        let mut c = make_character(8000.0);
        c.resources.decibel = 3000.0;
        assert!(ResourceValidator::validate_decibel(&c, 3000.0, "skill_1").is_ok());
    }

    // ------------------------------------------------------------------
    // validate_chain_point
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_chain_point_sufficient() {
        let mut c = make_character(8000.0);
        c.resources.chain_points = 3;
        assert!(ResourceValidator::validate_chain_point(&c, 1, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_chain_point_insufficient() {
        let c = make_character(8000.0);
        let err = ResourceValidator::validate_chain_point(&c, 1, "skill_1").unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::ChainPoint);
        assert!((err.current_value - 0.0).abs() < 1e-9);
        assert!((err.required_value - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_validate_chain_point_zero_required() {
        let mut c = make_character(8000.0);
        c.resources.chain_points = 0;
        assert!(ResourceValidator::validate_chain_point(&c, 0, "skill_1").is_ok());
    }

    // ------------------------------------------------------------------
    // validate_switch_cooldown
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_switch_cooldown_ready() {
        let team = make_team();
        assert!(ResourceValidator::validate_switch_cooldown(&team, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_switch_cooldown_active() {
        let mut team = make_team();
        team.switch_cooldown_remaining = 15;
        let err =
            ResourceValidator::validate_switch_cooldown(&team, "skill_1").unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::SwitchCooldown);
        assert!((err.current_value - 15.0).abs() < 1e-9);
        assert!((err.required_value - 0.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // validate_anomaly_state
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_anomaly_state_active() {
        let mut mgr = make_anomaly_mgr();
        mgr.accumulate("enemy_1", ElementTag::Fire, 100.0);
        // Fire anomaly accumulates to 100 = trigger threshold, becomes active
        assert!(ResourceValidator::validate_anomaly_state(
            &mgr,
            "enemy_1",
            &ElementTag::Fire,
            "skill_1"
        )
        .is_ok());
    }

    #[test]
    fn test_validate_anomaly_state_inactive() {
        let mgr = make_anomaly_mgr();
        let err = ResourceValidator::validate_anomaly_state(
            &mgr,
            "enemy_1",
            &ElementTag::Fire,
            "skill_1",
        )
        .unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::AnomalyState);
        assert!((err.current_value - 0.0).abs() < 1e-9);
        assert!((err.required_value - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_validate_anomaly_state_different_element() {
        let mut mgr = make_anomaly_mgr();
        mgr.accumulate("enemy_1", ElementTag::Fire, 100.0);
        // Fire is active, but we need Electric
        let err = ResourceValidator::validate_anomaly_state(
            &mgr,
            "enemy_1",
            &ElementTag::Electric,
            "skill_1",
        )
        .unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::AnomalyState);
    }

    // ------------------------------------------------------------------
    // validate_role
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_role_matches() {
        let c = make_character(8000.0); // SpecialtyTag::Attack
        assert!(ResourceValidator::validate_role(&c, &SpecialtyTag::Attack, "skill_1").is_ok());
    }

    #[test]
    fn test_validate_role_mismatch() {
        let c = make_character(8000.0);
        let err =
            ResourceValidator::validate_role(&c, &SpecialtyTag::Support, "skill_1").unwrap_err();
        assert_eq!(err.missing_resource, ResourceType::Role);
        assert!((err.current_value - 0.0).abs() < 1e-9);
        assert!((err.required_value - 1.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // validate_all
    // ------------------------------------------------------------------

    #[test]
    fn test_validate_all_passes() {
        let mut c = make_character(8000.0);
        c.resources.energy = 100.0;
        c.resources.decibel = 3000.0;
        c.resources.chain_points = 2;

        let skill = make_skill_with_costs(40.0, 0.0, 2000.0, 30);
        let team = make_team();
        let mgr = make_anomaly_mgr();

        let errors = ResourceValidator::validate_all(
            &c,
            &skill,
            &team,
            &mgr,
            "enemy_1",
            None,
            None,
            0,
            50,  // last_used_tick
            100, // current_tick
            "skill_1",
        );
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_validate_all_energy_error() {
        let c = make_character(8000.0); // energy = 0
        let skill = make_skill_with_costs(40.0, 0.0, 0.0, 0);
        let team = make_team();
        let mgr = make_anomaly_mgr();

        let errors = ResourceValidator::validate_all(
            &c,
            &skill,
            &team,
            &mgr,
            "enemy_1",
            None,
            None,
            0,
            0,
            100,
            "skill_1",
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, ResourceType::Energy);
    }

    #[test]
    fn test_validate_all_multiple_errors() {
        let c = make_character(500.0); // low HP, energy=0
        let skill = make_skill_with_costs(40.0, 1000.0, 0.0, 0);
        let mut team = make_team();
        team.switch_cooldown_remaining = 10;
        let mgr = make_anomaly_mgr();

        let errors = ResourceValidator::validate_all(
            &c,
            &skill,
            &team,
            &mgr,
            "enemy_1",
            Some(&ElementTag::Fire),
            None,
            1,
            0,
            100,
            "skill_1",
        );
        // Expected: energy + HP + switch_cooldown + anomaly_state + chain_point
        assert_eq!(errors.len(), 5);
        let types: Vec<ResourceType> = errors.iter().map(|e| e.missing_resource).collect();
        assert!(types.contains(&ResourceType::Energy));
        assert!(types.contains(&ResourceType::Hp));
        assert!(types.contains(&ResourceType::SwitchCooldown));
        assert!(types.contains(&ResourceType::AnomalyState));
        assert!(types.contains(&ResourceType::ChainPoint));
    }

    #[test]
    fn test_validate_all_skips_zero_cost() {
        let mut c = make_character(8000.0);
        c.resources.chain_points = 1;
        let skill = make_skill_with_costs(0.0, 0.0, 0.0, 0);
        let team = make_team();
        let mgr = make_anomaly_mgr();

        let errors = ResourceValidator::validate_all(
            &c,
            &skill,
            &team,
            &mgr,
            "enemy_1",
            None,
            Some(&SpecialtyTag::Attack),
            1,
            0,
            100,
            "skill_1",
        );
        // No errors: only role (matches) and chain_points (sufficient) are checked
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_validate_all_cooldown_error() {
        let mut c = make_character(8000.0);
        c.resources.energy = 100.0;
        let skill = make_skill_with_costs(0.0, 0.0, 0.0, 30);
        let team = make_team();
        let mgr = make_anomaly_mgr();

        let errors = ResourceValidator::validate_all(
            &c,
            &skill,
            &team,
            &mgr,
            "enemy_1",
            None,
            None,
            0,
            50, // last_used_tick = 50, so ready_tick = 80
            70, // current_tick = 70 < 80
            "skill_1",
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, ResourceType::Cooldown);
    }

    // ------------------------------------------------------------------
    // Display / error trait
    // ------------------------------------------------------------------

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError {
            action_id: "ult_1".into(),
            missing_resource: ResourceType::Energy,
            current_value: 10.0,
            required_value: 40.0,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("ult_1"));
        assert!(msg.contains("energy"));
        assert!(msg.contains("10"));
        assert!(msg.contains("40"));
    }

    #[test]
    fn test_resource_type_display() {
        assert_eq!(format!("{}", ResourceType::Energy), "energy");
        assert_eq!(format!("{}", ResourceType::Hp), "HP");
        assert_eq!(format!("{}", ResourceType::SwitchCooldown), "switch_cooldown");
        assert_eq!(format!("{}", ResourceType::AnomalyState), "anomaly_state");
        assert_eq!(format!("{}", ResourceType::Role), "role");
    }

    #[test]
    fn test_validation_error_implements_std_error() {
        let err = ValidationError {
            action_id: "test".into(),
            missing_resource: ResourceType::Energy,
            current_value: 5.0,
            required_value: 20.0,
        };
        let err_ref: &dyn std::error::Error = &err;
        assert!(err_ref.downcast_ref::<ValidationError>().is_some());
    }

    #[test]
    fn test_resource_type_clone_copy() {
        let rt = ResourceType::Decibel;
        let rt2 = rt; // Copy
        assert_eq!(rt, rt2);
    }
}
