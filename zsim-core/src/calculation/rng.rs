use rand::Rng;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// RNG 状态在某个时间点的快照。
/// 可以在之后恢复，以重放相同的随机序列。
#[derive(Debug, Clone)]
pub struct RngSnapshot {
    rng: ChaCha8Rng,
    call_count: u64,
}

/// 基于 ChaCha8Rng 的确定性随机数管理器。
///
/// 每个模拟实例应使用 `seed = base_seed + sim_index` 创建自己的 `RNGManager`，
/// 以确保可重复性，同时保持各模拟之间的独立性。
///
/// `call_count` 在每次 RNG 调用时递增，并在快照中记录，
/// 以便恢复快照时能够重现包括调用计数在内的精确序列。
#[derive(Debug, Clone)]
pub struct RNGManager {
    rng: ChaCha8Rng,
    seed: u64,
    call_count: u64,
}

impl RNGManager {
    /// 使用给定的种子值创建新的 RNGManager。
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            seed,
            call_count: 0,
        }
    }

    /// 生成一个 `[min, max)` 范围内的随机 `f64`。
    ///
    /// 递增调用计数器。
    pub fn gen_range(&mut self, min: f64, max: f64) -> f64 {
        self.call_count += 1;
        self.rng.gen_range(min..max)
    }

    /// 生成一个随机 `bool`，以给定概率为 `true`。
    ///
    /// 递增调用计数器。
    pub fn gen_bool(&mut self, probability: f64) -> bool {
        self.call_count += 1;
        self.rng.gen_bool(probability)
    }

    /// 暴击检查的简写：如果随机掷骰低于 `crit_rate` 则返回 `true`。
    ///
    /// 递增调用计数器。
    pub fn gen_crit(&mut self, crit_rate: f64) -> bool {
        self.gen_bool(crit_rate)
    }

    /// 获取当前 RNG 状态的快照。
    pub fn snapshot(&self) -> RngSnapshot {
        RngSnapshot {
            rng: self.rng.clone(),
            call_count: self.call_count,
        }
    }

    /// 从快照恢复 RNG 状态和调用计数。
    pub fn restore(&mut self, snapshot: RngSnapshot) {
        self.rng = snapshot.rng;
        self.call_count = snapshot.call_count;
    }

    /// 返回用于创建此管理器的种子值。
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// 返回迄今为止的 RNG 调用次数。
    pub fn call_count(&self) -> u64 {
        self.call_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 两个使用相同种子的管理器产生相同的序列。
    #[test]
    fn test_same_seed_identical_sequence() {
        let mut a = RNGManager::new(42);
        let mut b = RNGManager::new(42);
        for _ in 0..100 {
            assert_eq!(a.gen_range(0.0, 1.0), b.gen_range(0.0, 1.0));
        }
    }

    /// 不同的种子产生不同的序列。
    #[test]
    fn test_different_seed_different_sequence() {
        let mut a = RNGManager::new(42);
        let mut b = RNGManager::new(99);
        // 第一个值极大概率会不同
        assert_ne!(a.gen_range(0.0, 1.0), b.gen_range(0.0, 1.0));
    }

    /// gen_range 的值保持在 [min, max) 范围内。
    #[test]
    fn test_gen_range_bounds() {
        let mut rng = RNGManager::new(7);
        for _ in 0..1000 {
            let v = rng.gen_range(5.0, 10.0);
            assert!((5.0..10.0).contains(&v), "out of bounds: {v}");
        }
    }

    /// gen_bool 在多次试验中至少返回一次 true 和一次 false，
    /// 使用中等概率。
    #[test]
    fn test_gen_bool_variety() {
        let mut rng = RNGManager::new(13);
        let mut has_true = false;
        let mut has_false = false;
        for _ in 0..1000 {
            if rng.gen_bool(0.5) {
                has_true = true;
            } else {
                has_false = true;
            }
        }
        assert!(has_true, "expected at least one true");
        assert!(has_false, "expected at least one false");
    }

    /// gen_crit 委托给 gen_bool，使用给定的概率。
    #[test]
    fn test_gen_crit_behavior() {
        let mut rng = RNGManager::new(21);
        let mut crit_count = 0u64;
        let trials = 10_000;
        for _ in 0..trials {
            if rng.gen_crit(0.3) {
                crit_count += 1;
            }
        }
        // 在 30% 暴击率下进行 10000 次试验，期望约 3000 ± 合理误差范围
        assert!(crit_count > 2000, "crit_count too low: {crit_count}");
        assert!(crit_count < 4000, "crit_count too high: {crit_count}");
    }

    /// 每次 RNG 调用都会递增调用计数器。
    #[test]
    fn test_call_count_increments() {
        let mut rng = RNGManager::new(1);
        assert_eq!(rng.call_count(), 0);
        rng.gen_range(0.0, 1.0);
        assert_eq!(rng.call_count(), 1);
        rng.gen_bool(0.5);
        assert_eq!(rng.call_count(), 2);
        rng.gen_crit(0.5);
        assert_eq!(rng.call_count(), 3);
    }

    /// 快照捕获状态；恢复可精确还原。
    #[test]
    fn test_snapshot_restore() {
        let mut rng = RNGManager::new(99);
        // 推进几次调用
        rng.gen_range(0.0, 1.0);
        rng.gen_bool(0.5);
        let snap = rng.snapshot();

        // 进一步推进
        let v1 = rng.gen_range(0.0, 1.0);
        let v2 = rng.gen_bool(0.5);

        // 恢复后，接下来的值应匹配
        rng.restore(snap);
        assert_eq!(rng.gen_range(0.0, 1.0), v1);
        assert_eq!(rng.gen_bool(0.5), v2);
    }

    /// 恢复后，调用计数也会被恢复。
    #[test]
    fn test_snapshot_restore_preserves_call_count() {
        let mut rng = RNGManager::new(5);
        rng.gen_range(0.0, 1.0);
        rng.gen_range(0.0, 1.0);
        assert_eq!(rng.call_count(), 2);

        let snap = rng.snapshot();
        rng.gen_range(0.0, 1.0);
        assert_eq!(rng.call_count(), 3);

        rng.restore(snap);
        assert_eq!(rng.call_count(), 2);
    }

    /// 多个快照：可以恢复到较早的时间点并向前重放。
    #[test]
    fn test_multiple_snapshots() {
        let mut rng = RNGManager::new(123);

        rng.gen_range(0.0, 1.0);
        let snap1 = rng.snapshot();

        let v1 = rng.gen_range(0.0, 1.0);
        let snap2 = rng.snapshot();

        let v2 = rng.gen_range(0.0, 1.0);

        // 恢复到 snap1（v0 之后）→ 重放应产生 v1，然后是 v2
        rng.restore(snap1);
        assert_eq!(rng.gen_range(0.0, 1.0), v1);
        assert_eq!(rng.gen_range(0.0, 1.0), v2);

        // 恢复到 snap2（v1 之后）→ 重放应产生 v2
        rng.restore(snap2);
        assert_eq!(rng.gen_range(0.0, 1.0), v2);
    }

    /// 10 次使用相同种子的独立运行产生相同的序列。
    #[test]
    fn test_deterministic_10_runs() {
        const RUNS: usize = 10;
        const CALLS: usize = 50;
        let mut results: Vec<Vec<u64>> = Vec::with_capacity(RUNS);

        for _ in 0..RUNS {
            let mut rng = RNGManager::new(777);
            let mut vals = Vec::with_capacity(CALLS);
            for _ in 0..CALLS {
                // 混合不同类型的调用
                match vals.len() % 3 {
                    0 => vals.push(rng.gen_range(0.0, 100.0) as u64),
                    1 => vals.push(rng.gen_bool(0.5) as u64),
                    _ => vals.push(rng.gen_crit(0.3) as u64),
                }
            }
            results.push(vals);
        }

        for i in 1..RUNS {
            assert_eq!(results[0], results[i], "run {i} diverged from run 0");
        }
    }

    /// 即使在全新的 RNG 上，快照恢复也不会 panic。
    #[test]
    fn test_snapshot_restore_noop() {
        let mut rng = RNGManager::new(42);
        let snap = rng.snapshot();
        // 立即恢复，中间没有任何调用
        rng.restore(snap);
        assert_eq!(rng.call_count(), 0);
    }

    /// gen_range 使用负边界也能正常工作。
    #[test]
    fn test_gen_range_negative() {
        let mut rng = RNGManager::new(1);
        for _ in 0..100 {
            let v = rng.gen_range(-10.0, 10.0);
            assert!((-10.0..10.0).contains(&v), "out of bounds: {v}");
        }
    }

    /// seed() 返回构造时提供的种子值。
    #[test]
    fn test_seed_getter() {
        let rng = RNGManager::new(0xDEAD_BEEF);
        assert_eq!(rng.seed(), 0xDEAD_BEEF);
    }
}
