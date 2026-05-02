//! APL 动作队列管理器 —— 安排并执行每个角色的技能
//! 循环（轨道），提供完整的动作生命周期支持。

use std::collections::HashMap;

use crate::combat::skill::SkillData;
use crate::combat::team::TeamManager;
use crate::combat::validator::{ResourceValidator, ValidationError};
use crate::data::apl::{APLData, ActionEntry, Track};
use crate::entities::enemy::EnemyState;

/// 一个已验证并分派的动作，准备由模拟运行器执行。
///
/// 被 [`crate::combat::coordinated::CoordinatedActionSystem`] 和其他
/// 消费 APL 分派事件的系统使用。
#[derive(Debug, Clone, PartialEq)]
pub struct SkillAction {
    pub action_id: String,
    pub source_id: String,
    pub target_id: String,
    pub started_at_tick: u64,
}

// ---------------------------------------------------------------------------
// 事件类型
// ---------------------------------------------------------------------------

/// APL 管理器在动作执行过程中产生的事件。
///
/// 模拟运行器通过 [`APLManager::get_pending_actions`] 清空这些事件。
#[derive(Debug, Clone, PartialEq)]
pub enum PendingAction {
    /// 技能开始执行（动画开始）。
    ActionStarted { char_id: String, action_id: String },
    /// 技能执行期间达到了命中帧。
    HitFrameTriggered {
        char_id: String,
        action_id: String,
        frame: u64,
        multiplier: f64,
    },
    /// 技能的动画完成。
    ActionCompleted { char_id: String, action_id: String },
    /// 技能验证失败或找不到。
    ActionFailed {
        char_id: String,
        action_id: String,
        reason: String,
    },
    /// 可蓄力技能开始蓄力。
    ChargingStarted {
        char_id: String,
        action_id: String,
        charge_duration: u64,
    },
    /// 蓄力完成，变体动作开始。
    ChargingCompleted {
        char_id: String,
        action_id: String,
        variant_action_id: String,
    },
}

// ---------------------------------------------------------------------------
// 动画状态
// ---------------------------------------------------------------------------

/// 每个轨道的执行状态。
#[derive(Debug, Clone)]
pub enum AnimationState {
    /// 当前没有动作在执行。
    Idle,
    /// 技能正在播放动画。
    Animating {
        action_id: String,
        total_frames: u64,
        elapsed_frames: u64,
        /// 剩余的待触发命中帧：(帧号, 倍率)。
        hit_frames: Vec<(u64, f64)>,
    },
    /// 可蓄力动作正在蓄力。
    Charging {
        action_id: String,
        charge_elapsed: u64,
        charge_duration: u64,
        variant_action_id: String,
    },
}

// ---------------------------------------------------------------------------
// 轨道状态
// ---------------------------------------------------------------------------

/// 单个 APL 轨道的运行时状态。
#[derive(Debug, Clone)]
pub struct TrackState {
    pub track_id: String,
    pub char_id: String,
    /// 剩余动作：(action_id, scheduled_tick)。
    actions: Vec<ActionEntry>,
    /// `actions` 中下一个待检查动作的索引。
    pub next_action_index: usize,
    /// 最近完成的 action_id（用于前置条件检查）。
    pub last_completed_action_id: Option<String>,
    /// 当前执行状态（空闲/动画中/蓄力中）。
    pub animation: AnimationState,
    /// 当此轨道因前置条件被阻塞时为 true。
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

    /// 当此轨道中的所有动作都已分派时返回 true。
    pub fn is_exhausted(&self) -> bool {
        self.next_action_index >= self.actions.len()
            && matches!(self.animation, AnimationState::Idle)
    }

    /// 返回当前 (action_id, at_tick) 的引用（如果有的话）。
    pub fn current_action(&self) -> Option<&ActionEntry> {
        self.actions.get(self.next_action_index)
    }
}

// ---------------------------------------------------------------------------
// APL 管理器
// ---------------------------------------------------------------------------

/// 管理 APL 轨道执行，包含完整的动作生命周期：
/// 验证 → 扣除 → 开始技能 → 推进帧 → 触发命中帧 → 完成技能
#[derive(Debug)]
pub struct APLManager {
    pub tracks: Vec<TrackState>,
    pub is_exhausted: bool,
    pending_actions: Vec<PendingAction>,
    validator: ResourceValidator,
}

impl APLManager {
    /// 从 APL 计划创建一个新的 APL 管理器。
    pub fn new(apl_data: APLData) -> Self {
        let tracks: Vec<TrackState> = apl_data.tracks.iter().map(TrackState::from_track).collect();
        Self {
            tracks,
            pending_actions: Vec::new(),
            is_exhausted: false,
            validator: ResourceValidator::new(),
        }
    }

    /// 当每个轨道都已分派其所有动作时返回 true。
    pub fn all_tracks_exhausted(&self) -> bool {
        self.tracks.iter().all(|t| t.is_exhausted())
    }

    /// 处理给定 tick 的所有轨道。
    ///
    /// 每个模拟 tick 调用一次。对于每个轨道：
    /// - **空闲**：检查下一个排队动作是否到期，验证，
    ///   扣除资源，开始执行（动画或蓄力）。
    /// - **动画中**：推进帧，触发命中帧。
    /// - **蓄力中**：推进蓄力，完成后转换为变体。
    ///
    /// 返回资源检查失败的动作的验证错误。
    /// 更丰富的生命周期事件可以通过 [`get_pending_actions`] 清空。
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
            // -------- 动画中 / 蓄力中处理（无需查找技能） --------
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
                            // 状态保持为空闲（来自 replace 的结果）。
                            continue;
                        }

                        // 在当前经过的帧位置发射命中帧。
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
                            // 蓄力完成 — 查找变体并开始动画。
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
                AnimationState::Idle => { /* 继续执行下面的空闲处理 */ }
            }

            // -------- 空闲处理（开始下一个排队动作） --------
            let track_exhausted = self.tracks[track_idx].is_exhausted();
            if track_exhausted {
                continue;
            }

            // 读取当前动作条目。
            let entry = {
                let track = &self.tracks[track_idx];
                match track.actions.get(track.next_action_index) {
                    Some(e) => e.clone(),
                    None => continue,
                }
            };

            // 时间还没到。
            if entry.at > current_tick {
                continue;
            }

            // 查找技能数据。
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

            // 检查前置条件（先只读检查）。
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

            // 通过 char_id 在队伍中查找角色。
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

            // 验证资源。
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

            // 扣除 Decibel（仅当角色在场且技能消耗 Decibel 时）。
            if skill.decibel_cost > 0.0 && team.current_on_field_index == char_idx {
                let _ = team.consume_decibel(skill.decibel_cost);
            }

            // 扣除能量和 HP。
            {
                let character = &mut team.characters[char_idx];
                character.resources.energy =
                    (character.resources.energy - skill.energy_cost).max(0.0);
                character.current_stats.hp = (character.current_stats.hp - skill.hp_cost).max(0.0);
            }

            // 设置技能冷却。
            self.validator
                .set_cooldown(&skill.action_id, skill.cooldown_ticks, current_tick);

            // 将动作标记为"已完成"用于前置条件追踪
            // （在开始时发生，而非动画结束时）。
            {
                let track = &mut self.tracks[track_idx];
                track.last_completed_action_id = Some(entry.action_id.clone());
            }

            // 处理蓄力分支与即时动画。
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
                // 即时技能（<=1 帧）：在同一 tick 内开始 + 完成。
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
                    // 第 1 帧的命中帧已在上面发射。
                    // 发射任何额外的命中帧（对于 1 帧技能实际没有）。
                }
                self.pending_actions.push(PendingAction::ActionCompleted {
                    char_id,
                    action_id: entry.action_id.clone(),
                });
                // 轨道保持空闲（take 后的默认状态）。
            } else {
                // 多帧动画：启动动画时间线。
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

        // 刷新耗尽标志。
        self.is_exhausted = self.all_tracks_exhausted();

        errors
    }

    /// 发射第 1 帧命中帧事件（技能开始时的即时命中）。
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

    /// 清空所有待处理动作（消费队列模式）。
    pub fn get_pending_actions(&mut self) -> Vec<PendingAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// 内部资源验证器的可变引用。
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
            animation_frames: 1, // 默认：即时完成
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
    // APLManager 创建
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
    // TrackState 辅助函数
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
    // 动作分派
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
        // 应包含 normal_atk 的 ActionStarted
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

        // Tick 0 — 尚未分派
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert!(mgr.get_pending_actions().is_empty());

        // Tick 30 — 应分派
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

        // Tick 0：分派 normal_atk
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(!mgr.get_pending_actions().is_empty());

        // Tick 30：分派 ex_skill
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

        // 第二次调用：已耗尽，无动作
        mgr.process_next_action(1, &mut team, &skills, &enemies);
        assert!(mgr.get_pending_actions().is_empty());
    }

    // ------------------------------------------------------------------
    // 动画与命中帧测试
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

        // Tick 0：开始动画
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionStarted { .. })));

        // Tick 1-4：动画中（调用 2-5：已过帧 2→3→4→5）
        for _ in 0..4 {
            mgr.process_next_action(0, &mut team, &skills, &enemies);
            assert!(mgr.get_pending_actions().is_empty());
        }

        // Tick 5 → 已过帧=6 > 5 → ActionCompleted
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

        // Tick 0：开始（已过帧=1）
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionStarted { .. })));

        // 已过帧=2（无命中）
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(mgr.get_pending_actions().is_empty());

        // 已过帧=3 → 命中帧 3！
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

        // 已过帧=4,5,6（无命中）
        for _ in 0..3 {
            mgr.process_next_action(0, &mut team, &skills, &enemies);
            assert!(mgr.get_pending_actions().is_empty());
        }

        // 已过帧=7 → 命中帧 7！
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
    // 蓄力分支测试
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

        // 开始蓄力。
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        // 推进蓄力（4 个蓄力 tick）。
        for _ in 0..4 {
            mgr.process_next_action(0, &mut team, &skills, &enemies);
            assert!(mgr.get_pending_actions().is_empty());
        }

        // 第 5 tick → 蓄力完成，变体技能开始
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
    // 前置条件检查
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

        // 前置条件 "normal_atk" 尚未完成 — 被阻止
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

        // Tick 0：分派 normal_atk
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(!mgr.get_pending_actions().is_empty());

        // Tick 10：follow_up 现在应可执行
        let errors = mgr.process_next_action(10, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let actions = mgr.get_pending_actions();
        assert!(actions.iter().any(|pa| matches!(pa,
            PendingAction::ActionStarted { action_id, .. } if action_id == "follow_up"
        )));
    }

    // ------------------------------------------------------------------
    // 资源验证与扣除
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
        team.characters[0].resources.energy = 10.0; // ex_skill 消耗 30 能量
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].missing_resource,
            crate::combat::validator::ResourceType::Energy
        );
        // 动作未分派
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
        team.characters[0].current_stats.hp = 300.0; // hp_cost_skill 消耗 500 HP
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
    // 冷却追踪
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
        team.characters[0].resources.energy = 100.0; // 足够两次施放
        let enemies = vec![make_enemy()];

        // tick 0 处首次使用 — 成功
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let _ = mgr.get_pending_actions();

        // tick 10 处第二次使用 — 被冷却阻止（从 tick 0 起 30 tick）
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

        // tick 0 处首次使用
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        // tick 25 处第二次使用 — 冷却已过期（20 tick）
        let errors = mgr.process_next_action(25, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert!(!mgr.get_pending_actions().is_empty());
    }

    // ------------------------------------------------------------------
    // 多轨道
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
        // 每个轨道产生 ActionStarted + ActionCompleted（2 个轨道共 4 个）。
        assert_eq!(actions.len(), 4);
        // 两个轨道都应已开始 normal_atk
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

        // Tick 0：只有轨道 1 分派（ActionStarted + ActionCompleted = 2 个事件）
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(mgr.get_pending_actions().len(), 2);

        // Tick 30：轨道 2 尚未就绪
        mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert!(mgr.get_pending_actions().is_empty());

        // Tick 50：轨道 2 分派（ActionStarted + ActionCompleted = 2 个事件）
        let errors = mgr.process_next_action(50, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        let actions = mgr.get_pending_actions();
        assert_eq!(actions.len(), 2);
        assert!(actions.iter().any(|pa| matches!(pa,
            PendingAction::ActionStarted { char_id, .. } if char_id == "char_1"
        )));
    }

    // ------------------------------------------------------------------
    // 未知/缺失的技能
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
        // 为未知技能发出 ActionFailed 事件
        let pending = mgr.get_pending_actions();
        assert_eq!(pending.len(), 1);
        assert!(
            matches!(pending[0], PendingAction::ActionFailed { ref action_id, .. } if action_id == "nonexistent_skill")
        );
        // 轨道应已耗尽（动作被跳过）
        assert!(mgr.all_tracks_exhausted());
    }

    // ------------------------------------------------------------------
    // 未知角色 ID
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
        // 为未知角色发出 ActionFailed 事件
        let pending = mgr.get_pending_actions();
        assert_eq!(pending.len(), 1);
        assert!(
            matches!(pending[0], PendingAction::ActionFailed { ref char_id, .. } if char_id == "nonexistent_char")
        );
    }

    // ------------------------------------------------------------------
    // get_pending_actions 是消费性的
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
    // 上场角色的 Decibel 扣除
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
        team.characters[0].resources.decibel = 2000.0; // char_0 在场
        let enemies = vec![make_enemy()];

        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let _ = mgr.get_pending_actions();

        assert!((team.characters[0].resources.decibel - 500.0).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // 集成：完整的带资源追踪的多轨道测试
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

        // Tick 0：两个轨道都分派 normal_atk（2 个事件 × 2 个轨道 = 4）
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 4);

        // Tick 20：char_0 分派 ex_skill（消耗 30 能量，2 个事件）
        let errors = mgr.process_next_action(20, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 2);
        assert!((team.characters[0].resources.energy - 70.0).abs() < 1e-9);

        // Tick 30：char_1 分派第二次 normal_atk（2 个事件）
        let errors = mgr.process_next_action(30, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 2);

        // Tick 50：char_0 分派最后的 normal_atk（2 个事件）
        let errors = mgr.process_next_action(50, &mut team, &skills, &enemies);
        assert!(errors.is_empty());
        assert_eq!(mgr.get_pending_actions().len(), 2);

        // 所有轨道已耗尽
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

        // 尝试再次处理 — 应为空操作
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
        team.characters[0].resources.energy = 5.0; // 能量不足，ex_skill 需要 30 能量
        let enemies = vec![make_enemy()];
        let skills = default_skills();

        // Tick 0：ex_skill 验证失败
        let errors = mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert_eq!(errors.len(), 1);
        assert!(mgr.get_pending_actions().is_empty());

        // 轨道未推进 — 仍在 ex_skill 上
        assert_eq!(mgr.tracks[0].next_action_index, 0);

        // 补充能量并在 tick 10 重试
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

        // 在 tick 0 处首次调用
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(!mgr.get_pending_actions().is_empty());

        // 在同一 tick 处第二次调用：轨道已推进，无新事件
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        assert!(mgr.get_pending_actions().is_empty());
    }

    // ------------------------------------------------------------------
    // 验证器访问
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
    // 边界情况
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
        // 一个具有已知命中帧的技能：测试开始 → 命中 → 完成。
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

        // 开始
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionStarted { .. })));

        // 已过帧=2 → 命中
        mgr.process_next_action(0, &mut team, &skills, &enemies); // 已过帧=1→2
        let pa = mgr.get_pending_actions();
        assert!(pa.iter().any(|pa| matches!(
            pa,
            PendingAction::HitFrameTriggered {
                frame: 2,
                multiplier: 0.8,
                ..
            }
        )));

        // 已过帧=3, 4（2 次调用，无命中）
        for _ in 0..2 {
            mgr.process_next_action(0, &mut team, &skills, &enemies);
        }
        assert!(mgr.get_pending_actions().is_empty());

        // 已过帧=5 > 4 → 完成
        mgr.process_next_action(0, &mut team, &skills, &enemies);
        let pa = mgr.get_pending_actions();
        assert!(pa
            .iter()
            .any(|pa| matches!(pa, PendingAction::ActionCompleted { .. })));
    }
}
