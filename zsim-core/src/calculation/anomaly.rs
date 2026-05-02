use std::collections::HashMap;

use crate::entities::enums::ElementTag;

// ---------------------------------------------------------------------------
// Default anomaly durations (ticks at 60 fps = ~10 seconds per element).
// In ZZZ, these vary by element; the values below are representative.
// ---------------------------------------------------------------------------
const DEFAULT_ANOMALY_DURATION: u64 = 600;

/// Per-element anomaly gauge state for a single enemy.
#[derive(Debug, Clone, PartialEq)]
pub struct AnomalyState {
    pub element: ElementTag,
    /// Current buildup gauge (0.0 → threshold).
    pub gauge: f64,
    /// Buildup threshold for this element on this enemy.
    pub max_gauge: f64,
    /// Remaining ticks of an active anomaly (0 = no active anomaly).
    pub remaining_duration: u64,
    /// How many times this element's anomaly has been triggered.
    pub trigger_count: u32,
    /// Whether the anomaly is currently active (i.e. status effect is live).
    pub is_active: bool,
}

impl AnomalyState {
    pub fn new(element: ElementTag, max_gauge: f64) -> Self {
        Self {
            element,
            gauge: 0.0,
            max_gauge,
            remaining_duration: 0,
            trigger_count: 0,
            is_active: false,
        }
    }
}

/// Accumulation result returned by `accumulate()`.
#[derive(Debug, Clone, PartialEq)]
pub struct AccumulateResult {
    /// Gauge added this call.
    pub gauge_added: f64,
    /// Whether the gauge reached the threshold (triggers anomaly).
    pub triggered: bool,
    /// Whether a disorder was detected after triggering.
    pub disorder_detected: bool,
}

/// Trigger result returned by `trigger_anomaly()`.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerAnomalyResult {
    pub element: ElementTag,
    /// Number of times this anomaly has been triggered so far.
    pub trigger_count: u32,
}

/// Disorder result returned by `check_disorder()`.
#[derive(Debug, Clone, PartialEq)]
pub struct DisorderResult {
    pub old_element: ElementTag,
    pub old_gauge: f64,
    pub new_element: ElementTag,
    pub new_gauge: f64,
}

/// Manages per-enemy anomaly buildup, triggering, duration ticking, and
/// disorder detection.
///
/// Each enemy maintains a `Vec<AnomalyState>` — one slot per element that
/// has ever been accumulated on that enemy.  When an element reaches its
/// threshold the anomaly is triggered; if another anomaly was already active
/// on the same enemy, a disorder is detected.
pub struct AnomalyDisorderManager {
    /// enemy_id → anomaly states per element.
    states: HashMap<String, Vec<AnomalyState>>,
    /// Duration (in ticks) each element's anomaly lasts once triggered.
    anomaly_durations: HashMap<ElementTag, u64>,
}

impl AnomalyDisorderManager {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            anomaly_durations: HashMap::new(),
        }
    }

    /// Register a custom anomaly duration for an element.
    /// If not set, `DEFAULT_ANOMALY_DURATION` (600 ticks) is used.
    pub fn set_anomaly_duration(&mut self, element: ElementTag, duration_ticks: u64) {
        self.anomaly_durations.insert(element, duration_ticks);
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    fn duration_for(&self, element: &ElementTag) -> u64 {
        self.anomaly_durations
            .get(element)
            .copied()
            .unwrap_or(DEFAULT_ANOMALY_DURATION)
    }

    /// Ensure an AnomalyState slot exists for `element` on `enemy_id`.
    fn get_or_create_state(&mut self, enemy_id: &str, element: &ElementTag) -> &mut AnomalyState {
        let entry = self.states.entry(enemy_id.to_string()).or_default();

        if !entry.iter().any(|s| s.element == *element) {
            entry.push(AnomalyState::new(*element, 100.0));
        }

        entry.iter_mut().find(|s| s.element == *element).unwrap()
    }

    fn get_state(&self, enemy_id: &str, element: &ElementTag) -> Option<&AnomalyState> {
        self.states
            .get(enemy_id)?
            .iter()
            .find(|s| s.element == *element)
    }

    // ── Public API ───────────────────────────────────────────────────────

    /// Accumulate anomaly gauge on an enemy for a given element.
    ///
    /// Returns `AccumulateResult` indicating whether the gauge reached the
    /// threshold (`triggered`) and, if so, whether a disorder was also
    /// detected (`disorder_detected`).
    ///
    /// When `triggered` is `true`, the anomaly state is immediately
    /// transitioned to active (caller should use `trigger_anomaly` for
    /// explicit control — this method calls it internally).
    pub fn accumulate(
        &mut self,
        enemy_id: &str,
        element: ElementTag,
        amount: f64,
    ) -> AccumulateResult {
        // Step 1: read gauge, check threshold (drop mutable borrow before
        // calling methods that need &self).
        let (triggered, gauge_added) = {
            let state = self.get_or_create_state(enemy_id, &element);
            state.gauge += amount;
            let g = amount;
            let trig = state.gauge >= state.max_gauge;
            (trig, g)
        };

        let disorder_detected = if triggered {
            // Step 2: check disorder (&self only) and trigger (&mut self).
            let disorder = self.check_disorder_internal(enemy_id, &element);
            {
                let state = self.get_or_create_state(enemy_id, &element);
                state.gauge = 0.0;
            }
            self.trigger_anomaly_internal(enemy_id, &element);
            disorder
        } else {
            false
        };

        AccumulateResult {
            gauge_added,
            triggered,
            disorder_detected,
        }
    }

    /// Explicitly trigger an anomaly on an enemy for a given element.
    ///
    /// Resets the gauge, increments trigger count, and sets the anomaly as
    /// active with its configured duration.
    pub fn trigger_anomaly(
        &mut self,
        enemy_id: &str,
        element: ElementTag,
    ) -> Option<TriggerAnomalyResult> {
        self.trigger_anomaly_internal(enemy_id, &element);

        let state = self.get_state(enemy_id, &element)?;
        Some(TriggerAnomalyResult {
            element,
            trigger_count: state.trigger_count,
        })
    }

    fn trigger_anomaly_internal(&mut self, enemy_id: &str, element: &ElementTag) {
        // Compute duration before the mutable borrow.
        let duration = self.duration_for(element);
        if let Some(state) = self
            .states
            .get_mut(enemy_id)
            .and_then(|v| v.iter_mut().find(|s| s.element == *element))
        {
            state.gauge = 0.0;
            state.trigger_count += 1;
            state.remaining_duration = duration;
            state.is_active = true;
        }
    }

    /// Check whether applying a new anomaly on an enemy would trigger a
    /// disorder (i.e. a different-element anomaly is already active).
    pub fn check_disorder(
        &self,
        enemy_id: &str,
        new_element: &ElementTag,
    ) -> Option<DisorderResult> {
        if !self.check_disorder_internal(enemy_id, new_element) {
            return None;
        }

        // Find the active anomaly of a different element.
        let old_el = self
            .states
            .get(enemy_id)?
            .iter()
            .find(|s| s.element != *new_element && s.is_active)?;

        // The new element may not have a state slot yet; use 0 gauge if absent.
        let new_gauge = self
            .get_state(enemy_id, new_element)
            .map(|s| s.gauge)
            .unwrap_or(0.0);

        Some(DisorderResult {
            old_element: old_el.element,
            old_gauge: old_el.gauge,
            new_element: *new_element,
            new_gauge,
        })
    }

    /// Internal disorder check — returns true if a different-element anomaly
    /// is already active on the same enemy.
    fn check_disorder_internal(&self, enemy_id: &str, new_element: &ElementTag) -> bool {
        self.states
            .get(enemy_id)
            .is_some_and(|v| v.iter().any(|s| s.element != *new_element && s.is_active))
    }

    /// Advance one tick for all active anomalies.
    ///
    /// Decrements `remaining_duration` for each active anomaly.  When
    /// duration reaches zero, `is_active` is set to `false`.
    pub fn on_tick(&mut self, _current_tick: u64) {
        for state_list in self.states.values_mut() {
            for state in state_list.iter_mut() {
                if state.is_active && state.remaining_duration > 0 {
                    state.remaining_duration -= 1;
                    if state.remaining_duration == 0 {
                        state.is_active = false;
                    }
                }
            }
        }
    }

    /// Query the current anomaly gauge for an enemy/element pair.
    pub fn get_gauge(&self, enemy_id: &str, element: &ElementTag) -> f64 {
        self.get_state(enemy_id, element)
            .map(|s| s.gauge)
            .unwrap_or(0.0)
    }

    /// Check if a specific anomaly is currently active on an enemy.
    pub fn is_active(&self, enemy_id: &str, element: &ElementTag) -> bool {
        self.get_state(enemy_id, element)
            .map(|s| s.is_active)
            .unwrap_or(false)
    }

    /// Get the remaining duration (ticks) for an active anomaly.
    pub fn remaining_duration(&self, enemy_id: &str, element: &ElementTag) -> u64 {
        self.get_state(enemy_id, element)
            .map(|s| s.remaining_duration)
            .unwrap_or(0)
    }

    /// Return all elements with an active anomaly on the given enemy.
    pub fn active_elements(&self, enemy_id: &str) -> Vec<ElementTag> {
        self.states
            .get(enemy_id)
            .map(|v| {
                v.iter()
                    .filter(|s| s.is_active)
                    .map(|s| s.element)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Total number of anomaly state slots across all enemies.
    pub fn total_slots(&self) -> usize {
        self.states.values().map(|v| v.len()).sum()
    }

    /// Number of enemies being tracked.
    pub fn enemy_count(&self) -> usize {
        self.states.len()
    }
}

impl Default for AnomalyDisorderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Basic accumulation ─────────────────────────────────────────────

    #[test]
    fn test_accumulate_partial_no_trigger() {
        let mut mgr = AnomalyDisorderManager::new();
        let result = mgr.accumulate("enemy_A", ElementTag::Fire, 40.0);
        assert!(!result.triggered);
        assert!(!result.disorder_detected);
        assert!((result.gauge_added - 40.0).abs() < 1e-9);
        assert!((mgr.get_gauge("enemy_A", &ElementTag::Fire) - 40.0).abs() < 1e-9);
    }

    #[test]
    fn test_accumulate_exactly_max_triggers() {
        let mut mgr = AnomalyDisorderManager::new();
        // max_gauge defaults to 100.0
        let result = mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert!(result.triggered);
        assert!(!result.disorder_detected);
        // Gauge should be reset to 0 after trigger
        assert!((mgr.get_gauge("enemy_A", &ElementTag::Ice) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_accumulate_over_max_triggers() {
        let mut mgr = AnomalyDisorderManager::new();
        let result = mgr.accumulate("enemy_A", ElementTag::Fire, 150.0);
        assert!(result.triggered);
        assert_eq!(mgr.get_gauge("enemy_A", &ElementTag::Fire), 0.0);
    }

    // ── Trigger anomaly ─────────────────────────────────────────────────

    #[test]
    fn test_trigger_anomaly_sets_active() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.accumulate("enemy_A", ElementTag::Fire, 105.0);
        // accumulate triggers automatically
        assert!(mgr.is_active("enemy_A", &ElementTag::Fire));
        assert!(mgr.active_elements("enemy_A").contains(&ElementTag::Fire));
    }

    #[test]
    fn test_trigger_anomaly_increments_count() {
        let mut mgr = AnomalyDisorderManager::new();

        // Access internal state to check trigger_count
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        assert!(mgr.is_active("enemy_A", &ElementTag::Fire));

        // Verify trigger_count = 1
        match mgr.trigger_anomaly("enemy_A", ElementTag::Fire) {
            Some(r) => assert_eq!(r.trigger_count, 2),
            None => panic!("expected result"),
        }
    }

    #[test]
    fn test_trigger_anomaly_unknown_enemy_returns_none() {
        let mut mgr = AnomalyDisorderManager::new();
        let result = mgr.trigger_anomaly("nobody", ElementTag::Fire);
        assert!(result.is_none());
    }

    // ── Anomaly durations ───────────────────────────────────────────────

    #[test]
    fn test_trigger_anomaly_sets_duration() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Ice, 300);
        mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert_eq!(mgr.remaining_duration("enemy_A", &ElementTag::Ice), 300);
    }

    #[test]
    fn test_default_duration() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        assert_eq!(mgr.remaining_duration("enemy_A", &ElementTag::Fire), 600);
    }

    // ── on_tick ─────────────────────────────────────────────────────────

    #[test]
    fn test_on_tick_decrements_duration() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Fire, 10);
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);

        mgr.on_tick(1);
        assert_eq!(mgr.remaining_duration("enemy_A", &ElementTag::Fire), 9);
        assert!(mgr.is_active("enemy_A", &ElementTag::Fire));

        mgr.on_tick(2);
        assert_eq!(mgr.remaining_duration("enemy_A", &ElementTag::Fire), 8);
    }

    #[test]
    fn test_on_tick_expiry_clears_active() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Electric, 3);
        mgr.accumulate("enemy_A", ElementTag::Electric, 100.0);
        assert!(mgr.is_active("enemy_A", &ElementTag::Electric));

        mgr.on_tick(1);
        mgr.on_tick(2);
        mgr.on_tick(3);
        assert!(!mgr.is_active("enemy_A", &ElementTag::Electric));
        assert_eq!(mgr.remaining_duration("enemy_A", &ElementTag::Electric), 0);
    }

    #[test]
    fn test_on_tick_already_zero_does_not_underflow() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Ice, 1);
        mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);

        mgr.on_tick(1);
        mgr.on_tick(2); // already 0, should not underflow
        assert_eq!(mgr.remaining_duration("enemy_A", &ElementTag::Ice), 0);
    }

    // ── Disorder detection ──────────────────────────────────────────────

    #[test]
    fn test_disorder_detected_on_second_anomaly() {
        let mut mgr = AnomalyDisorderManager::new();

        // Trigger Fire anomaly first
        let r1 = mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        assert!(r1.triggered);
        assert!(!r1.disorder_detected);

        // Trigger Ice anomaly while Fire is still active → disorder
        let r2 = mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert!(r2.triggered);
        assert!(r2.disorder_detected);
    }

    #[test]
    fn test_no_disorder_for_same_element() {
        let mut mgr = AnomalyDisorderManager::new();

        // Trigger Fire twice in a row (first expires, second is new trigger)
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);

        // Wait for Fire to expire
        for t in 1..=600 {
            mgr.on_tick(t);
        }
        assert!(!mgr.is_active("enemy_A", &ElementTag::Fire));

        // Now trigger Fire again — no disorder (same element, no other active)
        let r = mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        assert!(r.triggered);
        assert!(!r.disorder_detected);
    }

    #[test]
    fn test_disorder_after_expiry() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Fire, 5);
        mgr.set_anomaly_duration(ElementTag::Ice, 10);

        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);

        // Let Fire expire
        for t in 1..=6 {
            mgr.on_tick(t);
        }
        assert!(!mgr.is_active("enemy_A", &ElementTag::Fire));

        // Now Ice trigger — no disorder because Fire already expired
        let r = mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert!(r.triggered);
        assert!(!r.disorder_detected);
    }

    // ── check_disorder ──────────────────────────────────────────────────

    #[test]
    fn test_check_disorder_returns_elements() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);

        // Fire is active, check for Ice
        let disorder = mgr.check_disorder("enemy_A", &ElementTag::Ice);
        assert!(disorder.is_some());

        let d = disorder.unwrap();
        assert_eq!(d.old_element, ElementTag::Fire);
        assert_eq!(d.new_element, ElementTag::Ice);
    }

    #[test]
    fn test_check_disorder_no_active_returns_none() {
        let mgr = AnomalyDisorderManager::new();
        let disorder = mgr.check_disorder("enemy_A", &ElementTag::Fire);
        assert!(disorder.is_none());
    }

    #[test]
    fn test_check_disorder_expired_anomaly_returns_none() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Fire, 5);
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);

        for t in 1..=6 {
            mgr.on_tick(t);
        }

        let disorder = mgr.check_disorder("enemy_A", &ElementTag::Ice);
        assert!(disorder.is_none());
    }

    // ── Query helpers ───────────────────────────────────────────────────

    #[test]
    fn test_get_gauge_unknown_returns_zero() {
        let mgr = AnomalyDisorderManager::new();
        assert_eq!(mgr.get_gauge("nobody", &ElementTag::Fire), 0.0);
    }

    #[test]
    fn test_is_active_unknown_returns_false() {
        let mgr = AnomalyDisorderManager::new();
        assert!(!mgr.is_active("nobody", &ElementTag::Fire));
    }

    #[test]
    fn test_remaining_duration_unknown_returns_zero() {
        let mgr = AnomalyDisorderManager::new();
        assert_eq!(mgr.remaining_duration("nobody", &ElementTag::Fire), 0);
    }

    #[test]
    fn test_active_elements_empty_for_unknown() {
        let mgr = AnomalyDisorderManager::new();
        assert!(mgr.active_elements("nobody").is_empty());
    }

    #[test]
    fn test_active_elements_multiple_elements() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Fire, 100);
        mgr.set_anomaly_duration(ElementTag::Ice, 100);

        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);

        // Both should be active (disorder was detected, but anomalies are both active)
        let active = mgr.active_elements("enemy_A");
        assert!(active.contains(&ElementTag::Fire));
        assert!(active.contains(&ElementTag::Ice));
    }

    // ── Total counts ────────────────────────────────────────────────────

    #[test]
    fn test_total_slots() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.accumulate("enemy_A", ElementTag::Fire, 10.0);
        mgr.accumulate("enemy_B", ElementTag::Ice, 10.0);
        mgr.accumulate("enemy_B", ElementTag::Fire, 10.0);
        // enemy_A: 1 slot; enemy_B: 2 slots
        assert_eq!(mgr.total_slots(), 3);
        assert_eq!(mgr.enemy_count(), 2);
    }

    // ── Full lifecycle ──────────────────────────────────────────────────

    #[test]
    fn test_full_lifecycle() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Fire, 10);
        mgr.set_anomaly_duration(ElementTag::Ice, 10);

        // 1. Accumulate some Fire gauge
        let r = mgr.accumulate("enemy_A", ElementTag::Fire, 60.0);
        assert!(!r.triggered);
        assert!((mgr.get_gauge("enemy_A", &ElementTag::Fire) - 60.0).abs() < 1e-9);

        // 2. Accumulate more Fire to trigger
        assert!(mgr.accumulate("enemy_A", ElementTag::Fire, 40.0).triggered);
        assert!(mgr.is_active("enemy_A", &ElementTag::Fire));
        assert!((mgr.get_gauge("enemy_A", &ElementTag::Fire) - 0.0).abs() < 1e-9);

        // 3. Tick down the anomaly
        for t in 1..=10 {
            mgr.on_tick(t);
        }
        assert!(!mgr.is_active("enemy_A", &ElementTag::Fire));

        // 4. Accumulate Ice — no disorder (Fire expired)
        let r = mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert!(r.triggered);
        assert!(!r.disorder_detected);
        assert!(mgr.is_active("enemy_A", &ElementTag::Ice));

        // 5. Fire again while Ice is active → disorder
        let r = mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        assert!(r.triggered);
        assert!(r.disorder_detected);

        // Both should be active now
        assert_eq!(mgr.active_elements("enemy_A").len(), 2);
    }

    // ── set_anomaly_duration ────────────────────────────────────────────

    #[test]
    fn test_set_anomaly_duration_custom() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Physical, 900);
        mgr.accumulate("enemy_A", ElementTag::Physical, 100.0);
        assert_eq!(
            mgr.remaining_duration("enemy_A", &ElementTag::Physical),
            900
        );
    }
}
