//! 使用 rayon 的并行模拟执行器。
//!
//! 并行运行多个独立模拟，每个在自己的线程上运行。
//! 每次运行都会重新创建所有每模拟状态（RNG、GameState、EventBus 等）。
//! 共享的只读数据（技能、角色、敌人、APL）通过 [`Arc`] 传递，避免跨线程的不必要克隆。

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};

use rayon::prelude::*;

use crate::combat::runner::{LoggedEvent, SimConfig, SimulationRunner};
use crate::combat::skill::SkillData;
use crate::data::apl::APLData;
use crate::entities::character::Character;
use crate::entities::enemy::EnemyState;
use crate::entities::enums::SimMode;

/// 单个模拟结果，带有用于识别的索引。
#[derive(Debug, Clone)]
pub struct SimResult {
    pub sim_index: usize,
    pub seed: u64,
    pub total_ticks: u64,
    pub termination_reason: Option<String>,
    pub events: Vec<LoggedEvent>,
}

/// 并行模拟执行的配置。
///
/// 大的只读数据块（`team_characters`、`enemies`、`skills`、`apl`）
/// 使用 [`Arc`] 包装以实现高效的跨线程共享。
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// 要运行的模拟数量。
    pub sim_count: usize,
    /// RNG 的基础种子（每次模拟使用 `seed = base_seed + sim_index`）。
    pub base_seed: u64,
    /// 每次模拟的最大 tick 数。
    pub max_tick: u64,
    /// 模拟模式（通常为 `Parallel`）。
    pub mode: SimMode,
    /// 共享的队伍角色模板（使用 Arc 包装以实现跨线程共享）。
    pub team_characters: Arc<Vec<Character>>,
    /// 共享的敌人模板。
    pub enemies: Arc<Vec<EnemyState>>,
    /// 共享的技能定义。
    pub skills: Arc<HashMap<String, SkillData>>,
    /// 共享的 APL 计划。
    pub apl: Arc<APLData>,
    /// 可选的邦布角色。
    pub bangboo: Arc<Option<Character>>,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            sim_count: 10,
            base_seed: 42,
            max_tick: 18000,
            mode: SimMode::Parallel,
            team_characters: Arc::new(Vec::new()),
            enemies: Arc::new(Vec::new()),
            skills: Arc::new(HashMap::new()),
            apl: Arc::new(APLData { tracks: Vec::new() }),
            bangboo: Arc::new(None),
        }
    }
}

/// 从并行工作线程发送给调用者的进度更新。
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// 已完成模拟的百分比。
    Percentage(f64),
    /// 所有模拟已完成。
    Done,
}

/// 使用 rayon 的并行模拟执行器。
///
/// 为每次运行创建独立的模拟状态（RNG / GameState / EventBus /
/// APLManager / BuffManager / AnomalyManager / EquipmentManager）
/// 并将工作分配到 rayon 线程池。
pub struct ParallelRunner;

impl ParallelRunner {
    /// 并行执行模拟。
    ///
    /// 每次模拟接收共享配置的独立克隆，并产生一个 [`SimResult`]。
    /// 返回所有结果以及一个 [`mpsc::Receiver`]，它以约 1% 的间隔提供进度更新。
    pub fn run(config: ParallelConfig) -> (Vec<SimResult>, mpsc::Receiver<ProgressUpdate>) {
        let (tx, rx) = mpsc::channel();

        let sim_count = config.sim_count;
        let base_seed = config.base_seed;

        // 空批次提前返回。
        if sim_count == 0 {
            let _ = tx.send(ProgressUpdate::Done);
            return (Vec::new(), rx);
        }

        let max_tick = config.max_tick;
        let mode = config.mode;
        let team_characters = config.team_characters;
        let enemies = config.enemies;
        let skills = config.skills;
        let apl = config.apl;
        let bangboo = config.bangboo;

        // 每完成约 1% 的运行报告进度。
        let progress_step = (sim_count as f64 / 100.0).ceil() as usize;
        let progress_step = progress_step.max(1);

        let counter = AtomicUsize::new(0);
        let tx_ref = &tx;
        let counter_ref = &counter;

        let results: Vec<SimResult> = (0..sim_count)
            .into_par_iter()
            .map(|i| {
                let seed = base_seed.wrapping_add(i as u64);

                let sim_config = SimConfig {
                    team_characters: (*team_characters).clone(),
                    enemies: (*enemies).clone(),
                    skills: (*skills).clone(),
                    apl: (*apl).clone(),
                    max_tick,
                    seed,
                    mode: mode.clone(),
                    loop_count: 1,
                    bangboo: (*bangboo).clone(),
                };

                let result = SimulationRunner::run(sim_config);

                // 以约 1% 的间隔报告进度。
                let completed = counter_ref.fetch_add(1, Ordering::SeqCst) + 1;
                if completed.is_multiple_of(progress_step) || completed == sim_count {
                    let pct = (completed as f64 / sim_count as f64 * 100.0).min(100.0);
                    let _ = tx_ref.send(ProgressUpdate::Percentage(pct));
                }

                SimResult {
                    sim_index: i,
                    seed,
                    total_ticks: result.total_ticks,
                    termination_reason: result.termination_reason,
                    events: result.events,
                }
            })
            .collect();

        let _ = tx.send(ProgressUpdate::Done);

        (results, rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::apl::{ActionEntry, Track};
    use crate::entities::character::Character;
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

    fn make_team_vec() -> Vec<Character> {
        vec![
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
        ]
    }

    fn make_skill(action_id: &str, energy_cost: f64, cooldown_ticks: u64) -> SkillData {
        SkillData {
            action_id: action_id.to_string(),
            action_type: SkillType::Normal,
            damage_multipliers: vec![],
            daze_multiplier: 0.0,
            hit_frames: vec![],
            invincible_frames: vec![],
            interruptible_frame: 0,
            is_snapshot: false,
            charge_branches: vec![],
            prerequisite_action_id: None,
            hp_cost: 0.0,
            energy_cost,
            decibel_cost: 0.0,
            cooldown_ticks,
            animation_frames: 1,
        }
    }

    fn make_skills_map(skills: Vec<SkillData>) -> HashMap<String, SkillData> {
        skills
            .into_iter()
            .map(|s| (s.action_id.clone(), s))
            .collect()
    }

    fn make_enemy(hp: f64) -> EnemyState {
        let mut enemy = EnemyState::new("test_enemy", EnemyType::Elite, 100.0);
        enemy.hp = hp;
        enemy
    }

    fn make_default_skills() -> HashMap<String, SkillData> {
        make_skills_map(vec![make_skill("normal_atk", 0.0, 0)])
    }

    fn default_parallel_config() -> ParallelConfig {
        ParallelConfig {
            sim_count: 10,
            base_seed: 42,
            max_tick: 20,
            mode: SimMode::Parallel,
            team_characters: Arc::new(make_team_vec()),
            enemies: Arc::new(vec![make_enemy(50000.0)]),
            skills: Arc::new(make_default_skills()),
            apl: Arc::new(APLData {
                tracks: vec![Track {
                    track_id: "t1".into(),
                    char_id: "char_0".into(),
                    actions: vec![ActionEntry {
                        action_id: "normal_atk".into(),
                        at: 0,
                    }],
                }],
            }),
            bangboo: Arc::new(None),
        }
    }

    // ------------------------------------------------------------------
    // 基本执行
    // ------------------------------------------------------------------

    #[test]
    fn test_parallel_basic_execution() {
        let (results, rx) = ParallelRunner::run(default_parallel_config());

        assert_eq!(results.len(), 10, "should produce 10 results");

        // 所有结果应该相同（相同种子 → 确定性输出）。
        let first = &results[0];
        for r in &results {
            assert_eq!(r.total_ticks, first.total_ticks);
            assert_eq!(r.termination_reason, first.termination_reason);
            assert_eq!(r.events.len(), first.events.len());
        }

        // 进度更新应该到达。
        let updates: Vec<ProgressUpdate> = rx.iter().collect();
        assert!(!updates.is_empty(), "should receive progress updates");
        assert!(updates.iter().any(|u| matches!(u, ProgressUpdate::Done)));
    }

    #[test]
    fn test_parallel_result_count_matches_config() {
        for count in &[1usize, 5, 50, 100] {
            let mut config = default_parallel_config();
            config.sim_count = *count;
            let (results, _rx) = ParallelRunner::run(config);
            assert_eq!(
                results.len(),
                *count,
                "expected {} results, got {}",
                count,
                results.len()
            );
        }
    }

    // ------------------------------------------------------------------
    // 种子无关的确定性
    // ------------------------------------------------------------------

    #[test]
    fn test_parallel_deterministic_results() {
        // 使用相同的技能（无随机性），所有模拟都产生相同的输出，
        // 尽管种子不同（base_seed + sim_index）。
        let config = ParallelConfig {
            sim_count: 50,
            max_tick: 30,
            ..default_parallel_config()
        };

        let (results, _rx) = ParallelRunner::run(config);

        assert_eq!(results.len(), 50);
        let first = &results[0];
        for r in &results {
            assert_eq!(
                r.total_ticks, first.total_ticks,
                "tick mismatch at sim {}",
                r.sim_index
            );
            assert_eq!(r.termination_reason, first.termination_reason);
            assert_eq!(r.events.len(), first.events.len());
        }

        // 种子应该是连续的。
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.seed, 42 + i as u64);
            assert_eq!(r.sim_index, i);
        }
    }

    // ------------------------------------------------------------------
    // 并行结果与串行（单线程）执行匹配
    // ------------------------------------------------------------------

    #[test]
    fn test_parallel_results_match_serial() {
        let team = make_team_vec();
        let enemies = vec![make_enemy(50000.0)];
        let skills = make_default_skills();
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry {
                        action_id: "normal_atk".into(),
                        at: 0,
                    },
                    ActionEntry {
                        action_id: "normal_atk".into(),
                        at: 10,
                    },
                ],
            }],
        };

        // 单线程运行（种子 = 42）。
        let serial_result = SimulationRunner::run(SimConfig {
            team_characters: team.clone(),
            enemies: enemies.clone(),
            skills: skills.clone(),
            apl: apl.clone(),
            max_tick: 20,
            seed: 42,
            mode: SimMode::Single,
            loop_count: 1,
            bangboo: None,
        });

        // 并行运行（100 次模拟，sim 0 使用 base_seed + 0 = 42）。
        let config = ParallelConfig {
            sim_count: 100,
            base_seed: 42,
            max_tick: 20,
            mode: SimMode::Parallel,
            team_characters: Arc::new(team),
            enemies: Arc::new(enemies),
            skills: Arc::new(skills),
            apl: Arc::new(apl),
            bangboo: Arc::new(None),
        };

        let (results, _rx) = ParallelRunner::run(config);

        assert_eq!(results.len(), 100);

        // 并行中的模拟 0 应与串行匹配（相同种子 42）。
        let r0 = &results[0];
        assert_eq!(r0.total_ticks, serial_result.total_ticks);
        assert_eq!(r0.termination_reason, serial_result.termination_reason);
        assert_eq!(r0.events.len(), serial_result.events.len());

        // 具有确定性技能的所有并行模拟产生相同的结果。
        for r in &results {
            assert_eq!(r.total_ticks, serial_result.total_ticks);
            assert_eq!(r.events.len(), serial_result.events.len());
        }
    }

    // ------------------------------------------------------------------
    // 边界情况
    // ------------------------------------------------------------------

    #[test]
    fn test_parallel_zero_sim_count() {
        let mut config = default_parallel_config();
        config.sim_count = 0;
        let (results, rx) = ParallelRunner::run(config);

        assert!(
            results.is_empty(),
            "zero sim count should return no results"
        );

        let updates: Vec<ProgressUpdate> = rx.iter().collect();
        assert!(
            updates.iter().any(|u| matches!(u, ProgressUpdate::Done)),
            "should receive Done for empty batch"
        );
    }

    #[test]
    fn test_parallel_single_sim() {
        let mut config = default_parallel_config();
        config.sim_count = 1;
        let (results, _rx) = ParallelRunner::run(config);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].sim_index, 0);
        assert_eq!(results[0].seed, 42); // base_seed + 0
    }

    #[test]
    fn test_parallel_progress_updates() {
        let mut config = default_parallel_config();
        config.sim_count = 100;
        let (results, rx) = ParallelRunner::run(config);

        assert_eq!(results.len(), 100);

        let updates: Vec<ProgressUpdate> = rx.iter().collect();

        // 应接收百分比更新。
        let percentages: Vec<f64> = updates
            .iter()
            .filter_map(|u| {
                if let ProgressUpdate::Percentage(pct) = u {
                    Some(*pct)
                } else {
                    None
                }
            })
            .collect();

        assert!(!percentages.is_empty(), "should have percentage updates");
        assert!(
            percentages.iter().any(|&p| (p - 100.0).abs() < 1e-6),
            "should have 100% update"
        );
        assert!(
            updates.iter().any(|u| matches!(u, ProgressUpdate::Done)),
            "should have Done update"
        );
    }
}
