use std::collections::HashMap;

/// Eight buff categories covering all combat stat domains.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BuffCategory {
    Stat,
    DamageBonus,
    Crit,
    Penetration,
    Anomaly,
    Stun,
    Resistance,
    Special,
}

/// How stacking behaves when the same buff is re-applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackType {
    /// New application replaces the existing buff entirely (refresh duration).
    Replace,
    /// Each application adds one stack up to max_stacks (refresh duration).
    Additive,
    /// Each application creates an independent instance (no stacking limit).
    Independent,
}

/// A single stat modifier within a buff.
#[derive(Debug, Clone, PartialEq)]
pub struct ModifierEntry {
    pub stat_name: String,
    pub value: f64,
}

impl ModifierEntry {
    pub fn new(stat_name: impl Into<String>, value: f64) -> Self {
        ModifierEntry {
            stat_name: stat_name.into(),
            value,
        }
    }
}

/// Template / blueprint for a buff loaded from data.
#[derive(Debug, Clone, PartialEq)]
pub struct BuffData {
    pub buff_id: String,
    pub category: BuffCategory,
    pub stack_type: StackType,
    pub max_stacks: u32,
    pub duration_ticks: u64,
    pub modifiers: Vec<ModifierEntry>,
}

impl BuffData {
    pub fn new(buff_id: impl Into<String>, category: BuffCategory, stack_type: StackType) -> Self {
        BuffData {
            buff_id: buff_id.into(),
            category,
            stack_type,
            max_stacks: 1,
            duration_ticks: 0,
            modifiers: Vec::new(),
        }
    }

    pub fn with_max_stacks(mut self, max_stacks: u32) -> Self {
        self.max_stacks = max_stacks.max(1);
        self
    }

    pub fn with_duration(mut self, ticks: u64) -> Self {
        self.duration_ticks = ticks;
        self
    }

    pub fn with_modifier(mut self, stat_name: impl Into<String>, value: f64) -> Self {
        self.modifiers
            .push(ModifierEntry::new(stat_name.into(), value));
        self
    }
}

/// An active buff instance on a character, tracking stacks and remaining time.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveBuff {
    pub data: BuffData,
    pub current_stacks: u32,
    pub remaining_ticks: u64,
    pub applied_at_tick: u64,
}

/// Aggregated effective stat modifiers for a character.
///
/// All values default to 0.0 — callers add the snapshot to base stats.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModifierSnapshot {
    pub atk_flat: f64,
    pub atk_pct: f64,
    pub def_flat: f64,
    pub def_pct: f64,
    pub hp_flat: f64,
    pub hp_pct: f64,
    pub crit_rate: f64,
    pub crit_dmg: f64,
    pub pen: f64,
    pub pen_ratio: f64,
    pub dmg_bonus: f64,
    pub anomaly_mastery: f64,
    pub anomaly_proficiency: f64,
    pub impact: f64,
    pub energy_regen: f64,
}

impl ModifierSnapshot {
    /// Sum two snapshots together (commutative).
    pub fn combine(&self, other: &ModifierSnapshot) -> ModifierSnapshot {
        ModifierSnapshot {
            atk_flat: self.atk_flat + other.atk_flat,
            atk_pct: self.atk_pct + other.atk_pct,
            def_flat: self.def_flat + other.def_flat,
            def_pct: self.def_pct + other.def_pct,
            hp_flat: self.hp_flat + other.hp_flat,
            hp_pct: self.hp_pct + other.hp_pct,
            crit_rate: self.crit_rate + other.crit_rate,
            crit_dmg: self.crit_dmg + other.crit_dmg,
            pen: self.pen + other.pen,
            pen_ratio: self.pen_ratio + other.pen_ratio,
            dmg_bonus: self.dmg_bonus + other.dmg_bonus,
            anomaly_mastery: self.anomaly_mastery + other.anomaly_mastery,
            anomaly_proficiency: self.anomaly_proficiency + other.anomaly_proficiency,
            impact: self.impact + other.impact,
            energy_regen: self.energy_regen + other.energy_regen,
        }
    }
}

/// Central buff manager holding active buffs keyed by character ID.
pub struct BuffManager {
    buffs: HashMap<String, Vec<ActiveBuff>>,
}

impl BuffManager {
    pub fn new() -> Self {
        BuffManager {
            buffs: HashMap::new(),
        }
    }

    /// Apply a buff to a character.  Stacking behaviour follows the buff's
    /// `stack_type`.  When the same `buff_id` already exists on the target
    /// the existing entry is updated; otherwise a new entry is pushed.
    pub fn apply_buff(&mut self, character_id: &str, data: BuffData, current_tick: u64) {
        let buff_list = self.buffs.entry(character_id.to_string()).or_default();

        match data.stack_type {
            StackType::Replace => {
                let duration = data.duration_ticks;
                // Remove any existing buff with the same ID, then push fresh.
                buff_list.retain(|b| b.data.buff_id != data.buff_id);
                buff_list.push(ActiveBuff {
                    data,
                    current_stacks: 1,
                    remaining_ticks: duration,
                    applied_at_tick: current_tick,
                });
            }
            StackType::Additive => {
                if let Some(existing) = buff_list
                    .iter_mut()
                    .find(|b| b.data.buff_id == data.buff_id)
                {
                    if existing.current_stacks < existing.data.max_stacks {
                        existing.current_stacks += 1;
                    }
                    existing.remaining_ticks = existing.data.duration_ticks;
                } else {
                    let duration = data.duration_ticks;
                    buff_list.push(ActiveBuff {
                        data,
                        current_stacks: 1,
                        remaining_ticks: duration,
                        applied_at_tick: current_tick,
                    });
                }
            }
            StackType::Independent => {
                let duration = data.duration_ticks;
                buff_list.push(ActiveBuff {
                    data,
                    current_stacks: 1,
                    remaining_ticks: duration,
                    applied_at_tick: current_tick,
                });
            }
        }
    }

    /// Remove all buffs whose remaining duration has reached zero.
    pub fn remove_expired(&mut self, _current_tick: u64) {
        for buff_list in self.buffs.values_mut() {
            buff_list.retain(|b| b.remaining_ticks > 0);
        }
    }

    /// Decrement remaining ticks for every active buff by one.
    /// Call once per simulation tick.
    pub fn on_tick(&mut self, _current_tick: u64) {
        for buff_list in self.buffs.values_mut() {
            for buff in buff_list.iter_mut() {
                if buff.remaining_ticks > 0 {
                    buff.remaining_ticks -= 1;
                }
            }
        }
    }

    /// Aggregate all active buff modifiers for a character into a single snapshot.
    /// Flat stats are multiplied by stack count; percentage stats are summed once per buff.
    pub fn get_effective_modifiers(&self, character_id: &str) -> ModifierSnapshot {
        let mut snap = ModifierSnapshot::default();

        if let Some(buff_list) = self.buffs.get(character_id) {
            for buff in buff_list {
                let stacks = buff.current_stacks as f64;
                for modifier in &buff.data.modifiers {
                    let val = modifier.value;
                    match modifier.stat_name.as_str() {
                        "atk_flat" => snap.atk_flat += val * stacks,
                        "atk_pct" => snap.atk_pct += val,
                        "def_flat" => snap.def_flat += val * stacks,
                        "def_pct" => snap.def_pct += val,
                        "hp_flat" => snap.hp_flat += val * stacks,
                        "hp_pct" => snap.hp_pct += val,
                        "crit_rate" => snap.crit_rate += val,
                        "crit_dmg" => snap.crit_dmg += val,
                        "pen" => snap.pen += val * stacks,
                        "pen_ratio" => snap.pen_ratio += val,
                        "dmg_bonus" => snap.dmg_bonus += val,
                        "anomaly_mastery" => snap.anomaly_mastery += val,
                        "anomaly_proficiency" => snap.anomaly_proficiency += val,
                        "impact" => snap.impact += val * stacks,
                        "energy_regen" => snap.energy_regen += val,
                        _ => {}
                    }
                }
            }
        }

        snap
    }

    /// Return the number of active buffs across all characters.
    pub fn total_active_buffs(&self) -> usize {
        self.buffs.values().map(|v| v.len()).sum()
    }
}

impl Default for BuffManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atk_buff() -> BuffData {
        BuffData::new("buff_atk_20", BuffCategory::Stat, StackType::Replace)
            .with_duration(100)
            .with_max_stacks(3)
            .with_modifier("atk_flat", 50.0)
    }

    // ── apply_buff ──────────────────────────────────────────────────────

    #[test]
    fn test_apply_buff_replace() {
        let mut mgr = BuffManager::new();
        let buff = atk_buff();
        mgr.apply_buff("char_A", buff, 0);

        let snap = mgr.get_effective_modifiers("char_A");
        assert_eq!(snap.atk_flat, 50.0);

        // Re-apply same buff — replace should override.
        let buff2 = BuffData::new("buff_atk_20", BuffCategory::Stat, StackType::Replace)
            .with_duration(100)
            .with_modifier("atk_flat", 80.0);
        mgr.apply_buff("char_A", buff2, 10);

        let snap = mgr.get_effective_modifiers("char_A");
        assert_eq!(snap.atk_flat, 80.0);
        assert_eq!(mgr.total_active_buffs(), 1);
    }

    #[test]
    fn test_apply_buff_additive() {
        let mut mgr = BuffManager::new();
        let buff = BuffData::new("buff_stack", BuffCategory::Stat, StackType::Additive)
            .with_duration(50)
            .with_max_stacks(3)
            .with_modifier("atk_flat", 30.0);

        mgr.apply_buff("char_A", buff.clone(), 0);
        mgr.apply_buff("char_A", buff.clone(), 0);
        mgr.apply_buff("char_A", buff.clone(), 0);
        mgr.apply_buff("char_A", buff, 0); // 4th — should cap at max_stacks

        let snap = mgr.get_effective_modifiers("char_A");
        // 3 stacks × 30 flat = 90
        assert_eq!(snap.atk_flat, 90.0);
        assert_eq!(mgr.total_active_buffs(), 1);
    }

    #[test]
    fn test_apply_buff_independent() {
        let mut mgr = BuffManager::new();
        let buff = BuffData::new("buff_indie", BuffCategory::Crit, StackType::Independent)
            .with_duration(30)
            .with_modifier("crit_rate", 0.1);

        mgr.apply_buff("char_A", buff.clone(), 0);
        mgr.apply_buff("char_A", buff, 0);

        // Two independent entries — their mods add up.
        let snap = mgr.get_effective_modifiers("char_A");
        assert_eq!(snap.crit_rate, 0.2);
        assert_eq!(mgr.total_active_buffs(), 2);
    }

    // ── remove_expired ──────────────────────────────────────────────────

    #[test]
    fn test_remove_expired_clears_zero_duration() {
        let mut mgr = BuffManager::new();
        let buff = BuffData::new("temp", BuffCategory::Stat, StackType::Replace).with_duration(3);
        mgr.apply_buff("char_A", buff, 0);

        // Manually drain remaining_ticks
        mgr.on_tick(1);
        mgr.on_tick(2);
        mgr.on_tick(3);
        assert_eq!(mgr.buffs.get("char_A").unwrap()[0].remaining_ticks, 0);

        mgr.remove_expired(3);
        assert!(mgr.buffs.get("char_A").unwrap().is_empty());
    }

    #[test]
    fn test_remove_expired_keeps_active() {
        let mut mgr = BuffManager::new();
        let buff =
            BuffData::new("long_buff", BuffCategory::Stat, StackType::Replace).with_duration(100);
        mgr.apply_buff("char_A", buff, 0);
        mgr.on_tick(1);
        mgr.remove_expired(1);

        let snap = mgr.get_effective_modifiers("char_A");
        // Should still be present (99 ticks remaining).
        assert_eq!(mgr.total_active_buffs(), 1);
        assert_eq!(snap, ModifierSnapshot::default()); // no modifiers on this buff
    }

    // ── on_tick ─────────────────────────────────────────────────────────

    #[test]
    fn test_on_tick_decrements_remaining() {
        let mut mgr = BuffManager::new();
        let buff =
            BuffData::new("tick_down", BuffCategory::Stat, StackType::Replace).with_duration(5);
        mgr.apply_buff("char_A", buff, 10);

        mgr.on_tick(11);
        assert_eq!(mgr.buffs.get("char_A").unwrap()[0].remaining_ticks, 4);

        mgr.on_tick(12);
        assert_eq!(mgr.buffs.get("char_A").unwrap()[0].remaining_ticks, 3);
    }

    #[test]
    fn test_on_tick_does_not_underflow() {
        let mut mgr = BuffManager::new();
        let buff = BuffData::new("short", BuffCategory::Stat, StackType::Replace).with_duration(1);
        mgr.apply_buff("char_A", buff, 0);
        mgr.on_tick(1);
        assert_eq!(mgr.buffs.get("char_A").unwrap()[0].remaining_ticks, 0);
        // Another tick should not underflow (guarded by > 0 check)
        mgr.on_tick(2);
        assert_eq!(mgr.buffs.get("char_A").unwrap()[0].remaining_ticks, 0);
    }

    // ── get_effective_modifiers ─────────────────────────────────────────

    #[test]
    fn test_get_effective_modifiers_empty_for_unknown_character() {
        let mgr = BuffManager::new();
        let snap = mgr.get_effective_modifiers("nobody");
        assert_eq!(snap, ModifierSnapshot::default());
    }

    #[test]
    fn test_get_effective_modifiers_aggregates_multiple_buffs() {
        let mut mgr = BuffManager::new();

        let atk_buff = BuffData::new("atk_boost", BuffCategory::Stat, StackType::Replace)
            .with_duration(60)
            .with_modifier("atk_flat", 100.0);
        let crit_buff = BuffData::new("crit_up", BuffCategory::Crit, StackType::Replace)
            .with_duration(60)
            .with_modifier("crit_rate", 0.15)
            .with_modifier("crit_dmg", 0.3);
        let dmg_buff = BuffData::new("dmg_up", BuffCategory::DamageBonus, StackType::Replace)
            .with_duration(60)
            .with_modifier("dmg_bonus", 0.25);

        mgr.apply_buff("char_A", atk_buff, 0);
        mgr.apply_buff("char_A", crit_buff, 0);
        mgr.apply_buff("char_A", dmg_buff, 0);

        let snap = mgr.get_effective_modifiers("char_A");
        assert_eq!(snap.atk_flat, 100.0);
        assert_eq!(snap.crit_rate, 0.15);
        assert_eq!(snap.crit_dmg, 0.3);
        assert_eq!(snap.dmg_bonus, 0.25);
    }

    #[test]
    fn test_get_effective_modifiers_pct_not_multiplied_by_stacks() {
        let mut mgr = BuffManager::new();
        let buff = BuffData::new("pct_buff", BuffCategory::Stat, StackType::Additive)
            .with_duration(100)
            .with_max_stacks(3)
            .with_modifier("atk_pct", 0.2)
            .with_modifier("atk_flat", 50.0);

        mgr.apply_buff("char_A", buff.clone(), 0);
        mgr.apply_buff("char_A", buff, 0); // 2 stacks

        let snap = mgr.get_effective_modifiers("char_A");
        // atk_flat is multiplied by stacks: 50 * 2 = 100
        // atk_pct is NOT multiplied by stacks: 0.2 (once per buff)
        assert_eq!(snap.atk_flat, 100.0);
        assert_eq!(snap.atk_pct, 0.2);
    }

    // ── Combine ─────────────────────────────────────────────────────────

    #[test]
    fn test_modifier_snapshot_combine() {
        let a = ModifierSnapshot {
            atk_flat: 50.0,
            atk_pct: 0.1,
            ..Default::default()
        };
        let b = ModifierSnapshot {
            atk_flat: 30.0,
            crit_rate: 0.05,
            ..Default::default()
        };
        let c = a.combine(&b);
        assert_eq!(c.atk_flat, 80.0);
        assert_eq!(c.atk_pct, 0.1);
        assert_eq!(c.crit_rate, 0.05);
    }

    // ── BuffData builder ────────────────────────────────────────────────

    #[test]
    fn test_buff_data_builder() {
        let buff = BuffData::new("test_buff", BuffCategory::Anomaly, StackType::Additive)
            .with_max_stacks(5)
            .with_duration(120)
            .with_modifier("anomaly_proficiency", 50.0);

        assert_eq!(buff.buff_id, "test_buff");
        assert_eq!(buff.category, BuffCategory::Anomaly);
        assert_eq!(buff.stack_type, StackType::Additive);
        assert_eq!(buff.max_stacks, 5);
        assert_eq!(buff.duration_ticks, 120);
        assert_eq!(buff.modifiers.len(), 1);
        assert_eq!(buff.modifiers[0].stat_name, "anomaly_proficiency");
        assert_eq!(buff.modifiers[0].value, 50.0);
    }

    #[test]
    fn test_max_stacks_clamped_at_one() {
        let buff =
            BuffData::new("clamped", BuffCategory::Stat, StackType::Additive).with_max_stacks(0); // should be clamped to 1
        assert_eq!(buff.max_stacks, 1);
    }

    // ── Full lifecycle ──────────────────────────────────────────────────

    #[test]
    fn test_buff_full_lifecycle() {
        let mut mgr = BuffManager::new();

        // Apply a short-duration buff and a long-duration buff.
        let short_buff = BuffData::new("short", BuffCategory::Stat, StackType::Replace)
            .with_duration(3)
            .with_modifier("atk_flat", 100.0);
        let long_buff = BuffData::new("long", BuffCategory::Crit, StackType::Replace)
            .with_duration(10)
            .with_modifier("crit_rate", 0.1);

        mgr.apply_buff("char_A", short_buff, 0);
        mgr.apply_buff("char_A", long_buff, 0);

        assert_eq!(mgr.total_active_buffs(), 2);

        // Tick 3 times — short buff expires.
        for t in 1..=3 {
            mgr.on_tick(t);
        }
        mgr.remove_expired(3);

        assert_eq!(mgr.total_active_buffs(), 1);
        let snap = mgr.get_effective_modifiers("char_A");
        assert_eq!(snap.atk_flat, 0.0);
        assert_eq!(snap.crit_rate, 0.1);
    }
}
