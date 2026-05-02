//! APL action queue manager — schedules and executes per-character skill
//! rotations (tracks) with full action lifecycle support.

use std::collections::HashMap;

use crate::combat::skill::SkillData;
use crate::combat::team::TeamManager;
use crate::combat::validator::{ResourceValidator, ValidationError};
use crate::data::apl::{APLData, ActionEntry, Track};
use crate::entities::enemy::EnemyState;

/// A validated and dispatched action, ready for execution by the simulation runner.
///
/// Used by [`crate::combat::coordinated::CoordinatedActionSystem`] and other
/// systems that consume APL dispatch events.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillAction {
    pub action_id: String,
    pub source_id: String,
    pub target_id: String,
    pub started_at_tick: u64,
}

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// Events produced by the APL manager during action execution.
///
/// The simulation runner drains these via [`APLManager::get_pending_actions`].
#[derive(Debug, Clone, PartialEq)]
pub enum PendingAction {
    /// A skill started executing (animation began).
    ActionStarted { char_id: String, action_id: String },
    /// A hit frame was reached during skill execution.
    HitFrameTriggered {
        char_id: String,
        action_id: String,
        frame: u64,
        multiplier: f64,
    },
    /// A skill's animation completed.
    ActionCompleted { char_id: String, action_id: String },
    /// A skill failed validation or could not be found.
    ActionFailed {
        char_id: String,
        action_id: String,
        reason: String,
    },
    /// Charging started for a chargeable skill.
    ChargingStarted {
        char_id: String,
        action_id: String,
        charge_duration: u64,
    },
    /// Charging completed and variant action began.
    ChargingCompleted {
        char_id: String,
        action_id: String,
        variant_action_id: String,
    },
}

// ---------------------------------------------------------------------------
// Animation state
// ---------------------------------------------------------------------------

/// Per-track execution state.
#[derive(Debug, Clone)]
pub enum AnimationState {
    /// No action currently executing.
    Idle,
    /// A skill is playing its animation.
    Animating {
        action_id: String,
        total_frames: u64,
        elapsed_frames: u64,
        /// Remaining hit frames to trigger: (frame_number, multiplier).
        hit_frames: Vec<(u64, f64)>,
    },
    /// A chargeable action is charging.
    Charging {
        action_id: String,
        charge_elapsed: u64,
        charge_duration: u64,
        variant_action_id: String,
    },
}

// ---------------------------------------------------------------------------
// Track state
// ---------------------------------------------------------------------------

/// Runtime state for a single APL track.
#[derive(Debug, Clone)]
pub struct TrackState {
    pub track_id: String,
    pub char_id: String,
    /// Remaining actions: (action_id, scheduled_tick).
    actions: Vec<ActionEntry>,
    /// Index into `actions` for the next action to check.
    pub next_action_index: usize,
    /// Most recently completed action_id (for prerequisite checks).
    pub last_completed_action_id: Option<String>,
    /// Current execution state (idle / animating / charging).
    pub animation: AnimationState,
    /// True when this track is blocked on a prerequisite.
    pub waiting_for_prerequisite: bool,
}

impl TrackState {
    fn from_track(track: &Track) -> Self {
        Self {
            track_id: track.track_id.clone(),
            char_id: track.char_id.clone(),
            actions: track.actions.clone(),
            next_action_index: 0,
            last_completed_action_id: None,
            animation: AnimationState::Idle,
            waiting_for_prerequisite: false,
        }
    }

    /// Returns true when all actions in this track have been dispatched.
    pub fn is_exhausted(&self) -> bool {
        self.next_action_index >= self.actions.len()
            && matches!(self.animation, AnimationState::Idle)
    }

    /// Returns a reference to the current (action_id, at_tick), if any.
    pub fn current_action(&self) -> Option<&ActionEntry> {
        self.actions.get(self.next_action_index)
    }
}

// ---------------------------------------------------------------------------
// APL Manager
// ---------------------------------------------------------------------------

/// Manages APL track execution with full action lifecycle:
/// validate → deduct → start_skill → advance_frames → trigger_hit_frames → complete_skill
#[derive(Debug)]
pub struct APLManager {
    pub tracks: Vec<TrackState>,
    pub is_exhausted: bool,
    pending_actions: Vec<PendingAction>,
    validator: ResourceValidator,
}

impl APLManager {
    /// Create a new APL manager from an APL plan.
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

    /// Process all tracks for the given tick.
    ///
    /// Called once per simulation tick.  For each track:
    /// - **Idle**: checks if the next queued action is due, validates,
    ///   deducts resources, and starts execution (animating or charging).
    /// - **Animating**: advances frames, triggers hit frames.
    /// - **Charging**: advances charge, transitions to variant on completion.
    ///
    /// Returns validation errors for actions that failed resource checks.
    /// Richer lifecycle events can be drained via [`get_pending_actions`].
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
            // -------- Animating / Charging processing (no skill_lookup needed) --------
            match &self.tracks[track_idx].animation {
                AnimationState::Animating { .. } => {
                    let state = std::mem::replace(
                        &mut self.tracks[track_idx].animation,
                        AnimationState::Idle,
                    );
                    if let AnimationState::Animating {
                        action_id,
                        total_frames,
                        mut elapsed_frames,
                        mut hit_frames,
                    } = state
                    {
                        elapsed_frames += 1;

                        if elapsed_frames > total_frames {
                            self.pending_actions.push(PendingAction::ActionCompleted {
                                char_id: self.tracks[track_idx].char_id.clone(),
                                action_id: action_id.clone(),
                            });
                            // State stays Idle (from replace).
                            continue;
                        }

                        // Emit hit frames at the current elapsed position.
                        let mut remaining = Vec::new();
                        for (frame, mult) in hit_frames {
                            if frame == elapsed_frames {
                                self.pending_actions.push(PendingAction::HitFrameTriggered {
                                    char_id: self.tracks[track_idx].char_id.clone(),
                                    action_id: action_id.clone(),
                                    frame,
                                    multiplier: mult,
                                });
                            } else {
                                remaining.push((frame, mult));
                            }
                        }

                        self.tracks[track_idx].animation = AnimationState::Animating {
                            action_id,
                            total_frames,
                            elapsed_frames,
                            hit_frames: remaining,
                        };
                    }
                    continue;
                }
                AnimationState::Charging { .. } => {
                    let state = std::mem::replace(
                        &mut self.tracks[track_idx].animation,
                        AnimationState::Idle,
                    );
                    if let AnimationState::Charging {
                        action_id,
                        charge_elapsed,
                        charge_duration,
                        variant_action_id,
                    } = state
                    {
                        let new_elapsed = charge_elapsed + 1;
                        if new_elapsed >= charge_duration {
                            // Charge complete — look up variant and start animation.
                            if let Some(variant_skill) = skills.get(&variant_action_id) {
                                let variant_frames: Vec<(u64, f64)> = variant_skill
                                    .damage_multipliers
                                    .iter()
                                    .map(|hf| (hf.frame, hf.multiplier))
                                    .collect();

                                Self::emit_frame1_hits(
                                    &mut self.pending_actions,
                                    &self.tracks[track_idx].char_id,
                                    &variant_action_id,
                                    &variant_frames,
                                );

                                self.tracks[track_idx].animation = AnimationState::Animating {
                                    action_id: variant_action_id.clone(),
                                    total_frames: variant_skill.animation_frames,
                                    elapsed_frames: 1,
                                    hit_frames: variant_frames,
                                };

                                self.pending_actions.push(PendingAction::ChargingCompleted {
                                    char_id: self.tracks[track_idx].char_id.clone(),
                                    action_id: action_id.clone(),
                                    variant_action_id: variant_action_id.clone(),
                                });
                                self.pending_actions.push(PendingAction::ActionStarted {
                                    char_id: self.tracks[track_idx].char_id.clone(),
                                    action_id: variant_action_id.clone(),
                                });
                            } else {
                                self.pending_actions.push(PendingAction::ActionFailed {
                                    char_id: self.tracks[track_idx].char_id.clone(),
                                    action_id: variant_action_id.clone(),
                                    reason: format!("Variant '{}' not found", variant_action_id),
                                });
                            }
                        } else {
                            self.tracks[track_idx].animation = AnimationState::Charging {
                                action_id,
                                charge_elapsed: new_elapsed,
                                charge_duration,
                                variant_action_id,
                            };
                        }
                    }
                    continue;
                }
                AnimationState::Idle => { /* fall through to idle processing below */ }
            }

            // -------- Idle processing (start next queued action) --------
            let track_exhausted = self.tracks[track_idx].is_exhausted();
            if track_exhausted {
                continue;
            }

            // Read the current action entry.
            let entry = {
                let track = &self.tracks[track_idx];
                match track.actions.get(track.next_action_index) {
                    Some(e) => e.clone(),
                    None => continue,
                }
            };

            // Not time yet.
            if entry.at > current_tick {
                continue;
            }

            // Look up skill data.
            let skill = match skills.get(&entry.action_id) {
                Some(s) => s.clone(),
                None => {
                    self.tracks[track_idx].next_action_index += 1;
                    self.pending_actions.push(PendingAction::ActionFailed {
                        char_id: self.tracks[track_idx].char_id.clone(),
                        action_id: entry.action_id.clone(),
                        reason: format!("Unknown action: {}", entry.action_id),
                    });
                    continue;
                }
            };

            // Check prerequisite (read-only first).
            let prereq_blocked = {
                let track = &self.tracks[track_idx];
                if let Some(ref prereq) = skill.prerequisite_action_id {
                    track.last_completed_action_id.as_ref() != Some(prereq)
                } else {
                    false
                }
            };
            if prereq_blocked {
                self.tracks[track_idx].waiting_for_prerequisite = true;
                continue;
            }

            let char_id = self.tracks[track_idx].char_id.clone();

            // Find character in team by char_id.
            let char_idx = match team.characters.iter().position(|c| c.char_id == char_id) {
                Some(idx) => idx,
                None => {
                    self.pending_actions.push(PendingAction::ActionFailed {
                        char_id: char_id.clone(),
                        action_id: entry.action_id.clone(),
                        reason: format!("Character '{}' not in team", char_id),
                    });
                    self.tracks[track_idx].next_action_index += 1;
                    continue;
                }
            };

            // Validate resources.
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

            // Deduct decibel (only if on-field and skill costs decibel).
            if skill.decibel_cost > 0.0 && team.current_on_field_index == char_idx {
                let _ = team.consume_decibel(skill.decibel_cost);
            }

            // Deduct energy and HP.
            {
                let character = &mut team.characters[char_idx];
                character.resources.energy =
                    (character.resources.energy - skill.energy_cost).max(0.0);
                character.current_stats.hp = (character.current_stats.hp - skill.hp_cost).max(0.0);
            }

            // Set skill cooldown.
            self.validator
                .set_cooldown(&skill.action_id, skill.cooldown_ticks, current_tick);

            // Mark action as "completed" for prerequisite tracking
            // (happens at start, not at animation end).
            {
                let track = &mut self.tracks[track_idx];
                track.last_completed_action_id = Some(entry.action_id.clone());
            }

            // Handle charge branches vs. immediate animation.
            if !skill.charge_branches.is_empty() {
                let branch = &skill.charge_branches[0];
                self.tracks[track_idx].animation = AnimationState::Charging {
                    action_id: skill.action_id.clone(),
                    charge_elapsed: 0,
                    charge_duration: branch.charge_duration,
                    variant_action_id: branch.variant_action_id.clone(),
                };
                self.pending_actions.push(PendingAction::ChargingStarted {
                    char_id: char_id.clone(),
                    action_id: entry.action_id.clone(),
                    charge_duration: branch.charge_duration,
                });
            } else if skill.animation_frames <= 1 {
                // Instant skill (≤1 frame): start + complete in the same tick.
                let hit_frames: Vec<(u64, f64)> = skill
                    .damage_multipliers
                    .iter()
                    .map(|hf| (hf.frame, hf.multiplier))
                    .collect();

                Self::emit_frame1_hits(
                    &mut self.pending_actions,
                    &char_id,
                    &entry.action_id,
                    &hit_frames,
                );

                self.pending_actions.push(PendingAction::ActionStarted {
                    char_id: char_id.clone(),
                    action_id: entry.action_id.clone(),
                });
                if skill.animation_frames == 1 {
                    // Hit frames at frame 1 were already emitted above.
                    // Emit any additional hit frames (none in practice for 1-frame).
                }
                self.pending_actions.push(PendingAction::ActionCompleted {
                    char_id,
                    action_id: entry.action_id.clone(),
                });
                // Track stays Idle (default after the take).
            } else {
                // Multi-frame animation: start the animation timeline.
                let hit_frames: Vec<(u64, f64)> = skill
                    .damage_multipliers
                    .iter()
                    .map(|hf| (hf.frame, hf.multiplier))
                    .collect();

                Self::emit_frame1_hits(
                    &mut self.pending_actions,
                    &char_id,
                    &entry.action_id,
                    &hit_frames,
                );

                self.tracks[track_idx].animation = AnimationState::Animating {
                    action_id: skill.action_id.clone(),
                    total_frames: skill.animation_frames,
                    elapsed_frames: 1,
                    hit_frames,
                };
                self.pending_actions.push(PendingAction::ActionStarted {
                    char_id,
                    action_id: entry.action_id.clone(),
                });
            }

            self.tracks[track_idx].next_action_index += 1;
        }

        // Refresh the exhausted flag.
        self.is_exhausted = self.all_tracks_exhausted();

        errors
    }

    /// Emit hit frame events for frame-1 hits (immediate hits on skill start).
    fn emit_frame1_hits(
        pending: &mut Vec<PendingAction>,
        char_id: &str,
        action_id: &str,
        hit_frames: &[(u64, f64)],
    ) {
        for &(frame, mult) in hit_frames {
            if frame == 1 {
                pending.push(PendingAction::HitFrameTriggered {
                    char_id: char_id.to_string(),
                    action_id: action_id.to_string(),
                    frame,
                    multiplier: mult,
                });
            }
        }
    }

    /// Drain all pending actions (consuming queue pattern).
    pub fn get_pending_actions(&mut self) -> Vec<PendingAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Mutable reference to the internal resource validator.
    pub fn validator_mut(&mut self) -> &mut ResourceValidator {
        &mut self.validator
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

    fn make_enemy() -> EnemyState {
        let mut enemy = EnemyState::new("test_enemy", EnemyType::Elite, 100.0);
        enemy.hp = 50000.0;
        enemy.resistances.insert(ElementTag::Fire, 0.5);
        enemy
    }

    fn make_skills_map(skills: Vec<SkillData>) -> HashMap<String, SkillData> {
        skills
            .into_iter()
            .map(|s| (s.action_id.clone(), s))
            .collect()
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
                    ActionEntry {
                        action_id: "atk".into(),
                        at: 0,
                    },
                    ActionEntry {
                        action_id: "skill".into(),
                        at: 30,
                    },
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
            actions: vec![ActionEntry {
                action_id: "a1".into(),
                at: 0,
            }],
        });
        assert!(!track.is_exhausted());
    }

    #[test]
    fn test_track_exhausted_when_done() {
        let mut track = TrackState::from_track(&Track {
            track_id: "t1".into(),
            char_id: "c1".into(),
            actions: vec![ActionEntry {
                action_id: "a1".into(),
                at: 0,
            }],
        });
        track.next_action_index = 1;
        assert!(track.is_exhausted());
    }

    #[test]
    fn test_track_current_action() {
        let track = TrackState::from_track(&Track {
            track_id: "t1".into(),
            char_id: "c1".into(),
            actions: vec![ActionEntry {
                action_id: "a1".into(),
                at: 0,
            }],
        });
        let entry = track.current_action().unwrap();
        assert_eq!(entry.action_id, "a1");
        assert_eq!(entry.at, 0);
    }

    #[test]
    fn test_track_current_action_none_when_exhausted() {
        let track = TrackState::from_track(&Track {
            track_id: "t1".into(),
            char_id: "c1".into(),
            actions: vec![],
        });
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
                actions: vec![ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);

        let actions = mgr.get_pending_actions();
        assert!(!actions.is_empty());
        // Should contain ActionStarted for normal_atk
        assert!(actions.iter().any(|pa| matches!(pa,
            PendingAction::ActionStarted { action_id, .. } if action_id == "normal_atk"
        )));
    }

    #[test]
    fn test_action_not_dispatched_before_scheduled_tick() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 30,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // Tick 0 — not dispatched yet
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert!(mgr.get_pending_actions().is_empty());

        // Tick 30 — should dispatch
        let errors = mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert!(!mgr.get_pending_actions().is_empty());
    }

    #[test]
    fn test_multiple_actions_in_sequence() {
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
                        action_id: "ex_skill".into(),
                        at: 30,
                    },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // Tick 0: dispatch normal_atk
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(!mgr.get_pending_actions().is_empty());

        // Tick 30: dispatch ex_skill
        let errors = mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let actions = mgr.get_pending_actions();
        assert!(actions.iter().any(|pa| matches!(pa,
            PendingAction::ActionStarted { action_id, .. } if action_id == "ex_skill"
        )));
    }

    #[test]
    fn test_track_exhausted_after_all_actions_dispatched() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 0,
                }],
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
                actions: vec![ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 0,
                }],
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
    // Animation and hit frame tests
    // ------------------------------------------------------------------

    #[test]
    fn test_animation_advances_and_completes() {
        let skill = SkillData {
            action_id: "anim_test".to_string(),
            action_type: SkillType::Normal,
            animation_frames: 5,
            ..SkillData::default_for_test()
        };
        let skills = make_skills_map(vec![skill]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "anim_test".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        // Tick 0: start animation
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionStarted { .. })));

        // Tick 1-4: animating (calls 2-5: elapsed 2→3→4→5)
        for _ in 0..4 {
            mgr.process_next_action(0, &mut team, &skills, &enemies);
            assert!(mgr.get_pending_actions().is_empty());
        }

        // Tick 5 → elapsed=6 > 5 → ActionCompleted
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionCompleted { .. })));
    }

    #[test]
    fn test_hit_frame_triggers_at_correct_frame() {
        let skill = SkillData {
            action_id: "multi_hit".to_string(),
            action_type: SkillType::Normal,
            damage_multipliers: vec![
                crate::combat::skill::HitFrame {
                    frame: 3,
                    multiplier: 1.5,
                },
                crate::combat::skill::HitFrame {
                    frame: 7,
                    multiplier: 2.0,
                },
            ],
            animation_frames: 10,
            ..SkillData::default_for_test()
        };
        let skills = make_skills_map(vec![skill]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "multi_hit".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        // Tick 0: start (elapsed=1)
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionStarted { .. })));

        // elapsed=2 (no hit)
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(mgr.get_pending_actions().is_empty());

        // elapsed=3 → hit frame 3!
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert_eq!(pa.len(), 1);
        assert!(matches!(
            pa[0],
            PendingAction::HitFrameTriggered {
                frame: 3,
                multiplier: 1.5,
                ..
            }
        ));

        // elapsed=4,5,6 (no hits)
        for _ in 0..3 {
            mgr.process_next_action(0, &mut team, &skills, &enemies);
            assert!(mgr.get_pending_actions().is_empty());
        }

        // elapsed=7 → hit frame 7!
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert_eq!(pa.len(), 1);
        assert!(matches!(
            pa[0],
            PendingAction::HitFrameTriggered {
                frame: 7,
                multiplier: 2.0,
                ..
            }
        ));
    }

    // ------------------------------------------------------------------
    // Charge branch tests
    // ------------------------------------------------------------------

    #[test]
    fn test_charge_branch_starts_charging() {
        let skill = SkillData {
            action_id: "hold_atk".to_string(),
            action_type: SkillType::Special,
            charge_branches: vec![crate::combat::skill::ChargeBranch {
                charge_duration: 30,
                variant_action_id: "hold_atk_charged".to_string(),
            }],
            ..SkillData::default_for_test()
        };
        let skills = make_skills_map(vec![skill]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "hold_atk".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert_eq!(pa.len(), 1);
        assert!(matches!(
            pa[0],
            PendingAction::ChargingStarted { ref action_id, charge_duration: 30, .. }
                if action_id == "hold_atk"
        ));
    }

    #[test]
    fn test_charge_complete_triggers_variant() {
        let skill = SkillData {
            action_id: "hold_atk".to_string(),
            action_type: SkillType::Special,
            charge_branches: vec![crate::combat::skill::ChargeBranch {
                charge_duration: 5,
                variant_action_id: "hold_atk_charged".to_string(),
            }],
            ..SkillData::default_for_test()
        };
        let variant = SkillData {
            action_id: "hold_atk_charged".to_string(),
            action_type: SkillType::Special,
            animation_frames: 3,
            ..SkillData::default_for_test()
        };
        let skills = make_skills_map(vec![skill, variant]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "hold_atk".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        // Start charging.
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        // Advance through charge (4 ticks of charging).
        for _ in 0..4 {
            mgr.process_next_action(0, &mut team, &skills, &enemies);
            assert!(mgr.get_pending_actions().is_empty());
        }

        // 5th tick → charge complete, variant starts.
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa.iter().any(|pa| matches!(pa,
            PendingAction::ChargingCompleted { ref variant_action_id, .. }
                if variant_action_id == "hold_atk_charged"
        )));
        assert!(pa.iter().any(|pa| matches!(pa,
            PendingAction::ActionStarted { ref action_id, .. }
                if action_id == "hold_atk_charged"
        )));
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
                actions: vec![ActionEntry {
                    action_id: "follow_up".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        // Prerequisite "normal_atk" not completed yet — blocked
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert!(mgr.get_pending_actions().is_empty());
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
                    ActionEntry {
                        action_id: "normal_atk".into(),
                        at: 0,
                    },
                    ActionEntry {
                        action_id: "follow_up".into(),
                        at: 10,
                    },
                ],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        // Tick 0: dispatch normal_atk
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(!mgr.get_pending_actions().is_empty());

        // Tick 10: follow_up should now be eligible
        let errors = mgr.process_next_action(10, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let actions = mgr.get_pending_actions();
        assert!(actions.iter().any(|pa| matches!(pa,
            PendingAction::ActionStarted { action_id, .. } if action_id == "follow_up"
        )));
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
                actions: vec![ActionEntry {
                    action_id: "ex_skill".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 10.0; // ex_skill costs 30
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].missing_resource,
            crate::combat::validator::ResourceType::Energy
        );
        // Action not dispatched
        assert!(mgr.get_pending_actions().is_empty());
    }

    #[test]
    fn test_energy_deducted_on_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "ex_skill".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 50.0;
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        assert!(
            (team.characters[0].resources.energy - 20.0).abs() < 1e-9,
            "expected 20 energy remaining"
        );
    }

    #[test]
    fn test_hp_deducted_on_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "hp_cost_skill".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].current_stats.hp = 8000.0;
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        assert!(
            (team.characters[0].current_stats.hp - 7500.0).abs() < 1e-9,
            "expected 7500 hp remaining"
        );
    }

    #[test]
    fn test_insufficient_hp_blocks_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "hp_cost_skill".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].current_stats.hp = 300.0; // hp_cost_skill costs 500
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].missing_resource,
            crate::combat::validator::ResourceType::Hp
        );
        assert!(mgr.get_pending_actions().is_empty());
    }

    // ------------------------------------------------------------------
    // Cooldown tracking
    // ------------------------------------------------------------------

    #[test]
    fn test_cooldown_prevents_immediate_reuse() {
        let skills = make_skills_map(vec![make_skill(
            "ex_skill",
            SkillType::Special,
            10.0,
            0.0,
            0.0,
            30,
        )]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry {
                        action_id: "ex_skill".into(),
                        at: 0,
                    },
                    ActionEntry {
                        action_id: "ex_skill".into(),
                        at: 10,
                    },
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
        assert_eq!(
            errors[0].missing_resource,
            crate::combat::validator::ResourceType::SkillCooldown
        );
    }

    #[test]
    fn test_cooldown_expires_and_allows_reuse() {
        let skills = make_skills_map(vec![make_skill(
            "ex_skill",
            SkillType::Special,
            10.0,
            0.0,
            0.0,
            20,
        )]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![
                    ActionEntry {
                        action_id: "ex_skill".into(),
                        at: 0,
                    },
                    ActionEntry {
                        action_id: "ex_skill".into(),
                        at: 25,
                    },
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
        assert!(!mgr.get_pending_actions().is_empty());
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
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());

        let actions = mgr.get_pending_actions();
        // Each track produces ActionStarted + ActionCompleted (4 total for 2 tracks).
        assert_eq!(actions.len(), 4);
        // Both tracks should have started normal_atk
        let sources: Vec<&str> = actions
            .iter()
            .filter_map(|pa| {
                if let PendingAction::ActionStarted { char_id, .. } = pa {
                    Some(char_id.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(sources.contains(&"char_0"));
        assert!(sources.contains(&"char_1"));
    }

    #[test]
    fn test_multi_track_independent_ticks() {
        let apl = APLData {
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
                        at: 50,
                    }],
                },
            ],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // Tick 0: only track 1 dispatches (ActionStarted + ActionCompleted = 2 events)
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 2);

        // Tick 30: track 2 not ready yet
        mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert!(mgr.get_pending_actions().is_empty());

        // Tick 50: track 2 dispatches (ActionStarted + ActionCompleted = 2 events)
        let errors = mgr.process_next_action(50, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let actions = mgr.get_pending_actions();
        assert_eq!(actions.len(), 2);
        assert!(actions.iter().any(|pa| matches!(pa,
            PendingAction::ActionStarted { char_id, .. } if char_id == "char_1"
        )));
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
                actions: vec![ActionEntry {
                    action_id: "nonexistent_skill".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills: HashMap<String, SkillData> = HashMap::new();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        // ActionFailed event emitted for unknown skill
        let pending = mgr.get_pending_actions();
        assert_eq!(pending.len(), 1);
        assert!(
            matches!(pending[0], PendingAction::ActionFailed { ref action_id, .. } if action_id == "nonexistent_skill")
        );
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
                actions: vec![ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        // ActionFailed event emitted for unknown character
        let pending = mgr.get_pending_actions();
        assert_eq!(pending.len(), 1);
        assert!(
            matches!(pending[0], PendingAction::ActionFailed { ref char_id, .. } if char_id == "nonexistent_char")
        );
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
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let first = mgr.get_pending_actions();
        assert!(!first.is_empty());
        assert!(mgr.get_pending_actions().is_empty());

        mgr.process_next_action(10, &mut team, &skills, &enemies);
        assert!(!mgr.get_pending_actions().is_empty());
    }

    // ------------------------------------------------------------------
    // Decibel deduction for on-field character
    // ------------------------------------------------------------------

    #[test]
    fn test_decibel_deducted_when_on_field() {
        let skills = make_skills_map(vec![make_skill(
            "ultimate",
            SkillType::Ultimate,
            0.0,
            0.0,
            1500.0,
            0,
        )]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "ultimate".into(),
                    at: 0,
                }],
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

    // ------------------------------------------------------------------
    // Integration: full multi-track with resource tracking
    // ------------------------------------------------------------------

    #[test]
    fn test_full_multi_track_integration() {
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
                        ActionEntry {
                            action_id: "normal_atk".into(),
                            at: 0,
                        },
                        ActionEntry {
                            action_id: "ex_skill".into(),
                            at: 20,
                        },
                        ActionEntry {
                            action_id: "normal_atk".into(),
                            at: 50,
                        },
                    ],
                },
                Track {
                    track_id: "t2".into(),
                    char_id: "char_1".into(),
                    actions: vec![
                        ActionEntry {
                            action_id: "normal_atk".into(),
                            at: 0,
                        },
                        ActionEntry {
                            action_id: "normal_atk".into(),
                            at: 30,
                        },
                    ],
                },
            ],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        team.characters[0].resources.energy = 100.0;
        let enemies = vec![make_enemy()];

        // Tick 0: both tracks dispatch normal_atk (2 events × 2 tracks = 4)
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 4);

        // Tick 20: char_0 dispatches ex_skill (costs 30 energy, 2 events)
        let errors = mgr.process_next_action(20, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 2);
        assert!((team.characters[0].resources.energy - 70.0).abs() < 1e-9);

        // Tick 30: char_1 dispatches second normal_atk (2 events)
        let errors = mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 2);

        // Tick 50: char_0 dispatches final normal_atk (2 events)
        let errors = mgr.process_next_action(50, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 2);

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
                actions: vec![ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 0,
                }],
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
                    ActionEntry {
                        action_id: "ex_skill".into(),
                        at: 0,
                    },
                    ActionEntry {
                        action_id: "normal_atk".into(),
                        at: 10,
                    },
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
        assert!(mgr.get_pending_actions().is_empty());

        // Track did NOT advance — still on ex_skill
        assert_eq!(mgr.tracks[0].next_action_index, 0);

        // Give more energy and retry at tick 10
        team.characters[0].resources.energy = 100.0;
        let errors = mgr.process_next_action(10, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let actions = mgr.get_pending_actions();
        assert!(actions.iter().any(|pa| matches!(pa,
            PendingAction::ActionStarted { action_id, .. } if action_id == "ex_skill"
        )));
    }

    #[test]
    fn test_same_tick_no_double_dispatch() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "normal_atk".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // First call at tick 0
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(!mgr.get_pending_actions().is_empty());

        // Second call at same tick: track already advanced, nothing new
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(mgr.get_pending_actions().is_empty());
    }

    // ------------------------------------------------------------------
    // Validator access
    // ------------------------------------------------------------------

    #[test]
    fn test_validator_mut_access() {
        let apl = APLData { tracks: vec![] };
        let mut mgr = APLManager::new(apl);
        let v = mgr.validator_mut();
        v.set_cooldown("test", 10, 0);
        assert_eq!(v.remaining_cooldown("test", 5), 5);
    }

    // ------------------------------------------------------------------
    // Edge cases
    // ------------------------------------------------------------------

    #[test]
    fn test_action_failed_for_unknown_skill_emits_event() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "ghost_skill".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];
        let skills = make_skills_map(vec![]);

        let _ = mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa.iter().any(|pa| matches!(pa,
            PendingAction::ActionFailed { action_id, .. } if action_id == "ghost_skill"
        )));
    }

    #[test]
    fn test_full_action_lifecycle_integration() {
        // A skill with known hit frames: test start → hit → complete.
        let skill = SkillData {
            action_id: "combo_hit".to_string(),
            action_type: SkillType::Normal,
            damage_multipliers: vec![crate::combat::skill::HitFrame {
                frame: 2,
                multiplier: 0.8,
            }],
            animation_frames: 4,
            ..SkillData::default_for_test()
        };
        let skills = make_skills_map(vec![skill]);

        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_0".into(),
                actions: vec![ActionEntry {
                    action_id: "combo_hit".into(),
                    at: 0,
                }],
            }],
        };
        let mut mgr = APLManager::new(apl);
        let mut team = make_team();
        let enemies = vec![make_enemy()];

        // Start
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionStarted { .. })));

        // elapsed=2 → hit
        mgr.process_next_action(0, &mut team, &skills, &enemies); // elapsed=1→2
        let pa = mgr.get_pending_actions();
        assert!(pa.iter().any(|pa| matches!(
            pa,
            PendingAction::HitFrameTriggered {
                frame: 2,
                multiplier: 0.8,
                ..
            }
        )));

        // elapsed=3, 4 (2 calls, no hit)
        for _ in 0..2 {
            mgr.process_next_action(0, &mut team, &skills, &enemies);
        }
        assert!(mgr.get_pending_actions().is_empty());

        // elapsed=5 > 4 → complete
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionCompleted { .. })));
    }
}
