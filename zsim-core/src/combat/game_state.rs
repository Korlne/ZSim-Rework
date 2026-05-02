//! 游戏状态机 —— 持有关卡、队伍、模式和终止状态。
//!
//! 这是模拟引擎的核心状态持有者。每个 tick，
//! 运行器调用 `advance_tick()` → 执行阶段 →
//! 然后 `check_termination()` 判断模拟是否应该停止。

use crate::combat::team::TeamManager;
use crate::entities::enemy::EnemyState;
use crate::entities::enums::SimMode;

/// 某个时间点的聚合模拟状态。
#[derive(Debug, Clone)]
pub struct GameState {
    /// 当前 tick 计数器，从 0 开始。
    pub current_tick: u64,
    /// 玩家队伍（1–3 个角色 + 可选邦布）。
    pub team: TeamManager,
    /// 单次或并行执行模式。
    pub mode: SimMode,
    /// 模拟是否已被标记为完成。
    pub is_terminated: bool,
    /// 人类可读的终止原因。
    pub termination_reason: Option<String>,
}

impl GameState {
    /// 创建一个新的 GameState，tick 为 0，未终止。
    pub fn new(team: TeamManager, mode: SimMode) -> Self {
        Self {
            current_tick: 0,
            team,
            mode,
            is_terminated: false,
            termination_reason: None,
        }
    }

    /// 将 tick 计数器前进 1。
    pub fn advance_tick(&mut self) {
        self.current_tick += 1;
    }

    /// 使用描述性原因将模拟标记为已终止。
    pub fn terminate(&mut self, reason: impl Into<String>) {
        self.is_terminated = true;
        self.termination_reason = Some(reason.into());
    }

    /// 检查四个终止条件，如果满足则设置 `is_terminated`。
    ///
    /// 当模拟应停止时返回 `true`。
    ///
    /// 终止条件（按顺序检查）：
    /// 1. 所有敌人 HP ≤ 0（胜利）。
    /// 2. 所有角色 HP ≤ 0（失败）。
    /// 3. `current_tick ≥ max_tick`（时间限制）。
    /// 4. APL 动作队列已耗尽（没有更多动作可执行）。
    pub fn check_termination(
        &mut self,
        enemies: &[EnemyState],
        max_tick: u64,
        apl_exhausted: bool,
    ) -> bool {
        if self.is_terminated {
            return true;
        }

        // 1. 所有敌人被击败
        if enemies.iter().all(|e| e.hp <= 0.0) {
            self.terminate("All enemies defeated");
            return true;
        }

        // 2. 所有角色被击败
        if self.team.all_dead() {
            self.terminate("All characters defeated");
            return true;
        }

        // 3. 达到最大 tick
        if self.current_tick >= max_tick {
            self.terminate("Max tick reached");
            return true;
        }

        // 4. APL 轨道已耗尽
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
    // 辅助函数
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
    // 创建与访问器
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
    // check_termination — 胜利
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
    // check_termination — 失败
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
    // check_termination — 最大 tick
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
        // max_tick = 0 的模拟立即终止
        let enemies = vec![make_enemy(10000.0)];
        assert!(state.check_termination(&enemies, 0, false));
        assert_eq!(
            state.termination_reason.as_deref(),
            Some("Max tick reached")
        );
    }

    // ------------------------------------------------------------------
    // check_termination — APL 已耗尽
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
    // check_termination — 未终止
    // ------------------------------------------------------------------

    #[test]
    fn test_no_termination_conditions_met() {
        let mut state = default_state();
        let enemies = vec![make_enemy(10000.0)];
        assert!(!state.check_termination(&enemies, 18000, false));
        assert!(!state.is_terminated);
    }

    // ------------------------------------------------------------------
    // check_termination — 已终止则短路返回
    // ------------------------------------------------------------------

    #[test]
    fn test_already_terminated_returns_true() {
        let mut state = default_state();
        state.terminate("manual stop");
        // 尽管敌人还活着且 tick < max，但由于 is_terminated 已设置，
        // 应该短路返回 true。
        let enemies = vec![make_enemy(10000.0)];
        assert!(state.check_termination(&enemies, 18000, false));
        assert_eq!(state.termination_reason.as_deref(), Some("manual stop"));
    }

    // ------------------------------------------------------------------
    // check_termination — 优先级（敌人击败优先于角色）
    // ------------------------------------------------------------------

    #[test]
    fn test_enemies_defeated_wins_over_apl_exhausted() {
        let mut state = default_state();
        let enemies = vec![make_enemy(0.0)];
        // 敌人死亡 AND APL 耗尽 —— 敌人死亡应优先
        assert!(state.check_termination(&enemies, 18000, true));
        assert_eq!(
            state.termination_reason.as_deref(),
            Some("All enemies defeated")
        );
    }
}
