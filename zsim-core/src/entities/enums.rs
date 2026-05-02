use serde::{Deserialize, Serialize};

/// Faction tag identifying a character's affiliation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FactionTag {
    #[serde(rename = "Gentle_House")]
    GentleHouse,
    #[serde(rename = "Criminal_Investigation")]
    CriminalInvestigation,
    #[serde(rename = "H.S.O.S.6")]
    Hsos6,
    #[serde(rename = "Belobog_Heavy_Ind")]
    BelobogHeavyInd,
    #[serde(rename = "Victoria_Housekeeping")]
    VictoriaHousekeeping,
    #[serde(rename = "Sons_of_Calydon")]
    SonsOfCalydon,
    #[serde(rename = "Lyra")]
    Lyra,
    #[serde(rename = "Obolus_Squad")]
    ObolusSquad,
    #[serde(rename = "Defense_Force_Silver")]
    DefenseForceSilver,
    #[serde(rename = "Mockingbird")]
    Mockingbird,
    #[serde(rename = "Yun_Kui_Mountain")]
    YunKuiMountain,
    #[serde(rename = "Odd_Eater_House")]
    OddEaterHouse,
    #[serde(rename = "Krampus_Dark_Branch")]
    KrampusDarkBranch,
    #[serde(rename = "Delusional_Angel")]
    DelusionalAngel,
    #[serde(rename = "City_Order_Dept")]
    CityOrderDept,
    #[serde(rename = "Other")]
    Other,
}

/// Specialty tag identifying a character's combat role.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpecialtyTag {
    #[serde(rename = "Attack")]
    Attack,
    #[serde(rename = "Stun")]
    Stun,
    #[serde(rename = "Support")]
    Support,
    #[serde(rename = "Rupture")]
    Rupture,
    #[serde(rename = "Anomaly")]
    Anomaly,
    #[serde(rename = "Defense")]
    Defense,
}

/// Element tag for damage typing and anomaly interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElementTag {
    #[serde(rename = "Ice")]
    Ice,
    #[serde(rename = "Fire")]
    Fire,
    #[serde(rename = "Ether")]
    Ether,
    #[serde(rename = "Physical")]
    Physical,
    #[serde(rename = "Electric")]
    Electric,
    #[serde(rename = "Auric Ink")]
    AuricInk,
    #[serde(rename = "Frost")]
    Frost,
    #[serde(rename = "Honed Edge")]
    HonedEdge,
}

/// Character field presence state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CharacterState {
    #[serde(rename = "Active")]
    Active,
    #[serde(rename = "Standby")]
    #[default]
    Standby,
    #[serde(rename = "Locked")]
    Locked,
}

/// Enemy tier classification for chain-attack limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EnemyType {
    #[serde(rename = "Normal")]
    #[default]
    Normal,
    #[serde(rename = "Elite")]
    Elite,
    #[serde(rename = "Boss")]
    Boss,
}

impl EnemyType {
    pub fn chain_attack_limit(&self) -> u32 {
        match self {
            EnemyType::Normal => 1,
            EnemyType::Elite => 2,
            EnemyType::Boss => 3,
        }
    }
}

/// Categories of skill actions in a character's kit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillType {
    #[serde(rename = "Normal")]
    Normal,
    #[serde(rename = "Special")]
    Special,
    #[serde(rename = "Ultimate")]
    Ultimate,
    #[serde(rename = "Chain")]
    Chain,
    #[serde(rename = "Dodge")]
    Dodge,
    #[serde(rename = "Assist")]
    Assist,
    #[serde(rename = "Coordinated")]
    Coordinated,
    #[serde(rename = "QuickAssist")]
    QuickAssist,
}

/// Trigger conditions for coordinated attacks and reactive skills.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerType {
    #[serde(rename = "OnDamageDealt")]
    OnDamageDealt,
    #[serde(rename = "OnAnomalyTriggered")]
    OnAnomalyTriggered,
    #[serde(rename = "OnChainAttack")]
    OnChainAttack,
    #[serde(rename = "OnDodge")]
    OnDodge,
    #[serde(rename = "OnParry")]
    OnParry,
}

/// Simulation execution mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimMode {
    #[serde(rename = "Single")]
    Single,
    #[serde(rename = "Parallel")]
    Parallel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_faction_serde_roundtrip() {
        let factions = vec![
            FactionTag::GentleHouse,
            FactionTag::CriminalInvestigation,
            FactionTag::Other,
        ];
        for f in factions {
            let json = serde_json::to_string(&f).expect("serialize");
            let back: FactionTag = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(f, back);
        }
    }

    #[test]
    fn test_faction_deserialize_from_python() {
        let json = r#""Gentle_House""#;
        let f: FactionTag = serde_json::from_str(json).expect("deserialize");
        assert_eq!(f, FactionTag::GentleHouse);
    }

    #[test]
    fn test_specialty_serde() {
        let json = r#""Attack""#;
        let s: SpecialtyTag = serde_json::from_str(json).expect("deserialize");
        assert_eq!(s, SpecialtyTag::Attack);
    }

    #[test]
    fn test_element_serde() {
        let json = r#""Fire""#;
        let e: ElementTag = serde_json::from_str(json).expect("deserialize");
        assert_eq!(e, ElementTag::Fire);
    }

    #[test]
    fn test_character_state_serde() {
        let json = r#""Active""#;
        let s: CharacterState = serde_json::from_str(json).expect("deserialize");
        assert_eq!(s, CharacterState::Active);
    }

    #[test]
    fn test_enemy_type_chain_limits() {
        assert_eq!(EnemyType::Normal.chain_attack_limit(), 1);
        assert_eq!(EnemyType::Elite.chain_attack_limit(), 2);
        assert_eq!(EnemyType::Boss.chain_attack_limit(), 3);
    }

    #[test]
    fn test_enemy_type_serde() {
        let json = r#""Boss""#;
        let et: EnemyType = serde_json::from_str(json).expect("deserialize");
        assert_eq!(et, EnemyType::Boss);
    }

    #[test]
    fn test_sim_mode_serde() {
        let json = r#""Parallel""#;
        let m: SimMode = serde_json::from_str(json).expect("deserialize");
        assert_eq!(m, SimMode::Parallel);
    }
}
