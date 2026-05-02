//! Game state machine — holds tick, team, mode, and termination state.
//!
//! This is the central state holder for the simulation engine.  Each
//! tick the runner calls `advance_tick()` → execute phase → then
//! `check_termination()` to decide whether the simulation should stop.

use crate::combat::team::TeamManager;
use crate::entities::enemy::EnemyState;
use crate::entities::enums::SimMode;

/// Aggregate simulation state at a point in time.
#[derive(Debug, Clone)]
pub struct GameState {
    /// Current tick counter, starting at 0.
    pub current_tick: u64,
    /// The player's team (1–3 characters + optional bangboo).
    pub team: TeamManager,
    /// Single-shot or parallel execution mode.
    pub mode: SimMode,
    /// Whether the simulation has been marked as finished.
    pub is_terminated: bool,
    /// Human-readable reason for termination.
    pub termination_reason: Option<String>,
}

impl GameState {
    /// Create a new GameState at tick 0, not terminated.
    pub fn new(team: TeamManager, mode: SimMode) -> Self {
        Self {
            current_tick: 0,
            team,
            mode,
            is_terminated: false,
            termination_reason: None,
        }
    }

    /// Advance the tick counter by 1.
    pub fn advance_tick(&mut self) {
        self.current_tick += 1;
    }

    /// Mark the simulation as terminated with a descriptive reason.
    pub fn terminate(&mut self, reason: impl Into<String>) {
        self.is_terminated = true;
        self.termination_reason = Some(reason.into());
    }

    /// Check the four termination conditions and set `is_terminated` if met.
    ///
    /// Returns `true` when the simulation should stop.
    ///
    /// Termination conditions (checked in order):
    /// 1. All enemies have HP ≤ 0 (victory).
    /// 2. All characters have HP ≤ 0 (defeat).
    /// 3. `current_tick ≥ max_tick` (time limit).
    /// 4. APL action queue is exhausted (no more actions to execute).
    pub fn check_termination(
        &mut self,
        enemies: &[EnemyState],
        max_tick: u64,
        apl_exhausted: bool,
    ) -> bool {
        if self.is_terminated {
            return true;
        }

        // 1. All enemies defeated
        if enemies.iter().all(|e| e.hp <= 0.0) {
            self.terminate("All enemies defeated");
            return true;
        }

        // 2. All characters defeated
        if self.team.all_dead() {
            self.terminate("All characters defeated");
            return true;
        }

        // 3. Max tick reached
        if self.current_tick >= max_tick {
            self.terminate("Max tick reached");
            return true;
        }

        // 4. APL tracks exhausted
        if apl_exhausted {
            self.terminate("APL tracks exhausted");
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::character::Character;
    use crate::entities::enums::{ElementTag, EnemyType, FactionTag, SpecialtyTag};
    use crate::entities::models::BaseStats;

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

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

    fn make_enemy(hp: f64) -> EnemyState {
        let mut e = EnemyState::new("test_enemy", EnemyType::Normal, 100.0);
        e.hp = hp;
        e
    }

    fn default_state() -> GameState {
        GameState::new(
            TeamManager::new(vec![make_character(8000.0), make_character(6000.0)]),
            SimMode::Single,
        )
    }

    // ------------------------------------------------------------------
    // creation & accessors
    // ------------------------------------------------------------------

    #[test]
    fn test_game_state_creation() {
        let state = default_state();
        assert_eq!(state.current_tick, 0);
        assert_eq!(state.team.characters.len(), 2);
        assert_eq!(state.mode, SimMode::Single);
        assert!(!state.is_terminated);
        assert!(state.termination_reason.is_none());
    }

    #[test]
    fn test_advance_tick() {
        let mut state = default_state();
        for _ in 0..5 {
            state.advance_tick();
        }
        assert_eq!(state.current_tick, 5);
    }

    #[test]
    fn test_advance_tick_increments_by_one() {
        let mut state = default_state();
        let prev = state.current_tick;
        state.advance_tick();
        assert_eq!(state.current_tick, prev + 1);
    }

    #[test]
    fn test_terminate() {
        let mut state = default_state();
        state.terminate("test reason");
        assert!(state.is_terminated);
        assert_eq!(state.termination_reason.as_deref(), Some("test reason"));
    }

    #[test]
    fn test_terminate_overwrites_reason() {
        let mut state = default_state();
        state.terminate("first");
        state.terminate("second");
        assert_eq!(state.termination_reason.as_deref(), Some("second"));
    }

    // ------------------------------------------------------------------
    // check_termination — victory
    // ------------------------------------------------------------------

    #[test]
    fn test_termination_all_enemies_dead() {
        let mut state = default_state();
        let enemies = vec![make_enemy(0.0), make_enemy(-10.0)];
        assert!(state.check_termination(&enemies, 18000, false));
        assert_eq!(
            state.termination_reason.as_deref(),
            Some("All enemies defeated")
        );
    }

    // ------------------------------------------------------------------
    // check_termination — defeat
    // ------------------------------------------------------------------

    #[test]
    fn test_termination_all_characters_dead() {
        let mut state = GameState::new(
            TeamManager::new(vec![make_character(0.0), make_character(0.0)]),
            SimMode::Single,
        );
        let enemies = vec![make_enemy(10000.0)];
        assert!(state.check_termination(&enemies, 18000, false));
        assert_eq!(
            state.termination_reason.as_deref(),
            Some("All characters defeated")
        );
    }

    // ------------------------------------------------------------------
    // check_termination — max tick
    // ------------------------------------------------------------------

    #[test]
    fn test_termination_max_tick() {
        let mut state = default_state();
        state.current_tick = 18000;
        let enemies = vec![make_enemy(10000.0)];
        assert!(state.check_termination(&enemies, 18000, false));
        assert_eq!(
            state.termination_reason.as_deref(),
            Some("Max tick reached")
        );
    }

    #[test]
    fn test_termination_at_tick_zero_with_zero_max() {
        let mut state = default_state();
        // A sim with max_tick = 0 terminates immediately
        let enemies = vec![make_enemy(10000.0)];
        assert!(state.check_termination(&enemies, 0, false));
        assert_eq!(
            state.termination_reason.as_deref(),
            Some("Max tick reached")
        );
    }

    // ------------------------------------------------------------------
    // check_termination — APL exhausted
    // ------------------------------------------------------------------

    #[test]
    fn test_termination_apl_exhausted() {
        let mut state = default_state();
        let enemies = vec![make_enemy(10000.0)];
        assert!(state.check_termination(&enemies, 18000, true));
        assert_eq!(
            state.termination_reason.as_deref(),
            Some("APL tracks exhausted")
        );
    }

    // ------------------------------------------------------------------
    // check_termination — not terminated
    // ------------------------------------------------------------------

    #[test]
    fn test_no_termination_conditions_met() {
        let mut state = default_state();
        let enemies = vec![make_enemy(10000.0)];
        assert!(!state.check_termination(&enemies, 18000, false));
        assert!(!state.is_terminated);
    }

    // ------------------------------------------------------------------
    // check_termination — already terminated short-circuits
    // ------------------------------------------------------------------

    #[test]
    fn test_already_terminated_returns_true() {
        let mut state = default_state();
        state.terminate("manual stop");
        // Even though enemies are alive and tick < max, it should
        // short-circuit to true because is_terminated is already set.
        let enemies = vec![make_enemy(10000.0)];
        assert!(state.check_termination(&enemies, 18000, false));
        assert_eq!(state.termination_reason.as_deref(), Some("manual stop"));
    }

    // ------------------------------------------------------------------
    // check_termination — priority (enemies defeated before characters)
    // ------------------------------------------------------------------

    #[test]
    fn test_enemies_defeated_wins_over_apl_exhausted() {
        let mut state = default_state();
        let enemies = vec![make_enemy(0.0)];
        // Both enemies dead AND apl_exhausted — enemy death should win
        assert!(state.check_termination(&enemies, 18000, true));
        assert_eq!(
            state.termination_reason.as_deref(),
            Some("All enemies defeated")
        );
    }
}
