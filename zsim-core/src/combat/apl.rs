use std::collections::HashMap;

use crate::combat::skill::SkillData;
use crate::combat::team::TeamManager;
use crate::combat::validator::{ResourceValidator, ValidationError};
use crate::data::apl::{APLData, ActionEntry, Track};
use crate::entities::enemy::EnemyState;

/// A validated and dispatched action, ready for execution by the simulation runner.
#[derive(Debug, Clone)]
pub struct SkillAction {
    pub action_id: String,
    pub source_id: String,
    pub target_id: String,
    pub started_at_tick: u64,
}

/// Runtime state for a single APL track.
#[derive(Debug, Clone)]
pub struct TrackState {
    pub track_id: String,
    pub char_id: String,
    /// Remaining actions in this track: (action_id, scheduled_tick).
    actions: Vec<ActionEntry>,
    /// Index into `actions` for the next action to check.
    pub next_action_index: usize,
    /// Tracks the most recently completed action_id (for prerequisite checks).
    pub last_completed_action_id: Option<String>,
}

impl TrackState {
    fn from_track(track: &Track) -> Self {
        Self {
            track_id: track.track_id.clone(),
            char_id: track.char_id.clone(),
            actions: track.actions.clone(),
            next_action_index: 0,
            last_completed_action_id: None,
        }
    }

    /// Returns true when all actions in this track have been dispatched.
    pub fn is_exhausted(&self) -> bool {
        self.next_action_index >= self.actions.len()
    }

    /// Returns a reference to the current (action_id, at_tick), if any.
    pub fn current_action(&self) -> Option<&ActionEntry> {
        self.actions.get(self.next_action_index)
    }
}

/// Manages APL (Action Priority List) track execution.
///
/// On each tick, scans all tracks for actions whose scheduled tick has arrived,
/// validates resource constraints, deducts costs, and queues [`SkillAction`]s
/// for the simulation runner to execute.
///
/// Tracks are **consuming**: once an action is dispatched it is removed from
/// the queue. `get_pending_actions()` drains the internal queue.
#[derive(Debug)]
pub struct APLManager {
    pub tracks: Vec<TrackState>,
    pub pending_actions: Vec<SkillAction>,
    pub is_exhausted: bool,
    validator: ResourceValidator,
}

impl APLManager {
    /// Create a new APLManager from an APL plan.
    ///
    /// The validator starts with no tracked cooldowns.
    pub fn new(apl_data: APLData) -> Self {
        let tracks: Vec<TrackState> = apl_data.tracks.iter().map(TrackState::from_track).collect();
        Self {
            tracks,
            pending_actions: Vec::new(),
            is_exhausted: false,
            validator: ResourceValidator::new(),
        }
    }

    /// Returns true when every track has dispatched all its actions.
    pub fn all_tracks_exhausted(&self) -> bool {
        self.tracks.iter().all(|t| t.is_exhausted())
    }

    /// Process the front action of each track whose scheduled tick has arrived.
    ///
    /// For each eligible action:
    /// 1. Validate prerequisites (prerequisite_action_id must be completed)
    /// 2. Validate resources via ResourceValidator
    /// 3. Deduct costs (energy, HP, decibel)
    /// 4. Set skill cooldown
    /// 5. Push a [`SkillAction`] to the pending queue
    ///
    /// Returns a list of [`ValidationError`]s for actions that failed resource checks.
    /// Actions blocked by prerequisites or future ticks are silently skipped.
    pub fn process_next_action(
        &mut self,
        current_tick: u64,
        team: &mut TeamManager,
        skills: &HashMap<String, SkillData>,
        enemies: &[EnemyState],
    ) -> Vec<ValidationError> {
        if self.is_exhausted {
            return Vec::new();
        }

        let mut errors = Vec::new();

        for track_idx in 0..self.tracks.len() {
            let track_exhausted = self.tracks[track_idx].is_exhausted();
            if track_exhausted {
                continue;
            }

            // Read the current action entry (immutable borrow on track)
            let entry = {
                let track = &self.tracks[track_idx];
                match track.actions.get(track.next_action_index) {
                    Some(e) => e.clone(),
                    None => continue,
                }
            };

            // Not time yet
            if entry.at > current_tick {
                continue;
            }

            // Look up skill data
            let skill = match skills.get(&entry.action_id) {
                Some(s) => s.clone(),
                None => {
                    // Unknown skill: skip this action and advance the track
                    self.tracks[track_idx].next_action_index += 1;
                    continue;
                }
            };

            // Check prerequisite (immutable borrow on track)
            {
                let track = &self.tracks[track_idx];
                if let Some(ref prereq) = skill.prerequisite_action_id {
                    if track.last_completed_action_id.as_ref() != Some(prereq) {
                        continue;
                    }
                }
            }

            let char_id = self.tracks[track_idx].char_id.clone();

            // Find character in team by char_id
            let char_idx = match team.characters.iter().position(|c| c.char_id == char_id) {
                Some(idx) => idx,
                None => continue,
            };

            // Validate resources (shared borrows on team + validator)
            {
                let character = &team.characters[char_idx];
                let enemy = enemies.first();
                let val_errors =
                    self.validator
                        .validate_all(character, &skill, team, current_tick, enemy);
                if !val_errors.is_empty() {
                    errors.extend(val_errors);
                    continue;
                }
            }

            // Deduct decibel (only if the character is on-field and skill costs decibel)
            if skill.decibel_cost > 0.0 && team.current_on_field_index == char_idx {
                let _ = team.consume_decibel(skill.decibel_cost);
            }

            // Deduct energy and HP from the character
            {
                let character = &mut team.characters[char_idx];
                character
                    .resources
                    .energy = (character.resources.energy - skill.energy_cost).max(0.0);
                character.current_stats.hp -= skill.hp_cost;
            }

            // Set skill cooldown
            self.validator
                .set_cooldown(&skill.action_id, skill.cooldown_ticks, current_tick);

            // Record pending action
            let target_id = enemies
                .first()
                .map(|e| e.enemy_id.clone())
                .unwrap_or_default();
            self.pending_actions.push(SkillAction {
                action_id: entry.action_id.clone(),
                source_id: char_id,
                target_id,
                started_at_tick: current_tick,
            });

            // Advance track pointer
            {
                let track = &mut self.tracks[track_idx];
                track.last_completed_action_id = Some(entry.action_id);
                track.next_action_index += 1;
            }
        }

        // Refresh the exhausted flag
        self.is_exhausted = self.all_tracks_exhausted();

        errors
    }

    /// Drain all pending skill actions (consuming queue pattern).
    ///
    /// Returns an empty vec if no actions are pending.
    pub fn get_pending_actions(&mut self) -> Vec<SkillAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::skill::SkillData;
    use crate::combat::team::TeamManager;
    use crate::data::apl::{ActionEntry, Track};
    use crate::entities::character::Character;
    use crate::entities::enemy::EnemyState;
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

    fn make_team() -> TeamManager {
        TeamManager::new(vec![
            make_character("char_0", SpecialtyTag::Attack, ElementTag::Physical, 100.0, 8000.0),
            make_character("char_1", SpecialtyTag::Support, ElementTag::Ether, 120.0, 6000.0),
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
            animation_frames: 30,
        }
    }

    fn make_enemy() -> EnemyState {
        let mut enemy = EnemyState::new("test_enemy", EnemyType::Elite, 100.0);
        enemy.hp = 50000.0;
        enemy.resistances.insert(ElementTag::Fire, 0.5);
        enemy
    }

    fn make_skills_map(skills: Vec<SkillData>) -> HashMap<String, SkillData> {
        skills.into_iter().map(|s| (s.action_id.clone(), s)).collect()
    }

    fn default_skills() -> HashMap<String, SkillData> {
        make_skills_map(vec![
            make_skill("normal_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0),
            make_skill("ex_skill", SkillType::Special, 30.0, 0.0, 0.0, 10),
            make_skill("ultimate", SkillType::Ultimate, 0.0, 0.0, 2000.0, 120),
            make_skill("hp_cost_skill", SkillType::Special, 0.0, 500.0, 0.0, 0),
        ])
    }

    // ------------------------------------------------------------------
    // APLManager creation
    // ------------------------------------------------------------------

    #[test]
    fn test_new_empty_tracks() {
        let apl = APLData { tracks: vec![] };
        let mgr = APLManager::new(apl);
        assert!(mgr.tracks.is_empty());
        assert!(mgr.pending_actions.is_empty());
        assert!(!mgr.is_exhausted);
        assert!(mgr.all_tracks_exhausted());
    }

    #[test]
    fn test_new_with_tracks() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry { action_id: "atk".into(), at: 0 },
                    ActionEntry { action_id: "skill".into(), at: 30 },
                ],
            }],
        };
        let mgr = APLManager::new(apl);
        assert_eq!(mgr.tracks.len(), 1);
        assert_eq!(mgr.tracks[0].track_id, "t1");
        assert_eq!(mgr.tracks[0].next_action_index, 0);
        assert!(!mgr.all_tracks_exhausted());
    }

    // ------------------------------------------------------------------
    // TrackState helpers
    // ------------------------------------------------------------------

    #[test]
    fn test_track_exhausted_initial() {
        let track = TrackState::from_track(&Track {
            track_id: "t1".into(),
            char_id: "c1".into(),
            actions: vec![ActionEntry { action_id: "a1".into(), at: 0 }],
        });
        assert!(!track.is_exhausted());
    }

    #[test]
    fn test_track_exhausted_when_done() {
        let mut track = TrackState::from_track(&Track {
            track_id: "t1".into(),
            char_id: "c1".into(),
            actions: vec![ActionEntry { action_id: "a1".into(), at: 0 }],
        });
        track.next_action_index = 1;
        assert!(track.is_exhausted());
    }

    #[test]
    fn test_track_current_action() {
        let track = TrackState::from_track(&Track {
            track_id: "t1".into(),
            char_id: "c1".into(),
            actions: vec![ActionEntry { action_id: "a1".into(), at: 0 }],
        });
        let entry = track.current_action().unwrap();
        assert_eq!(entry.action_id, "a1");
        assert_eq!(entry.at, 0);
    }

    #[test]
    fn test_track_current_action_none_when_exhausted() {
        let mut track = TrackState::from_track(&Track {
            track_id: "t1".into(),
            char_id: "c1".into(),
            actions: vec![],
        });
        track.next_action_index = 0;
        assert!(track.current_action().is_none());
    }

    // ------------------------------------------------------------------
    // Action dispatch
    // ------------------------------------------------------------------

    #[test]
    fn test_dispatch_action_at_tick_zero() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);

        let actions = mgr.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "normal_atk");
        assert_eq!(actions[0].source_id, "char_0");
        assert_eq!(actions[0].target_id, "test_enemy");
        assert_eq!(actions[0].started_at_tick, 0);
    }

    #[test]
    fn test_action_not_dispatched_before_scheduled_tick() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 30 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // Tick 0 — not dispatched yet
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 0);

        // Tick 30 — should dispatch
        let errors = mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 1);
    }

    #[test]
    fn test_multiple_actions_in_sequence() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry { action_id: "normal_atk".into(), at: 0 },
                    ActionEntry { action_id: "ex_skill".into(), at: 30 },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // Tick 0: dispatch normal_atk
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 1);

        // Tick 30: dispatch ex_skill
        mgr.process_next_action(30, &mut team, &skills, &enemies);
        let actions = mgr.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "ex_skill");
    }

    #[test]
    fn test_track_exhausted_after_all_actions_dispatched() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        assert!(!mgr.all_tracks_exhausted());
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();
        assert!(mgr.all_tracks_exhausted());
        assert!(mgr.is_exhausted);
    }

    #[test]
    fn test_no_dispatch_when_exhausted() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        // Second call: exhausted, no actions
        mgr.process_next_action(1, &mut team, &skills, &enemies);
        assert!(mgr.get_pending_actions().is_empty());
    }

    // ------------------------------------------------------------------
    // Prerequisite checking
    // ------------------------------------------------------------------

    #[test]
    fn test_prerequisite_not_met_blocks_dispatch() {
        let mut skill = make_skill("follow_up", SkillType::Special, 0.0, 0.0, 0.0, 0);
        skill.prerequisite_action_id = Some("normal_atk".into());

        let skills = make_skills_map(vec![
            make_skill("normal_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0),
            skill,
        ]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry { action_id: "follow_up".into(), at: 0 },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        // Prerequisite "normal_atk" not completed yet — blocked
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 0);
    }

    #[test]
    fn test_prerequisite_met_allows_dispatch() {
        let mut follow_up = make_skill("follow_up", SkillType::Special, 0.0, 0.0, 0.0, 0);
        follow_up.prerequisite_action_id = Some("normal_atk".into());

        let skills = make_skills_map(vec![
            make_skill("normal_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0),
            follow_up,
        ]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry { action_id: "normal_atk".into(), at: 0 },
                    ActionEntry { action_id: "follow_up".into(), at: 10 },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        // Tick 0: dispatch normal_atk
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 1);

        // Tick 10: follow_up should now be eligible
        let errors = mgr.process_next_action(10, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let actions = mgr.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "follow_up");
    }

    // ------------------------------------------------------------------
    // Resource validation + deduction
    // ------------------------------------------------------------------

    #[test]
    fn test_insufficient_energy_blocks_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "ex_skill".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 10.0; // ex_skill costs 30
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, crate::combat::validator::ResourceType::Energy);
        // Action not dispatched
        assert_eq!(mgr.get_pending_actions().len(), 0);
    }

    #[test]
    fn test_energy_deducted_on_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "ex_skill".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 50.0;
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        assert!((team.characters[0].resources.energy - 20.0).abs() < 1e-9, "expected 20 energy remaining");
    }

    #[test]
    fn test_hp_deducted_on_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "hp_cost_skill".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].current_stats.hp = 8000.0;
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        assert!((team.characters[0].current_stats.hp - 7500.0).abs() < 1e-9, "expected 7500 hp remaining");
    }

    #[test]
    fn test_insufficient_hp_blocks_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "hp_cost_skill".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].current_stats.hp = 300.0; // hp_cost_skill costs 500
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, crate::combat::validator::ResourceType::Hp);
        assert_eq!(mgr.get_pending_actions().len(), 0);
    }

    // ------------------------------------------------------------------
    // Cooldown tracking
    // ------------------------------------------------------------------

    #[test]
    fn test_cooldown_prevents_immediate_reuse() {
        let skills = make_skills_map(vec![
            make_skill("ex_skill", SkillType::Special, 10.0, 0.0, 0.0, 30),
        ]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry { action_id: "ex_skill".into(), at: 0 },
                    ActionEntry { action_id: "ex_skill".into(), at: 10 },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 100.0; // enough for two casts
        let enemies = vec![make_enemy()];

        // First use at tick 0 — succeeds
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let _ = mgr.get_pending_actions();

        // Second use at tick 10 — blocked by cooldown (30 ticks from tick 0)
        let errors = mgr.process_next_action(10, &mut team, &skills, &enemies);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].missing_resource, crate::combat::validator::ResourceType::SkillCooldown);
    }

    #[test]
    fn test_cooldown_expires_and_allows_reuse() {
        let skills = make_skills_map(vec![
            make_skill("ex_skill", SkillType::Special, 10.0, 0.0, 0.0, 20),
        ]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry { action_id: "ex_skill".into(), at: 0 },
                    ActionEntry { action_id: "ex_skill".into(), at: 25 },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 100.0;
        let enemies = vec![make_enemy()];

        // First use at tick 0
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        // Second use at tick 25 — cooldown expired (20 ticks)
        let errors = mgr.process_next_action(25, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 1);
    }

    // ------------------------------------------------------------------
    // Multi-track
    // ------------------------------------------------------------------

    #[test]
    fn test_multi_track_dispatch() {
        let apl = APLData {
            tracks: vec![
                Track {
                    track_id: "t1".into(),
                    char_id: "char_0".into(),
                    actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
                },
                Track {
                    track_id: "t2".into(),
                    char_id: "char_1".into(),
                    actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
                },
            ],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());

        let actions = mgr.get_pending_actions();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].source_id, "char_0");
        assert_eq!(actions[1].source_id, "char_1");
    }

    #[test]
    fn test_multi_track_independent_ticks() {
        let apl = APLData {
            tracks: vec![
                Track {
                    track_id: "t1".into(),
                    char_id: "char_0".into(),
                    actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
                },
                Track {
                    track_id: "t2".into(),
                    char_id: "char_1".into(),
                    actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 50 }],
                },
            ],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // Tick 0: only track 1 dispatches
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 1);

        // Tick 30: track 2 not ready yet
        mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 0);

        // Tick 50: track 2 dispatches
        mgr.process_next_action(50, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 1);
    }

    // ------------------------------------------------------------------
    // Unknown / missing skills
    // ------------------------------------------------------------------

    #[test]
    fn test_unknown_skill_id_is_skipped() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "nonexistent_skill".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills: HashMap<String, SkillData> = HashMap::new();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 0);
        // Track should be exhausted (action skipped)
        assert!(mgr.all_tracks_exhausted());
    }

    // ------------------------------------------------------------------
    // Unknown character ID
    // ------------------------------------------------------------------

    #[test]
    fn test_unknown_char_id_is_skipped() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "nonexistent_char".into(),
                actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 0);
    }

    // ------------------------------------------------------------------
    // get_pending_actions is consuming
    // ------------------------------------------------------------------

    #[test]
    fn test_get_pending_actions_drains() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry { action_id: "normal_atk".into(), at: 0 },
                    ActionEntry { action_id: "normal_atk".into(), at: 10 },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(mgr.pending_actions.len(), 1);

        let first = mgr.get_pending_actions();
        assert_eq!(first.len(), 1);
        assert!(mgr.pending_actions.is_empty());

        mgr.process_next_action(10, &mut team, &skills, &enemies);
        assert_eq!(mgr.pending_actions.len(), 1);
    }

    // ------------------------------------------------------------------
    // Decibel deduction for on-field character
    // ------------------------------------------------------------------

    #[test]
    fn test_decibel_deducted_when_on_field() {
        let skills = make_skills_map(vec![
            make_skill("ultimate", SkillType::Ultimate, 0.0, 0.0, 1500.0, 0),
        ]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "ultimate".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.decibel = 2000.0; // char_0 is on-field
        let enemies = vec![make_enemy()];

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        assert!((team.characters[0].resources.decibel - 500.0).abs() < 1e-9);
    }

    #[test]
    fn test_decibel_not_deducted_for_off_field_character() {
        let skills = make_skills_map(vec![
            make_skill("coord_atk", SkillType::Coordinated, 0.0, 0.0, 100.0, 0),
        ]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_1".into(), // char_1 is off-field
                actions: vec![ActionEntry { action_id: "coord_atk".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[1].resources.decibel = 500.0; // off-field, but has decibel
        team.characters[0].resources.decibel = 500.0; // on-field also has decibel
        let enemies = vec![make_enemy()];

        // validate_decibel checks on-field char. Since skill costs 100 decibel,
        // and on-field char has 500, validation passes. But deduction only happens
        // if the ACTING character is on-field — char_1 is off-field, so no deduction.
        // This is expected: coordinated attacks shouldn't have decibel cost in practice.
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let _ = mgr.get_pending_actions();

        // off-field character's decibel unchanged
        assert!((team.characters[1].resources.decibel - 500.0).abs() < 1e-9);
        // on-field character's decibel unchanged too
        assert!((team.characters[0].resources.decibel - 500.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // Integration: full multi-track with resource tracking
    // ------------------------------------------------------------------

    #[test]
    fn test_full_multi_track_integration() {
        // Two characters with different APL tracks
        let ex_skill = make_skill("ex_skill", SkillType::Special, 30.0, 0.0, 0.0, 15);

        let skills = make_skills_map(vec![
            make_skill("normal_atk", SkillType::Normal, 0.0, 0.0, 0.0, 0),
            ex_skill,
        ]);

        let apl = APLData {
            tracks: vec![
                Track {
                    track_id: "t1".into(),
                    char_id: "char_0".into(),
                    actions: vec![
                        ActionEntry { action_id: "normal_atk".into(), at: 0 },
                        ActionEntry { action_id: "ex_skill".into(), at: 20 },
                        ActionEntry { action_id: "normal_atk".into(), at: 50 },
                    ],
                },
                Track {
                    track_id: "t2".into(),
                    char_id: "char_1".into(),
                    actions: vec![
                        ActionEntry { action_id: "normal_atk".into(), at: 0 },
                        ActionEntry { action_id: "normal_atk".into(), at: 30 },
                    ],
                },
            ],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 100.0;
        let enemies = vec![make_enemy()];

        // Tick 0: both tracks dispatch normal_atk
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 2);

        // Tick 20: char_0 dispatches ex_skill (costs 30 energy)
        let errors = mgr.process_next_action(20, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 1);
        assert!((team.characters[0].resources.energy - 70.0).abs() < 1e-9);

        // Tick 30: char_1 dispatches second normal_atk
        let errors = mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 1);

        // Tick 50: char_0 dispatches final normal_atk
        let errors = mgr.process_next_action(50, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 1);

        // All tracks exhausted
        assert!(mgr.all_tracks_exhausted());
        assert!(mgr.is_exhausted);
    }

    #[test]
    fn test_exhausted_flag_no_longer_processes() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();
        assert!(mgr.is_exhausted);

        // Try to process again — should be a no-op
        let errors = mgr.process_next_action(100, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert!(mgr.get_pending_actions().is_empty());
    }

    #[test]
    fn test_error_on_insufficient_energy_does_not_advance_track() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry { action_id: "ex_skill".into(), at: 0 },
                    ActionEntry { action_id: "normal_atk".into(), at: 10 },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 5.0; // not enough for ex_skill (costs 30)
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // Tick 0: ex_skill fails validation
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(errors.len(), 1);
        assert_eq!(mgr.get_pending_actions().len(), 0);

        // Track did NOT advance — still on ex_skill
        assert_eq!(mgr.tracks[0].next_action_index, 0);

        // Give more energy and retry at tick 10
        team.characters[0].resources.energy = 100.0;
        let errors = mgr.process_next_action(10, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let actions = mgr.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "ex_skill");
    }

    #[test]
    fn test_same_tick_no_double_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry { action_id: "normal_atk".into(), at: 0 }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // First call at tick 0
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 1);

        // Second call at same tick: track already advanced, nothing new
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 0);
    }
}
