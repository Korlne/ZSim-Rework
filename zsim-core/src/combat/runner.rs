//! 模拟运行器 —— 驱动所有子系统的主事件循环，
//! 逐 tick 运行，直到满足终止条件。
//!
//! 每 tick 执行顺序：
//! 1. 发布 TickStart 事件
//! 2. 状态更新：buff 过期、异常 tick、切换冷却
//! 3. APL 处理 —— 推进动画、分派新动作
//! 4. 将 PendingActions 转换为 GameEvents，发布到 EventBus
//! 5. 协同动作处理 —— 响应 tick 事件
//! 6. 检查终止条件
//! 7. 推进 tick

use std::collections::HashMap;

use crate::calculation::anomaly::AnomalyDisorderManager;
use crate::calculation::buff::BuffManager;
use crate::combat::apl::{APLManager, PendingAction};
use crate::combat::coordinated::CoordinatedActionSystem;
use crate::combat::game_state::GameState;
use crate::combat::skill::SkillData;
use crate::combat::team::TeamManager;
use crate::data::apl::APLData;
use crate::entities::character::Character;
use crate::entities::enemy::EnemyState;
use crate::entities::enums::SimMode;
use crate::events::event_bus::EventBus;
use crate::events::signals::{EventType, GameEvent};

/// 单次模拟运行的配置。
#[derive(Debug, Clone)]
pub struct SimConfig {
    pub team_characters: Vec<Character>,
    pub enemies: Vec<EnemyState>,
    pub skills: HashMap<String, SkillData>,
    pub apl: APLData,
    pub max_tick: u64,
    pub seed: u64,
    pub mode: SimMode,
    pub bangboo: Option<Character>,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            team_characters: Vec::new(),
            enemies: Vec::new(),
            skills: HashMap::new(),
            apl: APLData { tracks: Vec::new() },
            max_tick: 18000,
            seed: 42,
            mode: SimMode::Single,
            bangboo: None,
        }
    }
}

/// 模拟中的单个记录事件，适合输出/日志记录。
///
/// 扩展了可选的有效载荷字段，映射到 `zsim-parquet` 用于列式存储和聚合的 15 列 Parquet 模式。
#[derive(Debug, Clone)]
pub struct LoggedEvent {
    pub tick: u64,
    pub event_type: EventType,
    pub source_id: Option<String>,
    pub target_id: Option<String>,
    pub action_id: Option<String>,
    pub damage: Option<f64>,
    pub is_crit: Option<bool>,
    // ── 扩展的 Parquet 有效载荷字段 ──────────────────────────────
    /// 事件涉及的元素（例如 "Fire"、"Electric"）。
    pub element: Option<String>,
    /// 累积或触发的异常条值。
    pub anomaly_gauge: Option<f64>,
    /// 造成的眩晕/失衡伤害。
    pub stun_dmg: Option<f64>,
    /// Buff 标识符（用于 BuffChanged 事件）。
    pub buff_id: Option<String>,
    /// Buff 值/大小。
    pub buff_value: Option<f64>,
    /// 此事件是否由协同动作生成。
    pub coordinated_flag: Option<bool>,
}

/// 单次模拟运行的结果。
#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub total_ticks: u64,
    pub termination_reason: Option<String>,
    pub events: Vec<LoggedEvent>,
    pub seed: u64,
}

/// 主模拟驱动器。
///
/// 创建并拥有单次运行的所有子系统。
pub struct SimulationRunner;

impl SimulationRunner {
    /// 使用给定的 [`SimConfig`] 执行模拟。
    ///
    /// tick 循环每次迭代遵循以下固定顺序：
    /// 1. 广播 `TickStart`
    /// 2. 状态更新（buff 过期、异常 tick、切换冷却）
    /// 3. `APLManager::process_next_action` —— 推进动画、分派新动作
    /// 4. 将待处理的 APL 动作转换为 `GameEvent`，发布到 `EventBus`
    /// 5. `CoordinatedActionSystem::process_events` —— 让监听器做出反应
    /// 6. `GameState::check_termination` —— 决定是否停止
    /// 7. `GameState::advance_tick`
    pub fn run(config: SimConfig) -> SimulationResult {
        // ── 初始化所有子系统 ──────────────────────────────────
        let team =
            TeamManager::with_bangboo(config.team_characters.clone(), config.bangboo.clone());
        let mut game_state = GameState::new(team, config.mode);
        let mut buff_mgr = BuffManager::new();
        let mut anomaly_mgr = AnomalyDisorderManager::new();
        let event_bus = EventBus::new();
        let mut apl_mgr = APLManager::new(config.apl.clone());
        let mut coordinated_system = CoordinatedActionSystem::new();
        let enemies = config.enemies.clone();

        let mut logged_events: Vec<LoggedEvent> = Vec::new();
        let mut processed_ticks: u64 = 0;

        // ── 主 tick 循环 ─────────────────────────────────────────────
        loop {
            let tick = game_state.current_tick;

            // 预检查静态终止条件（敌人、角色、max_tick）。
            // 这里传 `apl_exhausted = false` —— APL 耗尽仅在处理*之后*才有意义，
            // 因此在下面的后检查中处理。
            if game_state.check_termination(&enemies, config.max_tick, false) {
                break;
            }

            // 1. Tick 开始 — 广播事件
            event_bus.publish(GameEvent::new(EventType::TickStart, tick));

            // 2. 状态更新
            buff_mgr.on_tick(tick);
            buff_mgr.remove_expired(tick);
            anomaly_mgr.on_tick(tick);
            game_state.team.on_tick(); // 切换冷却递减

            // 3. APL 处理 — 推进动画并分派新动作
            let _validation_errors =
                apl_mgr.process_next_action(tick, &mut game_state.team, &config.skills, &enemies);

            // 4. 收集待处理动作，转换为 GameEvents，发布并记录
            let pending = apl_mgr.get_pending_actions();
            let mut tick_events: Vec<GameEvent> = Vec::new();

            for action in &pending {
                if let Some(ev) = pending_action_to_event(action, tick) {
                    event_bus.publish(ev.clone());
                    tick_events.push(ev.clone());

                    logged_events.push(LoggedEvent {
                        tick,
                        event_type: ev.event_type.clone(),
                        source_id: ev.source_id.clone(),
                        target_id: ev.target_id.clone(),
                        action_id: ev
                            .payload
                            .as_ref()
                            .and_then(|p| p.get("action_id"))
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        damage: ev
                            .payload
                            .as_ref()
                            .and_then(|p| p.get("damage"))
                            .and_then(|v| v.as_f64()),
                        is_crit: ev
                            .payload
                            .as_ref()
                            .and_then(|p| p.get("is_crit"))
                            .and_then(|v| v.as_bool()),
                        // 扩展的有效载荷字段 —— 不从当前 APL 事件中提取
                        element: None,
                        anomaly_gauge: None,
                        stun_dmg: None,
                        buff_id: None,
                        buff_value: None,
                        coordinated_flag: Some(false),
                    });
                }
            }

            // 5. 协同动作处理 — 响应 tick 事件
            coordinated_system.process_events(&tick_events, &game_state);
            let coordinated_actions = coordinated_system.get_pending_actions();

            for ca in &coordinated_actions {
                let ev = GameEvent::new(EventType::CoordinatedAction, tick)
                    .with_source(ca.source_id.clone())
                    .with_target(ca.target_id.clone())
                    .with_payload(serde_json::json!({"action_id": ca.action_id.clone()}));
                event_bus.publish(ev);
                logged_events.push(LoggedEvent {
                    tick,
                    event_type: EventType::CoordinatedAction,
                    source_id: Some(ca.source_id.clone()),
                    target_id: Some(ca.target_id.clone()),
                    action_id: Some(ca.action_id.clone()),
                    damage: None,
                    is_crit: None,
                    element: None,
                    anomaly_gauge: None,
                    stun_dmg: None,
                    buff_id: None,
                    buff_value: None,
                    coordinated_flag: Some(true),
                });
            }

            // 6. 后检查 — 仅检查 APL 耗尽（敌人/角色/max_tick
            //    已在循环顶部检查过）。
            // 空的 APL（无轨道）不被视为已耗尽 —
            // 默认情况下模拟会运行到 max_tick。只有当计划
            // 存在且所有轨道都已消耗完毕时才停止。
            processed_ticks += 1;
            let has_tracks = !config.apl.tracks.is_empty();
            if has_tracks && apl_mgr.all_tracks_exhausted() {
                game_state.terminate("APL tracks exhausted");
                break;
            }

            // 7. 前进到下一个 tick
            game_state.advance_tick();
        }

        SimulationResult {
            total_ticks: processed_ticks,
            termination_reason: game_state.termination_reason.clone(),
            events: logged_events,
            seed: config.seed,
        }
    }
}

/// 将 [`PendingAction`] 转换为可选的 [`GameEvent`]。
///
/// 只有自然映射到 [`EventType`] 变体的动作才会产生事件；
/// 内部记账动作（`ActionCompleted`、`ChargingCompleted`）会被过滤掉。
fn pending_action_to_event(action: &PendingAction, tick: u64) -> Option<GameEvent> {
    match action {
        PendingAction::ActionStarted { char_id, action_id } => Some(
            GameEvent::new(EventType::ActionStart, tick)
                .with_source(char_id.clone())
                .with_payload(serde_json::json!({"action_id": action_id})),
        ),
        PendingAction::HitFrameTriggered {
            char_id,
            action_id,
            frame,
            multiplier,
        } => Some(
            GameEvent::new(EventType::DamageDealt, tick)
                .with_source(char_id.clone())
                .with_payload(serde_json::json!({
                    "action_id": action_id,
                    "frame": frame,
                    "multiplier": multiplier,
                })),
        ),
        PendingAction::ActionFailed {
            char_id,
            action_id,
            reason,
        } => Some(
            GameEvent::new(EventType::ErrorRaised, tick)
                .with_source(char_id.clone())
                .with_payload(serde_json::json!({
                    "action_id": action_id,
                    "reason": reason,
                })),
        ),
        PendingAction::ChargingStarted {
            char_id,
            action_id,
            charge_duration,
        } => Some(
            GameEvent::new(EventType::ActionStart, tick)
                .with_source(char_id.clone())
                .with_payload(serde_json::json!({
                    "action_id": action_id,
                    "charge_duration": charge_duration,
                })),
        ),
        // 内部记账：没有匹配的事件类型
        PendingAction::ActionCompleted { .. } | PendingAction::ChargingCompleted { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::coordinated::CoordinatedListener;
    use crate::combat::skill::SkillData;
    use crate::data::apl::{ActionEntry, Track};
    use crate::entities::character::Character;
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
            animation_frames: 1, // default: instant completion
        }
    }

    fn make_enemy(hp: f64) -> EnemyState {
        let mut enemy = EnemyState::new("test_enemy", EnemyType::Elite, 100.0);
        enemy.hp = hp;
        enemy
    }

    fn make_skills_map(skills: Vec<SkillData>) -> HashMap<String, SkillData> {
        skills
            .into_iter()
            .map(|s| (s.action_id.clone(), s))
            .collect()
    }

    fn single_track_apl(char_id: &str, actions: Vec<ActionEntry>) -> APLData {
        APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: char_id.into(),
                actions,
            }],
        }
    }

    // ------------------------------------------------------------------
    // 默认配置与基本运行
    // ------------------------------------------------------------------

    #[test]
    fn test_default_config_no_crash() {
        // 空配置且没有敌人 → "All enemies defeated"（空真）。
        let cfg = SimConfig::default();
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 0);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("All enemies defeated")
        );
    }

    #[test]
    fn test_default_config_with_enemies() {
        // 敌人存活 + 无 APL → 运行到 max_tick
        let mut cfg = SimConfig::default();
        cfg.max_tick = 5;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 5);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("Max tick reached")
        );
    }

    // ------------------------------------------------------------------
    // 终止：最大 tick
    // ------------------------------------------------------------------

    #[test]
    fn test_termination_by_max_tick() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 10;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 10);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("Max tick reached")
        );
    }

    #[test]
    fn test_termination_by_max_tick_large_value() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 100;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 100);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("Max tick reached")
        );
    }

    // ------------------------------------------------------------------
    // 终止：APL 耗尽
    // ------------------------------------------------------------------

    #[test]
    fn test_termination_by_apl_exhausted() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 1000;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        cfg.skills = make_skills_map(vec![make_skill(
            "normal_atk",
            SkillType::Normal,
            0.0,
            0.0,
            0.0,
            0,
        )]);
        cfg.apl = single_track_apl(
            "char_0",
            vec![ActionEntry {
                action_id: "normal_atk".into(),
                at: 0,
            }],
        );
        let result = SimulationRunner::run(cfg);
        // Tick 0 处的一个动作 → 立即分派，APL 在
        // tick 1 耗尽 → 以 APL 耗尽终止
        assert_eq!(result.total_ticks, 1);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("APL tracks exhausted")
        );
    }

    #[test]
    fn test_termination_by_apl_exhausted_multi_track() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 1000;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        cfg.skills = make_skills_map(vec![make_skill(
            "normal_atk",
            SkillType::Normal,
            0.0,
            0.0,
            0.0,
            0,
        )]);
        cfg.apl = APLData {
            tracks: vec![
                Track {
                    track_id: "t1".into(),
                    char_id: "char_0".into(),
                    actions: vec![ActionEntry {
                        action_id: "normal_atk".into(),
                        at: 0,
                    }],
                },
                Track {
                    track_id: "t2".into(),
                    char_id: "char_1".into(),
                    actions: vec![ActionEntry {
                        action_id: "normal_atk".into(),
                        at: 0,
                    }],
                },
            ],
        };
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 1);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("APL tracks exhausted")
        );
    }

    // ------------------------------------------------------------------
    // 终止：敌人死亡
    // ------------------------------------------------------------------

    #[test]
    fn test_termination_by_enemy_death_zero_hp() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 1000;
        cfg.enemies = vec![make_enemy(0.0)]; // already dead
        cfg.team_characters = make_team().characters;
        // 无 APL → 应在 tick 0 处以 "All enemies defeated" 终止
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 0);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("All enemies defeated")
        );
    }

    #[test]
    fn test_termination_by_enemy_death_negative_hp() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 1000;
        cfg.enemies = vec![make_enemy(-100.0)];
        cfg.team_characters = make_team().characters;
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 0);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("All enemies defeated")
        );
    }

    #[test]
    fn test_termination_by_enemy_death_multi_enemy() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 1000;
        cfg.enemies = vec![make_enemy(0.0), make_enemy(0.0)];
        cfg.team_characters = make_team().characters;
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 0);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("All enemies defeated")
        );
    }

    // ------------------------------------------------------------------
    // 终止：角色死亡
    // ------------------------------------------------------------------

    #[test]
    fn test_termination_by_character_death() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 1000;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = vec![
            make_character(
                "dead_0",
                SpecialtyTag::Attack,
                ElementTag::Physical,
                0.0,
                0.0,
            ),
            make_character("dead_1", SpecialtyTag::Support, ElementTag::Ether, 0.0, 0.0),
        ];
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 0);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("All characters defeated")
        );
    }

    // ------------------------------------------------------------------
    // 事件日志
    // ------------------------------------------------------------------

    #[test]
    fn test_logged_events_contain_action_events() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 100;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        cfg.skills = make_skills_map(vec![make_skill(
            "normal_atk",
            SkillType::Normal,
            0.0,
            0.0,
            0.0,
            0,
        )]);
        cfg.apl = single_track_apl(
            "char_0",
            vec![ActionEntry {
                action_id: "normal_atk".into(),
                at: 0,
            }],
        );

        let result = SimulationRunner::run(cfg);

        // 应至少包含一个 normal_atk 的 ActionStart 事件
        let action_starts: Vec<&LoggedEvent> = result
            .events
            .iter()
            .filter(|e| e.event_type == EventType::ActionStart)
            .collect();
        assert!(!action_starts.is_empty(), "Expected ActionStart events");
        assert_eq!(action_starts[0].action_id.as_deref(), Some("normal_atk"));
        assert_eq!(action_starts[0].source_id.as_deref(), Some("char_0"));
    }

    #[test]
    fn test_logged_events_hit_frame_damage() {
        let mut skill = make_skill("combo", SkillType::Normal, 0.0, 0.0, 0.0, 0);
        skill.damage_multipliers = vec![crate::combat::skill::HitFrame {
            frame: 1,
            multiplier: 1.2,
        }];
        skill.animation_frames = 1;

        let mut cfg = SimConfig::default();
        cfg.max_tick = 100;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        cfg.skills = make_skills_map(vec![skill]);
        cfg.apl = single_track_apl(
            "char_0",
            vec![ActionEntry {
                action_id: "combo".into(),
                at: 0,
            }],
        );

        let result = SimulationRunner::run(cfg);

        // 应包含 DamageDealt 事件
        let dmg_events: Vec<&LoggedEvent> = result
            .events
            .iter()
            .filter(|e| e.event_type == EventType::DamageDealt)
            .collect();
        assert_eq!(dmg_events.len(), 1);
        assert_eq!(dmg_events[0].source_id.as_deref(), Some("char_0"));
        assert_eq!(dmg_events[0].action_id.as_deref(), Some("combo"));
    }

    #[test]
    fn test_logged_events_is_empty_with_no_apl() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 5;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        // 无 APL 轨道
        let result = SimulationRunner::run(cfg);
        // 无动作分派 → 无动作事件
        assert!(result
            .events
            .iter()
            .all(|e| e.event_type != EventType::ActionStart));
    }

    // ------------------------------------------------------------------
    // Tick 准确性
    // ------------------------------------------------------------------

    #[test]
    fn test_tick_count_matches_config() {
        for ticks in &[0u64, 1, 5, 10, 50] {
            let mut cfg = SimConfig::default();
            cfg.max_tick = *ticks;
            cfg.enemies = vec![make_enemy(50000.0)];
            cfg.team_characters = make_team().characters;
            let result = SimulationRunner::run(cfg);
            assert_eq!(
                result.total_ticks, *ticks,
                "expected {} ticks, got {}",
                ticks, result.total_ticks
            );
        }
    }

    // ------------------------------------------------------------------
    // 重复运行产生相同结果（确定性）
    // ------------------------------------------------------------------

    #[test]
    fn test_repeated_run_identical() {
        let mut config = SimConfig::default();
        config.max_tick = 20;
        config.enemies = vec![make_enemy(50000.0)];
        config.team_characters = make_team().characters;
        config.skills = make_skills_map(vec![make_skill(
            "normal_atk",
            SkillType::Normal,
            0.0,
            0.0,
            0.0,
            0,
        )]);
        config.apl = single_track_apl(
            "char_0",
            vec![
                ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 0,
                },
                ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 10,
                },
            ],
        );

        let r1 = SimulationRunner::run(config.clone());
        let r2 = SimulationRunner::run(config);

        assert_eq!(r1.total_ticks, r2.total_ticks);
        assert_eq!(r1.termination_reason, r2.termination_reason);
        assert_eq!(r1.events.len(), r2.events.len());
    }

    // ------------------------------------------------------------------
    // 协同动作系统集成
    // ------------------------------------------------------------------

    #[test]
    fn test_coordinated_system_no_crash() {
        // 运行器每个 tick 调用 coordinated_system.process_events()。
        // 没有注册监听器时，应该是一个空操作。
        let mut cfg = SimConfig::default();
        cfg.max_tick = 10;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        cfg.skills = make_skills_map(vec![make_skill(
            "normal_atk",
            SkillType::Normal,
            0.0,
            0.0,
            0.0,
            0,
        )]);
        cfg.apl = single_track_apl(
            "char_0",
            vec![ActionEntry {
                action_id: "normal_atk".into(),
                at: 0,
            }],
        );

        let result = SimulationRunner::run(cfg);
        assert!(result.total_ticks > 0);
        // 没有注册监听器时，不会产生 CoordinatedAction 事件
        assert!(result
            .events
            .iter()
            .all(|e| e.event_type != EventType::CoordinatedAction));
    }

    // ------------------------------------------------------------------
    // 边界情况
    // ------------------------------------------------------------------

    #[test]
    fn test_run_with_zero_max_tick_terminates_immediately() {
        let mut cfg = SimConfig::default();
        cfg.max_tick = 0;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 0);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("Max tick reached")
        );
    }

    #[test]
    fn test_run_with_partial_apl_exhaustion() {
        // 计划在 tick 0 和 tick 50 的两个动作。Max tick = 20。
        // 第二个动作不应分派，我们将以 max_tick 终止。
        let mut cfg = SimConfig::default();
        cfg.max_tick = 20;
        cfg.enemies = vec![make_enemy(50000.0)];
        cfg.team_characters = make_team().characters;
        cfg.skills = make_skills_map(vec![make_skill(
            "normal_atk",
            SkillType::Normal,
            0.0,
            0.0,
            0.0,
            0,
        )]);
        cfg.apl = single_track_apl(
            "char_0",
            vec![
                ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 0,
                },
                ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 50,
                },
            ],
        );
        let result = SimulationRunner::run(cfg);
        assert_eq!(result.total_ticks, 20);
        assert_eq!(
            result.termination_reason.as_deref(),
            Some("Max tick reached")
        );
        // 只有一个动作应被分派（在 tick 0）
        let action_starts: Vec<&LoggedEvent> = result
            .events
            .iter()
            .filter(|e| e.event_type == EventType::ActionStart)
            .collect();
        assert_eq!(action_starts.len(), 1);
    }
}
