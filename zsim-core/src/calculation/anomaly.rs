use std::collections::HashMap;

use crate::entities::enums::ElementTag;

// ---------------------------------------------------------------------------
// 默认异常持续时间（60 fps 下的 tick 数，约每元素 10 秒）。
// 在 ZZZ 中，不同元素的数据有所不同；以下数值为代表性取值。
// ---------------------------------------------------------------------------
const DEFAULT_ANOMALY_DURATION: u64 = 600;

/// 单个敌人的每元素异常累积状态。
#[derive(Debug, Clone, PartialEq)]
pub struct AnomalyState {
    pub element: ElementTag,
    /// 当前累积值（从 0.0 到阈值）。
    pub gauge: f64,
    /// 该元素在此敌人身上的累积阈值。
    pub max_gauge: f64,
    /// 活跃异常的剩余 tick 数（0 表示无活跃异常）。
    pub remaining_duration: u64,
    /// 该元素异常已被触发的次数。
    pub trigger_count: u32,
    /// 异常当前是否处于活跃状态（即状态效果正在生效）。
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

/// `accumulate()` 返回的累积结果。
#[derive(Debug, Clone, PartialEq)]
pub struct AccumulateResult {
    /// 本次调用增加的异常累积值。
    pub gauge_added: f64,
    /// 累积值是否已达到阈值（触发异常）。
    pub triggered: bool,
    /// 触发后是否检测到了紊乱（disorder）。
    pub disorder_detected: bool,
}

/// `trigger_anomaly()` 返回的触发结果。
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerAnomalyResult {
    pub element: ElementTag,
    /// 该异常迄今为止被触发的次数。
    pub trigger_count: u32,
}

/// `check_disorder()` 返回的紊乱结果。
#[derive(Debug, Clone, PartialEq)]
pub struct DisorderResult {
    pub old_element: ElementTag,
    pub old_gauge: f64,
    pub new_element: ElementTag,
    pub new_gauge: f64,
}

/// 管理每个敌人的异常累积、触发、持续时间计时以及紊乱检测。
///
/// 每个敌人维护一个 `Vec<AnomalyState>` —— 每个元素一个槽位，
/// 只要该元素曾在敌人身上累积过就会存在。当某个元素达到阈值时，
/// 异常被触发；如果同一敌人身上已有另一个异常处于活跃状态，
/// 则会检测到紊乱（disorder）。
pub struct AnomalyDisorderManager {
    /// 敌人 ID → 每个元素的异常状态。
    states: HashMap<String, Vec<AnomalyState>>,
    /// 每个元素的异常被触发后的持续时间（以 tick 为单位）。
    anomaly_durations: HashMap<ElementTag, u64>,
}

impl AnomalyDisorderManager {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            anomaly_durations: HashMap::new(),
        }
    }

    /// 注册某个元素的自定义异常持续时间。
    /// 如果未设置，则使用 `DEFAULT_ANOMALY_DURATION`（600 tick）。
    pub fn set_anomaly_duration(&mut self, element: ElementTag, duration_ticks: u64) {
        self.anomaly_durations.insert(element, duration_ticks);
    }

    // ── 内部辅助方法 ──────────────────────────────────────────────────

    fn duration_for(&self, element: &ElementTag) -> u64 {
        self.anomaly_durations
            .get(element)
            .copied()
            .unwrap_or(DEFAULT_ANOMALY_DURATION)
    }

    /// 确保 `enemy_id` 上存在 `element` 的 AnomalyState 槽位。
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

    // ── 公开 API ────────────────────────────────────────────────────────

    /// 在敌人身上累积指定元素的异常值。
    ///
    /// 返回 `AccumulateResult`，指示累积值是否达到阈值（`triggered`），
    /// 如果是，是否也检测到了紊乱（`disorder_detected`）。
    ///
    /// 当 `triggered` 为 `true` 时，异常状态立即转换为活跃
    /// （调用者可以使用 `trigger_anomaly` 进行显式控制 ——
    /// 但此方法会在内部调用它）。
    pub fn accumulate(
        &mut self,
        enemy_id: &str,
        element: ElementTag,
        amount: f64,
    ) -> AccumulateResult {
        // 步骤 1：读取累积值，检查阈值（在调用需要 &self 的方法前释放可变借用）。
        let (triggered, gauge_added) = {
            let state = self.get_or_create_state(enemy_id, &element);
            state.gauge += amount;
            let g = amount;
            let trig = state.gauge >= state.max_gauge;
            (trig, g)
        };

        let disorder_detected = if triggered {
            // 步骤 2：检查紊乱（仅 &self）并触发（&mut self）。
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

    /// 显式触发敌人身上指定元素的异常。
    ///
    /// 重置累积值、增加触发计数，并将异常设置为活跃状态
    /// 并附带其配置的持续时间。
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
        // 在可变借用之前计算持续时间。
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

    /// 检查在敌人身上施加新异常是否会触发紊乱
    /// （即是否存在另一个不同元素的异常已经处于活跃状态）。
    pub fn check_disorder(
        &self,
        enemy_id: &str,
        new_element: &ElementTag,
    ) -> Option<DisorderResult> {
        if !self.check_disorder_internal(enemy_id, new_element) {
            return None;
        }

        // 查找不同元素的活跃异常。
        let old_el = self
            .states
            .get(enemy_id)?
            .iter()
            .find(|s| s.element != *new_element && s.is_active)?;

        // 新元素可能还没有状态槽位；如果不存在则使用累积值 0。
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

    /// 内部紊乱检查 —— 如果同一敌人身上已有不同元素的异常处于活跃状态则返回 true。
    fn check_disorder_internal(&self, enemy_id: &str, new_element: &ElementTag) -> bool {
        self.states
            .get(enemy_id)
            .is_some_and(|v| v.iter().any(|s| s.element != *new_element && s.is_active))
    }

    /// 将所有活跃异常推进一个 tick。
    ///
    /// 对每个活跃异常的 `remaining_duration` 减一。当持续时间
    /// 归零时，`is_active` 被设置为 `false`。
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

    /// 查询敌人/元素组合的当前异常累积值。
    pub fn get_gauge(&self, enemy_id: &str, element: &ElementTag) -> f64 {
        self.get_state(enemy_id, element)
            .map(|s| s.gauge)
            .unwrap_or(0.0)
    }

    /// 检查指定异常当前是否在敌人身上处于活跃状态。
    pub fn is_active(&self, enemy_id: &str, element: &ElementTag) -> bool {
        self.get_state(enemy_id, element)
            .map(|s| s.is_active)
            .unwrap_or(false)
    }

    /// 获取活跃异常的剩余持续时间（以 tick 为单位）。
    pub fn remaining_duration(&self, enemy_id: &str, element: &ElementTag) -> u64 {
        self.get_state(enemy_id, element)
            .map(|s| s.remaining_duration)
            .unwrap_or(0)
    }

    /// 返回指定敌人上所有处于活跃状态的异常元素。
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

    /// 所有敌人的异常状态槽位总数。
    pub fn total_slots(&self) -> usize {
        self.states.values().map(|v| v.len()).sum()
    }

    /// 被追踪的敌人数量。
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

    // ── 基础累积 ──────────────────────────────────────────────────────

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
        // max_gauge 默认值为 100.0
        let result = mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert!(result.triggered);
        assert!(!result.disorder_detected);
        // 触发后累积值应重置为 0
        assert!((mgr.get_gauge("enemy_A", &ElementTag::Ice) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_accumulate_over_max_triggers() {
        let mut mgr = AnomalyDisorderManager::new();
        let result = mgr.accumulate("enemy_A", ElementTag::Fire, 150.0);
        assert!(result.triggered);
        assert_eq!(mgr.get_gauge("enemy_A", &ElementTag::Fire), 0.0);
    }

    // ── 触发异常 ──────────────────────────────────────────────────────────

    #[test]
    fn test_trigger_anomaly_sets_active() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.accumulate("enemy_A", ElementTag::Fire, 105.0);
        // accumulate 会自动触发异常
        assert!(mgr.is_active("enemy_A", &ElementTag::Fire));
        assert!(mgr.active_elements("enemy_A").contains(&ElementTag::Fire));
    }

    #[test]
    fn test_trigger_anomaly_increments_count() {
        let mut mgr = AnomalyDisorderManager::new();

        // 访问内部状态以检查 trigger_count
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        assert!(mgr.is_active("enemy_A", &ElementTag::Fire));

        // 验证 trigger_count = 1
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

    // ── 异常持续时间 ──────────────────────────────────────────────────────

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

    // ── 逐 tick 更新 ────────────────────────────────────────────────────

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
        mgr.on_tick(2); // 已经是 0，不应下溢
        assert_eq!(mgr.remaining_duration("enemy_A", &ElementTag::Ice), 0);
    }

    // ── 紊乱检测 ─────────────────────────────────────────────────────────

    #[test]
    fn test_disorder_detected_on_second_anomaly() {
        let mut mgr = AnomalyDisorderManager::new();

        // 先触发 Fire 异常
        let r1 = mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        assert!(r1.triggered);
        assert!(!r1.disorder_detected);

        // 在 Fire 仍处于活跃状态时触发 Ice 异常 → 紊乱
        let r2 = mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert!(r2.triggered);
        assert!(r2.disorder_detected);
    }

    #[test]
    fn test_no_disorder_for_same_element() {
        let mut mgr = AnomalyDisorderManager::new();

        // 连续触发两次 Fire（第一次过期，第二次是新的触发）
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);

        // 等待 Fire 过期
        for t in 1..=600 {
            mgr.on_tick(t);
        }
        assert!(!mgr.is_active("enemy_A", &ElementTag::Fire));

        // 现在再次触发 Fire — 无紊乱（相同元素，没有其他活跃异常）
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

        // 让 Fire 过期
        for t in 1..=6 {
            mgr.on_tick(t);
        }
        assert!(!mgr.is_active("enemy_A", &ElementTag::Fire));

        // 现在触发 Ice — 无紊乱，因为 Fire 已经过期
        let r = mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert!(r.triggered);
        assert!(!r.disorder_detected);
    }

    // ── 检查紊乱 ─────────────────────────────────────────────────────────

    #[test]
    fn test_check_disorder_returns_elements() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);

        // Fire 处于活跃状态，检查 Ice
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

    // ── 查询辅助方法 ─────────────────────────────────────────────────────

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

        // 两者都应处于活跃状态（检测到了紊乱，但两个异常都是活跃的）
        let active = mgr.active_elements("enemy_A");
        assert!(active.contains(&ElementTag::Fire));
        assert!(active.contains(&ElementTag::Ice));
    }

    // ── 总数统计 ─────────────────────────────────────────────────────────

    #[test]
    fn test_total_slots() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.accumulate("enemy_A", ElementTag::Fire, 10.0);
        mgr.accumulate("enemy_B", ElementTag::Ice, 10.0);
        mgr.accumulate("enemy_B", ElementTag::Fire, 10.0);
        // enemy_A: 1 个槽位；enemy_B: 2 个槽位
        assert_eq!(mgr.total_slots(), 3);
        assert_eq!(mgr.enemy_count(), 2);
    }

    // ── 完整生命周期 ─────────────────────────────────────────────────────

    #[test]
    fn test_full_lifecycle() {
        let mut mgr = AnomalyDisorderManager::new();
        mgr.set_anomaly_duration(ElementTag::Fire, 10);
        mgr.set_anomaly_duration(ElementTag::Ice, 10);

        // 1. 累积一些 Fire 异常值
        let r = mgr.accumulate("enemy_A", ElementTag::Fire, 60.0);
        assert!(!r.triggered);
        assert!((mgr.get_gauge("enemy_A", &ElementTag::Fire) - 60.0).abs() < 1e-9);

        // 2. 累积更多 Fire 以触发异常
        assert!(mgr.accumulate("enemy_A", ElementTag::Fire, 40.0).triggered);
        assert!(mgr.is_active("enemy_A", &ElementTag::Fire));
        assert!((mgr.get_gauge("enemy_A", &ElementTag::Fire) - 0.0).abs() < 1e-9);

        // 3. 逐 tick 减少异常持续时间
        for t in 1..=10 {
            mgr.on_tick(t);
        }
        assert!(!mgr.is_active("enemy_A", &ElementTag::Fire));

        // 4. 累积 Ice — 无紊乱（Fire 已过期）
        let r = mgr.accumulate("enemy_A", ElementTag::Ice, 100.0);
        assert!(r.triggered);
        assert!(!r.disorder_detected);
        assert!(mgr.is_active("enemy_A", &ElementTag::Ice));

        // 5. 在 Ice 活跃时再次触发 Fire → 紊乱
        let r = mgr.accumulate("enemy_A", ElementTag::Fire, 100.0);
        assert!(r.triggered);
        assert!(r.disorder_detected);

        // 现在两者都应处于活跃状态
        assert_eq!(mgr.active_elements("enemy_A").len(), 2);
    }

    // ── 设置异常持续时间 ─────────────────────────────────────────────────

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
