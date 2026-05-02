use serde::{Deserialize, Serialize};

use crate::entities::enums::SkillType;

/// A single hit frame with its tick offset and damage multiplier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitFrame {
    pub frame: u64,
    pub multiplier: f64,
}

/// A charge branch: when charging for `charge_duration` ticks, execute `variant_action_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargeBranch {
    pub charge_duration: u64,
    pub variant_action_id: String,
}

/// Full definition of a skill action loaded from data/skills/.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillData {
    pub action_id: String,
    pub action_type: SkillType,
    #[serde(default)]
    pub damage_multipliers: Vec<HitFrame>,
    #[serde(default)]
    pub daze_multiplier: f64,
    #[serde(default)]
    pub hit_frames: Vec<u64>,
    #[serde(default)]
    pub invincible_frames: Vec<u64>,
    #[serde(default)]
    pub interruptible_frame: u64,
    #[serde(default)]
    pub is_snapshot: bool,
    #[serde(default)]
    pub charge_branches: Vec<ChargeBranch>,
    pub prerequisite_action_id: Option<String>,
    #[serde(default)]
    pub hp_cost: f64,
    #[serde(default)]
    pub energy_cost: f64,
    #[serde(default)]
    pub decibel_cost: f64,
    #[serde(default)]
    pub cooldown_ticks: u64,
    /// Total animation frames before the action completes.
    #[serde(default = "default_animation_frames")]
    pub animation_frames: u64,
}

fn default_animation_frames() -> u64 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_minimal_skill() {
        let json = r#"{
            "action_id": "Attack_Normal_1",
            "action_type": "Normal"
        }"#;
        let skill: SkillData = serde_json::from_str(json).expect("deserialize");
        assert_eq!(skill.action_id, "Attack_Normal_1");
        assert_eq!(skill.action_type, SkillType::Normal);
        assert!(skill.damage_multipliers.is_empty());
        assert_eq!(skill.daze_multiplier, 0.0);
        assert_eq!(skill.animation_frames, 1);
        assert!(!skill.is_snapshot);
        assert!(skill.charge_branches.is_empty());
        assert!(skill.prerequisite_action_id.is_none());
    }

    #[test]
    fn test_deserialize_full_skill() {
        let json = r#"{
            "action_id": "Ultimate_1",
            "action_type": "Ultimate",
            "damage_multipliers": [
                {"frame": 10, "multiplier": 1.5},
                {"frame": 25, "multiplier": 2.0},
                {"frame": 40, "multiplier": 6.0}
            ],
            "daze_multiplier": 3.5,
            "hit_frames": [10, 25, 40],
            "invincible_frames": [0, 1, 2, 3, 4, 5],
            "interruptible_frame": 60,
            "is_snapshot": true,
            "charge_branches": [
                {"charge_duration": 30, "variant_action_id": "Ultimate_1_Charged"}
            ],
            "prerequisite_action_id": null,
            "hp_cost": 0.0,
            "energy_cost": 100.0,
            "decibel_cost": 3000.0,
            "cooldown_ticks": 120,
            "animation_frames": 90
        }"#;
        let skill: SkillData = serde_json::from_str(json).expect("deserialize");
        assert_eq!(skill.action_id, "Ultimate_1");
        assert_eq!(skill.action_type, SkillType::Ultimate);
        assert_eq!(skill.damage_multipliers.len(), 3);
        assert_eq!(skill.damage_multipliers[0].frame, 10);
        assert_eq!(skill.damage_multipliers[0].multiplier, 1.5);
        assert_eq!(skill.daze_multiplier, 3.5);
        assert_eq!(skill.hit_frames.len(), 3);
        assert_eq!(skill.invincible_frames.len(), 6);
        assert_eq!(skill.interruptible_frame, 60);
        assert!(skill.is_snapshot);
        assert_eq!(skill.charge_branches.len(), 1);
        assert_eq!(skill.charge_branches[0].charge_duration, 30);
        assert_eq!(
            skill.charge_branches[0].variant_action_id,
            "Ultimate_1_Charged"
        );
        assert!(skill.prerequisite_action_id.is_none());
        assert_eq!(skill.hp_cost, 0.0);
        assert_eq!(skill.energy_cost, 100.0);
        assert_eq!(skill.decibel_cost, 3000.0);
        assert_eq!(skill.cooldown_ticks, 120);
        assert_eq!(skill.animation_frames, 90);
    }

    #[test]
    fn test_serde_roundtrip() {
        let skill = SkillData {
            action_id: "Skill_Ex_1".into(),
            action_type: SkillType::Special,
            damage_multipliers: vec![HitFrame {
                frame: 5,
                multiplier: 2.5,
            }],
            daze_multiplier: 2.0,
            hit_frames: vec![5],
            invincible_frames: vec![3, 4],
            interruptible_frame: 30,
            is_snapshot: false,
            charge_branches: vec![],
            prerequisite_action_id: Some("Attack_Normal_3".into()),
            hp_cost: 0.0,
            energy_cost: 40.0,
            decibel_cost: 0.0,
            cooldown_ticks: 10,
            animation_frames: 40,
        };
        let json = serde_json::to_string(&skill).expect("serialize");
        let back: SkillData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.action_id, skill.action_id);
        assert_eq!(back.action_type, skill.action_type);
        assert_eq!(back.damage_multipliers.len(), 1);
        assert_eq!(back.prerequisite_action_id, Some("Attack_Normal_3".into()));
    }

    #[test]
    fn test_hit_frame_serde() {
        let json = r#"{"frame": 15, "multiplier": 3.2}"#;
        let hf: HitFrame = serde_json::from_str(json).expect("deserialize");
        assert_eq!(hf.frame, 15);
        assert_eq!(hf.multiplier, 3.2);
    }

    #[test]
    fn test_charge_branch_serde() {
        let json = r#"{"charge_duration": 45, "variant_action_id": "Skill_Ex_2_Hold"}"#;
        let cb: ChargeBranch = serde_json::from_str(json).expect("deserialize");
        assert_eq!(cb.charge_duration, 45);
        assert_eq!(cb.variant_action_id, "Skill_Ex_2_Hold");
    }
}
