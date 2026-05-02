use crate::entities::enums::SimMode;

/// Global game state tracking tick count, termination conditions,
/// and a reference to the team (to be fully integrated in US-011).
pub struct GameState {
    pub current_tick: u64,
    pub mode: SimMode,
    pub is_terminated: bool,
    pub termination_reason: Option<String>,

    /// Maximum tick before automatic termination (default 18000).
    pub max_tick: u64,
    /// Whether the APL has exhausted all track actions.
    pub apl_exhausted: bool,
    /// Whether all enemies are dead.
    pub all_enemies_dead: bool,
    /// Whether all characters are dead.
    pub all_characters_dead: bool,
}

impl GameState {
    /// Create a new game state.
    pub fn new(mode: SimMode) -> Self {
        GameState {
            current_tick: 0,
            mode,
            is_terminated: false,
            termination_reason: None,
            max_tick: 18000,
            apl_exhausted: false,
            all_enemies_dead: false,
            all_characters_dead: false,
        }
    }

    /// Advance the simulation by one tick.
    pub fn advance_tick(&mut self) {
        self.current_tick += 1;
    }

    /// Check all termination conditions.
    ///
    /// Returns `true` if the simulation should end.  The specific
    /// reason is stored in `self.termination_reason`.
    pub fn check_termination(&mut self) -> bool {
        if self.all_enemies_dead {
            self.terminate("all enemies dead");
        } else if self.all_characters_dead {
            self.terminate("all characters dead");
        } else if self.current_tick >= self.max_tick {
            self.terminate("max tick reached");
        } else if self.apl_exhausted {
            self.terminate("APL track actions exhausted");
        }

        self.is_terminated
    }

    /// Mark the simulation as terminated with a given reason.
    pub fn terminate(&mut self, reason: &str) {
        self.is_terminated = true;
        self.termination_reason = Some(reason.to_string());
    }

    /// Reset termination state (useful for re-running a simulation).
    pub fn reset_termination(&mut self) {
        self.is_terminated = false;
        self.termination_reason = None;
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new(SimMode::Single)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_game_state() {
        let state = GameState::new(SimMode::Parallel);
        assert_eq!(state.current_tick, 0);
        assert_eq!(state.mode, SimMode::Parallel);
        assert!(!state.is_terminated);
        assert!(state.termination_reason.is_none());
        assert_eq!(state.max_tick, 18000);
    }

    #[test]
    fn test_advance_tick() {
        let mut state = GameState::default();
        state.advance_tick();
        assert_eq!(state.current_tick, 1);
        state.advance_tick();
        state.advance_tick();
        assert_eq!(state.current_tick, 3);
    }

    #[test]
    fn test_terminate() {
        let mut state = GameState::default();
        state.terminate("test reason");
        assert!(state.is_terminated);
        assert_eq!(state.termination_reason, Some("test reason".into()));
    }

    #[test]
    fn test_check_termination_all_enemies_dead() {
        let mut state = GameState::default();
        state.all_enemies_dead = true;
        assert!(state.check_termination());
        assert!(state.is_terminated);
        assert_eq!(state.termination_reason, Some("all enemies dead".into()));
    }

    #[test]
    fn test_check_termination_all_characters_dead() {
        let mut state = GameState::default();
        state.all_characters_dead = true;
        assert!(state.check_termination());
        assert_eq!(state.termination_reason, Some("all characters dead".into()));
    }

    #[test]
    fn test_check_termination_max_tick() {
        let mut state = GameState::default();
        state.current_tick = 18000;
        assert!(state.check_termination());
        assert_eq!(state.termination_reason, Some("max tick reached".into()));
    }

    #[test]
    fn test_check_termination_apl_exhausted() {
        let mut state = GameState::default();
        state.apl_exhausted = true;
        assert!(state.check_termination());
        assert_eq!(
            state.termination_reason,
            Some("APL track actions exhausted".into())
        );
    }

    #[test]
    fn test_check_termination_no_condition() {
        let mut state = GameState::default();
        state.current_tick = 100;
        assert!(!state.check_termination());
        assert!(!state.is_terminated);
    }

    #[test]
    fn test_priority_all_enemies_over_characters() {
        let mut state = GameState::default();
        state.all_enemies_dead = true;
        state.all_characters_dead = true;
        state.check_termination();
        assert_eq!(
            state.termination_reason,
            Some("all enemies dead".into()),
            "enemy death should take priority over character death"
        );
    }

    #[test]
    fn test_reset_termination() {
        let mut state = GameState::default();
        state.terminate("ended");
        assert!(state.is_terminated);

        state.reset_termination();
        assert!(!state.is_terminated);
        assert!(state.termination_reason.is_none());
    }

    #[test]
    fn test_check_termination_below_max_tick() {
        let mut state = GameState::default();
        state.current_tick = 17999;
        assert!(!state.check_termination());
        assert!(!state.is_terminated);
    }

    #[test]
    fn test_max_tick_custom() {
        let mut state = GameState::default();
        state.max_tick = 100;
        state.current_tick = 100;
        assert!(state.check_termination());
    }
}
