use crate::entities::character::Character;
use crate::entities::enums::CharacterState;

/// Default switch cooldown in ticks (0.5s @ 60 fps ≈ 30 ticks).
const SWITCH_COOLDOWN_TICKS: u64 = 30;

/// Maximum decibel (ult energy) per character.
const MAX_DECIBEL: f64 = 3000.0;

/// Manages a team of 1–3 characters, a bangboo, on-field state,
/// switch cooldown, decibel (ult energy), and chain points.
#[derive(Debug, Clone)]
pub struct TeamManager {
    pub characters: Vec<Character>,
    pub bangboo: Option<Character>,
    pub current_on_field_index: usize,
    pub switch_cooldown_remaining: u64,
}

impl TeamManager {
    /// Create a new team with the given characters and optional bangboo.
    ///
    /// The first character (index 0) starts on the field in Active state;
    /// all others start in Standby.
    pub fn new(characters: Vec<Character>) -> Self {
        Self::with_bangboo(characters, None)
    }

    /// Create a new team with characters and an optional bangboo.
    pub fn with_bangboo(mut characters: Vec<Character>, bangboo: Option<Character>) -> Self {
        // First character starts active, others standby
        if let Some(first) = characters.first_mut() {
            first.state = CharacterState::Active;
        }
        for c in characters.iter_mut().skip(1) {
            c.state = CharacterState::Standby;
        }
        Self {
            characters,
            bangboo,
            current_on_field_index: 0,
            switch_cooldown_remaining: 0,
        }
    }

    // ------------------------------------------------------------------
    // Query methods
    // ------------------------------------------------------------------

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

    /// Get the index of the current on-field character.
    pub fn on_field_index(&self) -> usize {
        self.current_on_field_index
    }

    /// Get the remaining switch cooldown in ticks.
    pub fn switch_cooldown(&self) -> u64 {
        self.switch_cooldown_remaining
    }

    /// Whether a switch is currently on cooldown.
    pub fn is_switch_on_cooldown(&self) -> bool {
        self.switch_cooldown_remaining > 0
    }

    // ------------------------------------------------------------------
    // On-field / off-field accessors
    // ------------------------------------------------------------------

    /// Immutable reference to the current on-field character.
    pub fn get_on_field(&self) -> &Character {
        &self.characters[self.current_on_field_index]
    }

    /// Mutable reference to the current on-field character.
    pub fn get_on_field_mut(&mut self) -> &mut Character {
        &mut self.characters[self.current_on_field_index]
    }

    /// Immutable reference to an off-field character by their index in the team.
    /// Returns `None` if the index is out of bounds or is the on-field character.
    pub fn get_off_field(&self, index: usize) -> Option<&Character> {
        if index == self.current_on_field_index || index >= self.characters.len() {
            return None;
        }
        Some(&self.characters[index])
    }

    /// Mutable reference to an off-field character by their index in the team.
    /// Returns `None` if the index is out of bounds or is the on-field character.
    pub fn get_off_field_mut(&mut self, index: usize) -> Option<&mut Character> {
        if index == self.current_on_field_index || index >= self.characters.len() {
            return None;
        }
        Some(&mut self.characters[index])
    }

    // ------------------------------------------------------------------
    // Character switching
    // ------------------------------------------------------------------

    /// Switch the active character to `index`.
    ///
    /// The current on-field character changes to `Standby`, the target
    /// changes to `Active`, and a switch cooldown is set.
    ///
    /// Returns `Ok(())` on success, or `Err` if the index is invalid or
    /// the switch is on cooldown.
    pub fn switch_to(&mut self, index: usize) -> Result<(), SwitchError> {
        if index >= self.characters.len() {
            return Err(SwitchError::InvalidIndex(index));
        }
        if index == self.current_on_field_index {
            return Err(SwitchError::AlreadyOnField);
        }
        if self.switch_cooldown_remaining > 0 {
            return Err(SwitchError::OnCooldown(self.switch_cooldown_remaining));
        }
        // Ensure target character is alive
        if self.characters[index].current_stats.hp <= 0.0 {
            return Err(SwitchError::TargetDead(index));
        }

        // Deactivate current, activate target
        self.characters[self.current_on_field_index].state = CharacterState::Standby;
        self.characters[index].state = CharacterState::Active;
        self.current_on_field_index = index;
        self.switch_cooldown_remaining = SWITCH_COOLDOWN_TICKS;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Decibel (ult energy) management
    // ------------------------------------------------------------------

    /// Add decibel to all characters (but not the bangboo).
    /// Each character's decibel is capped at MAX_DECIBEL (3000).
    pub fn add_decibel(&mut self, amount: f64) {
        for c in &mut self.characters {
            c.resources.decibel = (c.resources.decibel + amount).min(MAX_DECIBEL);
        }
    }

    /// Consume decibel from the on-field character.
    /// Returns `Ok(())` if sufficient, `Err` with the current amount otherwise.
    pub fn consume_decibel(&mut self, amount: f64) -> Result<(), InsufficientResource> {
        let c = &mut self.characters[self.current_on_field_index];
        if c.resources.decibel < amount {
            return Err(InsufficientResource {
                current: c.resources.decibel,
                required: amount,
            });
        }
        c.resources.decibel -= amount;
        Ok(())
    }

    /// Get the decibel value of a specific character by index.
    pub fn decibel_of(&self, index: usize) -> Option<f64> {
        self.characters.get(index).map(|c| c.resources.decibel)
    }

    // ------------------------------------------------------------------
    // Chain point management
    // ------------------------------------------------------------------

    /// Add one chain point to the on-field character (capped at u32::MAX).
    pub fn add_chain_point(&mut self) {
        let c = &mut self.characters[self.current_on_field_index];
        c.resources.chain_points = c.resources.chain_points.saturating_add(1);
    }

    /// Consume one chain point from the on-field character.
    /// Returns `Ok(())` if at least 1 point available, `Err` otherwise.
    pub fn consume_chain_point(&mut self) -> Result<(), InsufficientResource> {
        let c = &mut self.characters[self.current_on_field_index];
        if c.resources.chain_points < 1 {
            return Err(InsufficientResource {
                current: c.resources.chain_points as f64,
                required: 1.0,
            });
        }
        c.resources.chain_points -= 1;
        Ok(())
    }

    /// Number of chain points for the on-field character.
    pub fn chain_points(&self) -> u32 {
        self.characters[self.current_on_field_index]
            .resources
            .chain_points
    }

    // ------------------------------------------------------------------
    // Tick progression
    // ------------------------------------------------------------------

    /// Advance one tick: decrement switch cooldown (floor at 0).
    pub fn on_tick(&mut self) {
        self.switch_cooldown_remaining = self.switch_cooldown_remaining.saturating_sub(1);
    }
}

// ------------------------------------------------------------------
// Error types
// ------------------------------------------------------------------

/// Errors that can occur during character switching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchError {
    InvalidIndex(usize),
    AlreadyOnField,
    OnCooldown(u64),
    TargetDead(usize),
}

impl std::fmt::Display for SwitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchError::InvalidIndex(idx) => write!(f, "invalid character index: {idx}"),
            SwitchError::AlreadyOnField => write!(f, "character is already on the field"),
            SwitchError::OnCooldown(remaining) => {
                write!(f, "switch on cooldown for {remaining} more ticks")
            }
            SwitchError::TargetDead(idx) => write!(f, "target character at index {idx} is dead"),
        }
    }
}

impl std::error::Error for SwitchError {}

/// Insufficient resource (decibel or chain point) error.
#[derive(Debug, Clone, PartialEq)]
pub struct InsufficientResource {
    pub current: f64,
    pub required: f64,
}

impl std::fmt::Display for InsufficientResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "insufficient resource: {:.1} / {:.1}",
            self.current, self.required
        )
    }
}

impl std::error::Error for InsufficientResource {}

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

    fn make_bangboo() -> Character {
        let mut c = Character::new(
            "bangboo_01",
            FactionTag::GentleHouse,
            SpecialtyTag::Support,
            ElementTag::Physical,
            BaseStats::default(),
        );
        c.char_id = "bangboo_01".into();
        c
    }

    fn three_character_team() -> TeamManager {
        TeamManager::new(vec![
            make_character(8000.0),
            make_character(6000.0),
            make_character(5000.0),
        ])
    }

    // ------------------------------------------------------------------
    // Creation
    // ------------------------------------------------------------------

    #[test]
    fn test_new_team() {
        let chars = vec![make_character(8000.0), make_character(6000.0)];
        let team = TeamManager::new(chars);
        assert_eq!(team.characters.len(), 2);
        assert!(team.bangboo.is_none());
        assert_eq!(team.current_on_field_index, 0);
    }

    #[test]
    fn test_with_bangboo() {
        let chars = vec![make_character(8000.0)];
        let bangboo = make_bangboo();
        let team = TeamManager::with_bangboo(chars, Some(bangboo));
        assert_eq!(team.characters.len(), 1);
        assert!(team.bangboo.is_some());
        assert_eq!(team.bangboo.as_ref().unwrap().char_id, "bangboo_01");
    }

    #[test]
    fn test_first_character_starts_active() {
        let team = three_character_team();
        assert_eq!(team.characters[0].state, CharacterState::Active);
        assert_eq!(team.characters[1].state, CharacterState::Standby);
        assert_eq!(team.characters[2].state, CharacterState::Standby);
    }

    // ------------------------------------------------------------------
    // all_dead / alive_count (backward compatible)
    // ------------------------------------------------------------------

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

    // ------------------------------------------------------------------
    // Query methods
    // ------------------------------------------------------------------

    #[test]
    fn test_on_field_index() {
        let mut team = three_character_team();
        assert_eq!(team.on_field_index(), 0);
        team.current_on_field_index = 2;
        assert_eq!(team.on_field_index(), 2);
    }

    #[test]
    fn test_switch_cooldown_initially_zero() {
        let team = three_character_team();
        assert_eq!(team.switch_cooldown(), 0);
        assert!(!team.is_switch_on_cooldown());
    }

    // ------------------------------------------------------------------
    // get_on_field / get_off_field
    // ------------------------------------------------------------------

    #[test]
    fn test_get_on_field() {
        let team = three_character_team();
        assert_eq!(team.get_on_field().char_id, "test_char");
    }

    #[test]
    fn test_get_on_field_mut() {
        let mut team = three_character_team();
        let c = team.get_on_field_mut();
        c.resources.energy = 50.0;
        assert_eq!(team.characters[0].resources.energy, 50.0);
    }

    #[test]
    fn test_get_off_field_valid() {
        let team = three_character_team();
        let off = team.get_off_field(1);
        assert!(off.is_some());
        assert_eq!(off.unwrap().char_id, "test_char");
    }

    #[test]
    fn test_get_off_field_on_field_returns_none() {
        let team = three_character_team();
        assert!(team.get_off_field(0).is_none());
    }

    #[test]
    fn test_get_off_field_out_of_bounds() {
        let team = three_character_team();
        assert!(team.get_off_field(5).is_none());
    }

    #[test]
    fn test_get_off_field_mut_valid() {
        let mut team = three_character_team();
        let off = team.get_off_field_mut(1);
        assert!(off.is_some());
        off.unwrap().resources.energy = 99.0;
        assert_eq!(team.characters[1].resources.energy, 99.0);
    }

    #[test]
    fn test_get_off_field_mut_on_field_returns_none() {
        let mut team = three_character_team();
        assert!(team.get_off_field_mut(0).is_none());
    }

    // ------------------------------------------------------------------
    // switch_to
    // ------------------------------------------------------------------

    #[test]
    fn test_switch_to_basic() {
        let mut team = three_character_team();
        assert!(team.switch_to(1).is_ok());
        assert_eq!(team.current_on_field_index, 1);
        assert_eq!(team.characters[0].state, CharacterState::Standby);
        assert_eq!(team.characters[1].state, CharacterState::Active);
    }

    #[test]
    fn test_switch_to_sets_cooldown() {
        let mut team = three_character_team();
        assert!(team.switch_to(2).is_ok());
        assert!(team.switch_cooldown_remaining > 0);
        assert!(team.is_switch_on_cooldown());
    }

    #[test]
    fn test_switch_to_invalid_index() {
        let mut team = three_character_team();
        let err = team.switch_to(10).unwrap_err();
        assert_eq!(err, SwitchError::InvalidIndex(10));
    }

    #[test]
    fn test_switch_to_already_on_field() {
        let mut team = three_character_team();
        let err = team.switch_to(0).unwrap_err();
        assert_eq!(err, SwitchError::AlreadyOnField);
    }

    #[test]
    fn test_switch_to_while_on_cooldown() {
        let mut team = three_character_team();
        team.switch_cooldown_remaining = 15;
        let err = team.switch_to(1).unwrap_err();
        assert_eq!(err, SwitchError::OnCooldown(15));
    }

    #[test]
    fn test_switch_to_dead_character() {
        let mut team = three_character_team();
        team.characters[1].current_stats.hp = 0.0;
        let err = team.switch_to(1).unwrap_err();
        assert_eq!(err, SwitchError::TargetDead(1));
    }

    #[test]
    fn test_switch_to_single_character_team_fails() {
        let mut team = TeamManager::new(vec![make_character(8000.0)]);
        // Only index 0 exists, switching to it is AlreadyOnField
        let err = team.switch_to(0).unwrap_err();
        assert_eq!(err, SwitchError::AlreadyOnField);
    }

    // ------------------------------------------------------------------
    // Decibel management
    // ------------------------------------------------------------------

    #[test]
    fn test_add_decibel_to_all() {
        let mut team = three_character_team();
        team.add_decibel(500.0);
        assert_eq!(team.characters[0].resources.decibel, 500.0);
        assert_eq!(team.characters[1].resources.decibel, 500.0);
        assert_eq!(team.characters[2].resources.decibel, 500.0);
    }

    #[test]
    fn test_add_decibel_stacks() {
        let mut team = three_character_team();
        team.add_decibel(200.0);
        team.add_decibel(300.0);
        assert_eq!(team.characters[0].resources.decibel, 500.0);
    }

    #[test]
    fn test_add_decibel_caps_at_max() {
        let mut team = three_character_team();
        team.add_decibel(5000.0);
        assert_eq!(team.characters[0].resources.decibel, MAX_DECIBEL);
        assert_eq!(team.characters[1].resources.decibel, MAX_DECIBEL);
        assert_eq!(team.characters[2].resources.decibel, MAX_DECIBEL);
    }

    #[test]
    fn test_add_decibel_negative_is_noop() {
        let mut team = three_character_team();
        team.characters[0].resources.decibel = 100.0;
        team.add_decibel(-50.0);
        // -50 would underflow towards... let's use max(0, ...) to keep it clean
        // With .min(MAX_DECIBEL), a negative addition works as expected.
        assert_eq!(
            team.characters[0].resources.decibel,
            50.0_f64.max(0.0).min(MAX_DECIBEL)
        );
    }

    #[test]
    fn test_consume_decibel_success() {
        let mut team = three_character_team();
        team.characters[0].resources.decibel = 2000.0;
        assert!(team.consume_decibel(1500.0).is_ok());
        assert_eq!(team.characters[0].resources.decibel, 500.0);
    }

    #[test]
    fn test_consume_decibel_insufficient() {
        let mut team = three_character_team();
        team.characters[0].resources.decibel = 100.0;
        let err = team.consume_decibel(200.0).unwrap_err();
        assert!((err.current - 100.0).abs() < 1e-9);
        assert!((err.required - 200.0).abs() < 1e-9);
        // No change to decibel on failure
        assert_eq!(team.characters[0].resources.decibel, 100.0);
    }

    #[test]
    fn test_decibel_of() {
        let mut team = three_character_team();
        team.characters[1].resources.decibel = 2500.0;
        assert!((team.decibel_of(1).unwrap() - 2500.0).abs() < 1e-9);
    }

    #[test]
    fn test_decibel_of_out_of_bounds() {
        let team = three_character_team();
        assert!(team.decibel_of(10).is_none());
    }

    #[test]
    fn test_consume_decibel_on_off_field_character() {
        let mut team = three_character_team();
        // Switch to character at index 1, then consume from on-field (now index 1)
        team.switch_to(1).unwrap();
        team.characters[1].resources.decibel = 800.0;
        assert!(team.consume_decibel(300.0).is_ok());
        assert_eq!(team.characters[1].resources.decibel, 500.0);
        // Character at index 0 should be unchanged
        assert_eq!(team.characters[0].resources.decibel, 0.0);
    }

    // ------------------------------------------------------------------
    // Chain point management
    // ------------------------------------------------------------------

    #[test]
    fn test_add_chain_point() {
        let mut team = three_character_team();
        team.add_chain_point();
        assert_eq!(team.characters[0].resources.chain_points, 1);
    }

    #[test]
    fn test_add_chain_point_stacks() {
        let mut team = three_character_team();
        team.add_chain_point();
        team.add_chain_point();
        team.add_chain_point();
        assert_eq!(team.characters[0].resources.chain_points, 3);
    }

    #[test]
    fn test_add_chain_point_to_on_field_after_switch() {
        let mut team = three_character_team();
        team.switch_to(1).unwrap();
        team.add_chain_point();
        assert_eq!(team.characters[1].resources.chain_points, 1);
        assert_eq!(team.characters[0].resources.chain_points, 0);
    }

    #[test]
    fn test_consume_chain_point_success() {
        let mut team = three_character_team();
        team.characters[0].resources.chain_points = 2;
        assert!(team.consume_chain_point().is_ok());
        assert_eq!(team.characters[0].resources.chain_points, 1);
    }

    #[test]
    fn test_consume_chain_point_insufficient() {
        let mut team = three_character_team();
        let err = team.consume_chain_point().unwrap_err();
        assert!((err.current - 0.0).abs() < 1e-9);
        assert!((err.required - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_chain_points_query() {
        let mut team = three_character_team();
        team.characters[0].resources.chain_points = 3;
        assert_eq!(team.chain_points(), 3);
    }

    // ------------------------------------------------------------------
    // on_tick
    // ------------------------------------------------------------------

    #[test]
    fn test_on_tick_decrements_switch_cooldown() {
        let mut team = three_character_team();
        team.switch_to(1).unwrap();
        let before = team.switch_cooldown_remaining;
        team.on_tick();
        assert_eq!(team.switch_cooldown_remaining, before - 1);
    }

    #[test]
    fn test_on_tick_does_not_underflow() {
        let mut team = three_character_team();
        team.switch_cooldown_remaining = 0;
        team.on_tick();
        assert_eq!(team.switch_cooldown_remaining, 0);
    }

    #[test]
    fn test_switch_cooldown_expires_after_enough_ticks() {
        let mut team = three_character_team();
        team.switch_to(1).unwrap();
        let cooldown = team.switch_cooldown_remaining;
        for _ in 0..cooldown {
            assert!(team.is_switch_on_cooldown());
            team.on_tick();
        }
        assert!(!team.is_switch_on_cooldown());
        // Should be able to switch again
        assert!(team.switch_to(2).is_ok());
    }

    // ------------------------------------------------------------------
    // Full integration: switch → decibel → chain → tick → switch back
    // ------------------------------------------------------------------

    #[test]
    fn test_full_rotation() {
        let mut team = three_character_team();
        // Start: character 0 is on field
        assert_eq!(team.on_field_index(), 0);

        // Add resources to all
        team.add_decibel(500.0);
        assert_eq!(team.characters[0].resources.decibel, 500.0);

        // Switch to character 1
        assert!(team.switch_to(1).is_ok());
        assert_eq!(team.on_field_index(), 1);

        // Add chain point to character 1 (on field)
        team.add_chain_point();
        assert_eq!(team.chain_points(), 1);

        // Character 1 adds decibel and consumes it
        team.add_decibel(2000.0);
        assert_eq!(team.characters[1].resources.decibel, 2500.0);
        assert!(team.consume_decibel(2400.0).is_ok());
        assert_eq!(team.characters[1].resources.decibel, 100.0);

        // Consume chain point
        assert!(team.consume_chain_point().is_ok());
        assert_eq!(team.chain_points(), 0);

        // Wait out cooldown
        let cooldown = team.switch_cooldown_remaining;
        for _ in 0..=cooldown {
            team.on_tick();
        }

        // Switch back to character 0
        assert!(team.switch_to(0).is_ok());
        assert_eq!(team.on_field_index(), 0);
        assert_eq!(team.characters[0].state, CharacterState::Active);
        assert_eq!(team.characters[1].state, CharacterState::Standby);
    }
}
