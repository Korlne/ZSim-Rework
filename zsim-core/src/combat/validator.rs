use std::collections::HashMap;
use std::fmt;

use crate::combat::skill::SkillData;
use crate::combat::team::TeamManager;
use crate::entities::character::Character;
use crate::entities::enemy::EnemyState;
use crate::entities::enums::{SkillType, SpecialtyTag};

/// 可能验证失败的资源类别。
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

/// 资源验证检查失败时返回的结构化错误。
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

/// 执行前资源验证器，筛选技能执行资格。
///
/// 追踪技能冷却时间，并在允许动作执行前验证 8 个资源维度。
#[derive(Debug, Clone)]
pub struct ResourceValidator {
    /// 映射 action_id -> 该技能冷却过期时的 tick。
    cooldowns: HashMap<String, u64>,
}

impl ResourceValidator {
    /// 创建一个没有追踪冷却时间的新验证器。
    pub fn new() -> Self {
        Self {
            cooldowns: HashMap::new(),
        }
    }

    // ------------------------------------------------------------------
    // 冷却管理
    // ------------------------------------------------------------------

    /// 为动作记录冷却时间。如果 `cooldown_ticks == 0` 则不执行任何操作。
    pub fn set_cooldown(&mut self, action_id: &str, cooldown_ticks: u64, current_tick: u64) {
        if cooldown_ticks > 0 {
            self.cooldowns
                .insert(action_id.to_string(), current_tick + cooldown_ticks);
        }
    }

    /// 手动清除冷却时间（例如冷却重置或增益效果）。
    pub fn clear_cooldown(&mut self, action_id: &str) {
        self.cooldowns.remove(action_id);
    }

    /// 获取动作的剩余冷却 tick 数（如果就绪或从未设置则为 0）。
    pub fn remaining_cooldown(&self, action_id: &str, current_tick: u64) -> u64 {
        self.cooldowns
            .get(action_id)
            .map(|&expiry| expiry.saturating_sub(current_tick))
            .unwrap_or(0)
    }

    // ------------------------------------------------------------------
    // 维度 1：能量
    // ------------------------------------------------------------------

    /// 验证角色有足够的能量支付技能消耗。
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
    // 维度 2：冷却
    // ------------------------------------------------------------------

    /// 验证技能在当前 tick 不在冷却中。
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
    // 维度 3：HP
    // ------------------------------------------------------------------

    /// 验证角色有足够的 HP 支付技能的 HP 消耗。
    ///
    /// 使用严格的大于检查 HP —— 0 HP 意味着死亡，消耗后角色必须存活。
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
    // 维度 4：Decibel
    // ------------------------------------------------------------------

    /// 验证上场角色有足够的 Decibel 支付技能消耗。
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
    // 维度 5：连携点数
    // ------------------------------------------------------------------

    /// 验证上场角色至少有 1 个连携点数。
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
    // 维度 6：切换冷却
    // ------------------------------------------------------------------

    /// 验证队伍切换不在冷却中。
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
    // 维度 7：异常状态（敌人免疫检查）
    // ------------------------------------------------------------------

    /// 验证敌人对角色的元素没有免疫。
    ///
    /// 抗性为 0.0 表示完全免疫 —— 角色无法对该敌人触发异常。
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
    // 维度 8：角色（专长要求）
    // ------------------------------------------------------------------

    /// 定义限制技能类型所允许的专长。
    ///
    /// `None` 表示该技能类型没有角色限制。
    fn required_specialties(action_type: &SkillType) -> Option<&'static [SpecialtyTag]> {
        match action_type {
            // 协同攻击仅对下场角色可用
            SkillType::Coordinated => Some(&[
                SpecialtyTag::Support,
                SpecialtyTag::Anomaly,
                SpecialtyTag::Rupture,
            ]),
            // 所有其他技能类型没有角色限制
            _ => None,
        }
    }

    /// 验证角色的专长是否符合技能类型的要求。
    ///
    /// 大多数技能类型没有限制。受限类型（例如 Coordinated）对照允许的专长列表进行检查。
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
    // 聚合验证：validate_all
    // ------------------------------------------------------------------

    /// 运行所有适用的验证并收集所有失败。
    ///
    /// 与单个验证器不同，这**不会**短路 ——
    /// 它会评估所有 8 个维度并返回发现的每个错误。
    ///
    /// **条件检查：**
    /// - 连携点数：仅在 `skill.action_type == Chain` 时检查。
    /// - 切换冷却：仅对 `Assist` / `QuickAssist` 检查。
    /// - 异常状态：仅在 `enemy` 为 `Some` 时检查。
    ///
    /// 当所有检查通过时返回空的 `Vec`。
    pub fn validate_all(
        &self,
        character: &Character,
        skill: &SkillData,
        team: &TeamManager,
        current_tick: u64,
        enemy: Option<&EnemyState>,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // 1. 能量
        if let Err(e) = self.validate_energy(character, skill) {
            errors.push(e);
        }

        // 2. 技能冷却
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

        // 5. 连携点数（适用于 Chain 类型技能）
        if skill.action_type == SkillType::Chain {
            if let Err(e) = self.validate_chain_point(&skill.action_id, team) {
                errors.push(e);
            }
        }

        // 6. 切换冷却（适用于触发切换的动作）
        if matches!(
            skill.action_type,
            SkillType::Assist | SkillType::QuickAssist
        ) {
            if let Err(e) = self.validate_switch_cooldown(&skill.action_id, team) {
                errors.push(e);
            }
        }

        // 7. 异常状态（需要敌人上下文）
        if let Some(enemy) = enemy {
            if let Err(e) = self.validate_anomaly_state(character, enemy) {
                errors.push(e);
            }
        }

        // 8. 角色
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
    // 辅助函数
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
    // 冷却管理（5 个测试）
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
    // validate_energy（2 个测试）
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
    // validate_hp（3 个测试）
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
    // validate_decibel（2 个测试）
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
    // validate_chain_point（2 个测试）
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
    // validate_switch_cooldown（2 个测试）
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
    // validate_anomaly_state（2 个测试）
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
    // validate_cooldown（2 个测试）
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
    // validate_role（3 个测试）
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
    // validate_all（9 个测试）
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
    // 集成：完整流程与冷却生命周期（2 个测试）
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

        // 所有检查应该通过
        let errors = v.validate_all(&character, &skill, &team, 0, Some(&enemy));
        assert!(
            errors.is_empty(),
            "expected all validations to pass, got: {:?}",
            errors
        );

        // 执行技能：设置冷却
        let mut v = v;
        v.set_cooldown(&skill.action_id, skill.cooldown_ticks, 0);

        // 立即复用应该触发冷却失败
        let errors = v.validate_all(&character, &skill, &team, 5, Some(&enemy));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, ResourceType::SkillCooldown);

        // 冷却过期后，应该再次通过
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
    // 错误显示与 trait 实现（2 个测试）
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
    // 默认实现
    // ------------------------------------------------------------------

    #[test]
    fn test_default_creates_empty() {
        let v: ResourceValidator = Default::default();
        assert_eq!(v.remaining_cooldown("anything", 0), 0);
    }
}
