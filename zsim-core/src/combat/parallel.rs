//! Parallel simulation executor using rayon.
//!
//! Runs multiple independent simulations in parallel, each on its own thread.
//! All per-simulation state (RNG, GameState, EventBus, etc.) is created fresh
//! for each run.  Shared read-only data (skills, characters, enemies, APL)
//! is passed via [`Arc`] to avoid unnecessary cloning across threads.

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

/// A single simulation result with index for identification.
#[derive(Debug, Clone)]
pub struct SimResult {
    pub sim_index: usize,
    pub seed: u64,
    pub total_ticks: u64,
    pub termination_reason: Option<String>,
    pub events: Vec<LoggedEvent>,
}

/// Configuration for parallel simulation execution.
///
/// Large read-only data blocks (`team_characters`, `enemies`, `skills`, `apl`)
/// are [`Arc`]-wrapped for efficient cross-thread sharing.
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Number of simulations to run.
    pub sim_count: usize,
    /// Base seed for RNG (each sim uses `seed = base_seed + sim_index`).
    pub base_seed: u64,
    /// Maximum ticks per simulation.
    pub max_tick: u64,
    /// Simulation mode (typically `Parallel`).
    pub mode: SimMode,
    /// Shared team characters template (Arc-wrapped for cross-thread sharing).
    pub team_characters: Arc<Vec<Character>>,
    /// Shared enemy template.
    pub enemies: Arc<Vec<EnemyState>>,
    /// Shared skill definitions.
    pub skills: Arc<HashMap<String, SkillData>>,
    /// Shared APL plan.
    pub apl: Arc<APLData>,
    /// Optional bangboo character.
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

/// Progress update sent from parallel workers to the caller.
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// Percentage of total simulations completed.
    Percentage(f64),
    /// All simulations finished.
    Done,
}

/// Parallel simulation executor using rayon.
///
/// Creates independent simulation state (RNG / GameState / EventBus /
/// APLManager / BuffManager / AnomalyManager / EquipmentManager) for
/// each run and distributes work across the rayon thread pool.
pub struct ParallelRunner;

impl ParallelRunner {
    /// Execute simulations in parallel.
    ///
    /// Each simulation receives an independent clone of the shared config
    /// and produces a [`SimResult`].  Returns all results along with a
    /// [`mpsc::Receiver`] that provides progress updates at ~1% intervals.
    pub fn run(config: ParallelConfig) -> (Vec<SimResult>, mpsc::Receiver<ProgressUpdate>) {
        let (tx, rx) = mpsc::channel();

        let sim_count = config.sim_count;
        let base_seed = config.base_seed;

        // Early-out for empty batch.
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

        // Report progress every ~1% of completed runs.
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
                    bangboo: (*bangboo).clone(),
                };

                let result = SimulationRunner::run(sim_config);

                // Report progress at ~1% intervals.
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
    // Helpers
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
    // Basic execution
    // ------------------------------------------------------------------

    #[test]
    fn test_parallel_basic_execution() {
        let (results, rx) = ParallelRunner::run(default_parallel_config());

        assert_eq!(results.len(), 10, "should produce 10 results");

        // All results should be identical (same seed → deterministic output).
        let first = &results[0];
        for r in &results {
            assert_eq!(r.total_ticks, first.total_ticks);
            assert_eq!(r.termination_reason, first.termination_reason);
            assert_eq!(r.events.len(), first.events.len());
        }

        // Progress updates should arrive.
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
    // Seed-independent determinism
    // ------------------------------------------------------------------

    #[test]
    fn test_parallel_deterministic_results() {
        // With the same skills (no randomness), all sims produce identical output
        // despite differing seeds (base_seed + sim_index).
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

        // Seeds should be sequential.
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.seed, 42 + i as u64);
            assert_eq!(r.sim_index, i);
        }
    }

    // ------------------------------------------------------------------
    // Parallel results match serial (single-threaded) execution
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

        // Single-threaded run (seed = 42).
        let serial_result = SimulationRunner::run(SimConfig {
            team_characters: team.clone(),
            enemies: enemies.clone(),
            skills: skills.clone(),
            apl: apl.clone(),
            max_tick: 20,
            seed: 42,
            mode: SimMode::Single,
            bangboo: None,
        });

        // Parallel run (100 sims, sim 0 uses base_seed + 0 = 42).
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

        // Sim 0 in parallel should match serial (same seed 42).
        let r0 = &results[0];
        assert_eq!(r0.total_ticks, serial_result.total_ticks);
        assert_eq!(r0.termination_reason, serial_result.termination_reason);
        assert_eq!(r0.events.len(), serial_result.events.len());

        // All parallel sims with deterministic skills produce identical results.
        for r in &results {
            assert_eq!(r.total_ticks, serial_result.total_ticks);
            assert_eq!(r.events.len(), serial_result.events.len());
        }
    }

    // ------------------------------------------------------------------
    // Edge cases
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

        // Should receive percentage updates.
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
