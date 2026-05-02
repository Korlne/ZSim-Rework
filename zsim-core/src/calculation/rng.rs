use rand::Rng;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// A snapshot of the RNG state at a given point.
/// Can be restored later to replay the same random sequence.
#[derive(Debug, Clone)]
pub struct RngSnapshot {
    rng: ChaCha8Rng,
    call_count: u64,
}

/// Deterministic random number manager based on ChaCha8Rng.
///
/// Each simulation instance should create its own `RNGManager` with
/// `seed = base_seed + sim_index` to ensure reproducibility while
/// keeping each simulation independent.
///
/// The `call_count` is incremented on every RNG call and tracked
/// inside snapshots so that restoring a snapshot reproduces the
/// exact sequence including call count tracking.
#[derive(Debug, Clone)]
pub struct RNGManager {
    rng: ChaCha8Rng,
    seed: u64,
    call_count: u64,
}

impl RNGManager {
    /// Create a new RNGManager seeded with the given value.
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            seed,
            call_count: 0,
        }
    }

    /// Generate a random `f64` in `[min, max)`.
    ///
    /// Increments the call counter.
    pub fn gen_range(&mut self, min: f64, max: f64) -> f64 {
        self.call_count += 1;
        self.rng.gen_range(min..max)
    }

    /// Generate a random `bool` with the given probability of being `true`.
    ///
    /// Increments the call counter.
    pub fn gen_bool(&mut self, probability: f64) -> bool {
        self.call_count += 1;
        self.rng.gen_bool(probability)
    }

    /// Shorthand for a crit check: returns `true` if a random roll is below `crit_rate`.
    ///
    /// Increments the call counter.
    pub fn gen_crit(&mut self, crit_rate: f64) -> bool {
        self.gen_bool(crit_rate)
    }

    /// Take a snapshot of the current RNG state.
    pub fn snapshot(&self) -> RngSnapshot {
        RngSnapshot {
            rng: self.rng.clone(),
            call_count: self.call_count,
        }
    }

    /// Restore RNG state and call count from a snapshot.
    pub fn restore(&mut self, snapshot: RngSnapshot) {
        self.rng = snapshot.rng;
        self.call_count = snapshot.call_count;
    }

    /// Return the seed used to create this manager.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Return the number of RNG calls made so far.
    pub fn call_count(&self) -> u64 {
        self.call_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two managers with the same seed produce the same sequence.
    #[test]
    fn test_same_seed_identical_sequence() {
        let mut a = RNGManager::new(42);
        let mut b = RNGManager::new(42);
        for _ in 0..100 {
            assert_eq!(a.gen_range(0.0, 1.0), b.gen_range(0.0, 1.0));
        }
    }

    /// Different seeds produce different sequences.
    #[test]
    fn test_different_seed_different_sequence() {
        let mut a = RNGManager::new(42);
        let mut b = RNGManager::new(99);
        // With extremely high probability the first value will differ
        assert_ne!(a.gen_range(0.0, 1.0), b.gen_range(0.0, 1.0));
    }

    /// gen_range values stay within [min, max).
    #[test]
    fn test_gen_range_bounds() {
        let mut rng = RNGManager::new(7);
        for _ in 0..1000 {
            let v = rng.gen_range(5.0, 10.0);
            assert!((5.0..10.0).contains(&v), "out of bounds: {v}");
        }
    }

    /// gen_bool returns true at least once and false at least once
    /// over many trials with a moderate probability.
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

    /// gen_crit delegates to gen_bool with the given probability.
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
        // With 30% crit rate over 10000 trials, expect ~3000 ± reasonable margin
        assert!(crit_count > 2000, "crit_count too low: {crit_count}");
        assert!(crit_count < 4000, "crit_count too high: {crit_count}");
    }

    /// Each RNG call increments the call counter.
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

    /// Snapshot captures state; restore brings it back exactly.
    #[test]
    fn test_snapshot_restore() {
        let mut rng = RNGManager::new(99);
        // Advance past a few calls
        rng.gen_range(0.0, 1.0);
        rng.gen_bool(0.5);
        let snap = rng.snapshot();

        // Advance some more
        let v1 = rng.gen_range(0.0, 1.0);
        let v2 = rng.gen_bool(0.5);

        // Restore and the next values should match
        rng.restore(snap);
        assert_eq!(rng.gen_range(0.0, 1.0), v1);
        assert_eq!(rng.gen_bool(0.5), v2);
    }

    /// After restore, call count is also restored.
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

    /// Multiple snapshots: can restore to an earlier point and replay forward.
    #[test]
    fn test_multiple_snapshots() {
        let mut rng = RNGManager::new(123);

        rng.gen_range(0.0, 1.0);
        let snap1 = rng.snapshot();

        let v1 = rng.gen_range(0.0, 1.0);
        let snap2 = rng.snapshot();

        let v2 = rng.gen_range(0.0, 1.0);

        // Restore to snap1 (after v0) → replay should produce v1, then v2
        rng.restore(snap1);
        assert_eq!(rng.gen_range(0.0, 1.0), v1);
        assert_eq!(rng.gen_range(0.0, 1.0), v2);

        // Restore to snap2 (after v1) → replay should produce v2
        rng.restore(snap2);
        assert_eq!(rng.gen_range(0.0, 1.0), v2);
    }

    /// 10 independent runs with the same seed produce the same sequence.
    #[test]
    fn test_deterministic_10_runs() {
        const RUNS: usize = 10;
        const CALLS: usize = 50;
        let mut results: Vec<Vec<u64>> = Vec::with_capacity(RUNS);

        for _ in 0..RUNS {
            let mut rng = RNGManager::new(777);
            let mut vals = Vec::with_capacity(CALLS);
            for _ in 0..CALLS {
                // Mix of different call types
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

    /// Snapshot restore never panics even on a fresh RNG.
    #[test]
    fn test_snapshot_restore_noop() {
        let mut rng = RNGManager::new(42);
        let snap = rng.snapshot();
        // restore immediately without any calls in between
        rng.restore(snap);
        assert_eq!(rng.call_count(), 0);
    }

    /// gen_range with negative bounds still works.
    #[test]
    fn test_gen_range_negative() {
        let mut rng = RNGManager::new(1);
        for _ in 0..100 {
            let v = rng.gen_range(-10.0, 10.0);
            assert!((-10.0..10.0).contains(&v), "out of bounds: {v}");
        }
    }

    /// seed() returns the seed provided at construction.
    #[test]
    fn test_seed_getter() {
        let rng = RNGManager::new(0xDEAD_BEEF);
        assert_eq!(rng.seed(), 0xDEAD_BEEF);
    }
}
