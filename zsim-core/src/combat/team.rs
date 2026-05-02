use crate::entities::character::Character;

/// Manages a team of 1-3 characters and their battlefield state.
///
/// Minimal implementation supporting GameState (US-010). Full team
/// mechanics (switch, decibel, chain points) are added in US-011.
#[derive(Debug, Clone)]
pub struct TeamManager {
    pub characters: Vec<Character>,
}

impl TeamManager {
    pub fn new(characters: Vec<Character>) -> Self {
        Self { characters }
    }

    /// Returns true when all team members have HP ≤ 0.0.
    pub fn all_dead(&self) -> bool {
        self.characters.is_empty() || self.characters.iter().all(|c| c.current_stats.hp <= 0.0)
    }

    /// Count of team members with HP > 0.0.
    pub fn alive_count(&self) -> usize {
        self.characters
            .iter()
            .filter(|c| c.current_stats.hp > 0.0)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::enums::{ElementTag, FactionTag, SpecialtyTag};
    use crate::entities::models::BaseStats;

    fn make_character(hp: f64) -> Character {
        let mut c = Character::new(
            "test_char",
            FactionTag::GentleHouse,
            SpecialtyTag::Attack,
            ElementTag::Physical,
            BaseStats::default().with_hp(hp),
        );
        c.current_stats.hp = hp;
        c
    }

    #[test]
    fn test_new_team() {
        let chars = vec![make_character(8000.0), make_character(6000.0)];
        let team = TeamManager::new(chars);
        assert_eq!(team.characters.len(), 2);
    }

    #[test]
    fn test_all_dead_empty_team() {
        let team = TeamManager::new(vec![]);
        assert!(team.all_dead());
    }

    #[test]
    fn test_all_dead_some_alive() {
        let chars = vec![make_character(0.0), make_character(5000.0)];
        let team = TeamManager::new(chars);
        assert!(!team.all_dead());
    }

    #[test]
    fn test_all_dead_all_zero_hp() {
        let chars = vec![make_character(0.0), make_character(0.0)];
        let team = TeamManager::new(chars);
        assert!(team.all_dead());
    }

    #[test]
    fn test_alive_count() {
        let chars = vec![
            make_character(0.0),
            make_character(5000.0),
            make_character(3000.0),
        ];
        let team = TeamManager::new(chars);
        assert_eq!(team.alive_count(), 2);
    }

    #[test]
    fn test_alive_count_all_dead() {
        let chars = vec![make_character(0.0), make_character(0.0)];
        let team = TeamManager::new(chars);
        assert_eq!(team.alive_count(), 0);
    }
}
