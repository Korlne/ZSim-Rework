//! End-to-end integration tests for the ZSim simulation pipeline.
//!
//! These tests load real game data, run simulations, write results to Parquet,
//! and verify that the aggregator can read and produce sensible statistics.
//! They also verify seed reproducibility.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

/// Helper: resolve the project root (the workspace root).
fn project_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = zsim-parquet/
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().to_path_buf()
}

fn data_dir() -> PathBuf {
    project_root().join("data")
}

/// Load characters from the real data directory.
fn load_characters() -> Vec<zsim_core::entities::character::Character> {
    let path = data_dir().join("characters");
    zsim_core::data::loader::DataLoader::load_characters(&path)
        .expect("load_characters should succeed with real data")
}

/// Load enemies from the real data directory.
fn load_enemies() -> Vec<zsim_core::entities::enemy::EnemyState> {
    let path = data_dir().join("enemies");
    zsim_core::data::loader::DataLoader::load_enemies(&path)
        .expect("load_enemies should succeed with real data")
}

/// Load skills from the real data directory, returned as a HashMap keyed by action_id.
fn load_skills_map() -> HashMap<String, zsim_core::combat::skill::SkillData> {
    let path = data_dir().join("skills");
    let skills: Vec<_> = zsim_core::data::loader::DataLoader::load_skills(&path)
        .expect("load_skills should succeed with real data");
    skills
        .into_iter()
        .map(|s| (s.action_id.clone(), s))
        .collect()
}

/// Load the sample APL file.
fn load_apl() -> zsim_core::data::apl::APLData {
    let path = data_dir().join("apl").join("sample_apl.json");
    zsim_core::data::loader::DataLoader::load_apl(&path)
        .expect("load_apl should succeed with sample file")
}

/// Run N simulations with the given seed, collect results.
fn run_simulations(
    sim_count: usize,
    base_seed: u64,
    max_tick: u64,
) -> Vec<zsim_core::combat::parallel::SimResult> {
    let characters = Arc::new(load_characters());
    let enemies = Arc::new(load_enemies());
    let skills = Arc::new(load_skills_map());
    let apl = Arc::new(load_apl());

    let config = zsim_core::combat::parallel::ParallelConfig {
        sim_count,
        base_seed,
        max_tick,
        mode: zsim_core::entities::enums::SimMode::Single,
        team_characters: characters,
        enemies,
        skills,
        apl,
        bangboo: Arc::new(None),
    };

    let (results, _progress_rx) = zsim_core::combat::parallel::ParallelRunner::run(config);
    results
}

/// Helper: write simulation results to a NamedTempFile and return (tmp, path).
/// The caller must keep `tmp` alive while reading from `path`.
fn write_parquet_temp(
    results: &[zsim_core::combat::parallel::SimResult],
) -> (tempfile::NamedTempFile, PathBuf) {
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    let path = tmp.path().to_path_buf();
    let mut writer = zsim_parquet::writer::ParquetWriter::new(&path).expect("create writer");
    writer.write_batch(results).expect("write batch");
    writer.close().expect("close writer");
    tmp.flush().expect("flush tempfile");
    (tmp, path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_data_loading() {
    // Verify all real data files can be loaded end-to-end.
    let characters = load_characters();
    assert!(!characters.is_empty(), "at least one character");
    let enemies = load_enemies();
    assert!(!enemies.is_empty(), "at least one enemy");
    let skills = load_skills_map();
    assert!(!skills.is_empty(), "at least one skill");
    let apl = load_apl();
    assert!(!apl.tracks.is_empty(), "at least one APL track");
}

#[test]
fn test_e2e_single_simulation_with_real_data() {
    // Run a single simulation using the parallel-runner path and verify completion.
    let results = run_simulations(1, 42, 200);
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(
        result.total_ticks <= 200,
        "total_ticks ({}) should not exceed max_tick",
        result.total_ticks
    );
    assert!(
        result.termination_reason.is_some(),
        "termination_reason should be set"
    );
}

#[test]
fn test_e2e_parallel_10_simulations() {
    // Run 10 simulations in parallel with real data.
    let results = run_simulations(10, 42, 500);

    assert_eq!(results.len(), 10, "should return 10 results");

    // All should have a termination reason.
    for (i, r) in results.iter().enumerate() {
        assert!(
            r.termination_reason.is_some(),
            "result {} should have termination reason",
            i
        );
    }
}

#[test]
fn test_e2e_seed_reproducibility_10_runs() {
    // Same seed should produce identical results across 10 runs.
    let results_a = run_simulations(10, 99, 200);
    let results_b = run_simulations(10, 99, 200);

    assert_eq!(results_a.len(), results_b.len());

    for (i, (a, b)) in results_a.iter().zip(results_b.iter()).enumerate() {
        assert_eq!(
            a.total_ticks, b.total_ticks,
            "run {}: total_ticks mismatch (seed reproducibility)",
            i
        );
        assert_eq!(
            a.termination_reason, b.termination_reason,
            "run {}: termination_reason mismatch",
            i
        );
        assert_eq!(
            a.events.len(),
            b.events.len(),
            "run {}: event count mismatch",
            i
        );
    }
}

#[test]
fn test_e2e_write_parquet_and_aggregate() {
    // Run simulations, write to Parquet, read back with aggregator.
    use zsim_parquet::aggregator::{AggQuery, AggResult, ParquetAggregator};

    let results = run_simulations(5, 42, 200);
    let (_tmp, path) = write_parquet_temp(&results);

    // TotalDamage (may be 0 if no events, but should not error)
    let total = ParquetAggregator::aggregate(&path, &AggQuery::TotalDamage)
        .expect("TotalDamage aggregation");
    assert!(
        matches!(&total, AggResult::TotalDamage(v) if *v >= 0.0),
        "TotalDamage should be non-negative, got {:?}",
        total
    );

    // StatsSummary
    let summary = ParquetAggregator::aggregate(&path, &AggQuery::StatsSummary)
        .expect("StatsSummary aggregation");
    if let AggResult::StatsSummary(stats) = &summary {
        assert!(
            stats.mean.is_finite(),
            "StatsSummary mean should be finite, got {}",
            stats.mean
        );
    } else {
        panic!("Expected StatsSummary result, got {:?}", summary);
    }

    // DPS (may be empty with no events)
    let dps = ParquetAggregator::aggregate(
        &path,
        &AggQuery::DPS {
            window_ticks: 60,
        },
    )
    .expect("DPS aggregation");
    assert!(
        matches!(&dps, AggResult::DPS(_)),
        "Expected DPS result, got {:?}",
        dps
    );

    // DamageBreakdown (may be empty with no events)
    let breakdown = ParquetAggregator::aggregate(&path, &AggQuery::DamageBreakdown)
        .expect("DamageBreakdown aggregation");
    assert!(
        matches!(&breakdown, AggResult::DamageBreakdown(_)),
        "Expected DamageBreakdown result, got {:?}",
        breakdown
    );

    // _tmp is dropped here, deleting the temp file
}

#[test]
fn test_e2e_parquet_seed_reproducibility() {
    // Same seed should produce identical Parquet files (hash check).
    fn parquet_hash(seed: u64) -> Vec<u8> {
        let results = run_simulations(3, seed, 100);
        let (_tmp, path) = write_parquet_temp(&results);

        // Read file and compute hash while _tmp is alive
        let bytes = std::fs::read(&path).expect("read parquet file");
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish().to_le_bytes().to_vec()
        // _tmp dropped here, temp file deleted
    }

    let hash_a = parquet_hash(42);
    let hash_b = parquet_hash(42);

    assert_eq!(
        hash_a, hash_b,
        "same seed should produce identical Parquet files"
    );
}

#[test]
fn test_e2e_aggregation_returns_sensible_values() {
    // Verify aggregation results are within expected ranges.
    use zsim_parquet::aggregator::{AggQuery, AggResult, ParquetAggregator};

    let results = run_simulations(5, 42, 300);
    let (_tmp, path) = write_parquet_temp(&results);

    // CritRate should be between 0 and 1
    let crit = ParquetAggregator::aggregate(&path, &AggQuery::CritRate)
        .expect("CritRate aggregation");
    if let AggResult::CritRate(r) = &crit {
        assert!(
            r.crit_rate >= 0.0 && r.crit_rate <= 1.0,
            "CritRate should be in [0, 1], got {}",
            r.crit_rate
        );
    }

    // AnomalyStats should be non-negative
    let anomaly = ParquetAggregator::aggregate(&path, &AggQuery::AnomalyStats)
        .expect("AnomalyStats aggregation");
    if let AggResult::AnomalyStats(stats) = &anomaly {
        assert!(
            stats.total_anomaly_damage >= 0.0,
            "anomaly damage should be non-negative"
        );
        assert!(
            stats.total_gauge >= 0.0,
            "anomaly gauge should be non-negative"
        );
        for (_, element_stats) in &stats.per_element {
            assert!(
                element_stats.damage >= 0.0,
                "element damage should be non-negative"
            );
            assert!(
                element_stats.gauge >= 0.0,
                "element gauge should be non-negative"
            );
        }
    }

    // StunStats should be non-negative
    let stun = ParquetAggregator::aggregate(&path, &AggQuery::StunStats)
        .expect("StunStats aggregation");
    if let AggResult::StunStats(s) = &stun {
        assert!(
            s.total_stun_damage >= 0.0,
            "stun damage should be non-negative"
        );
    }

    // _tmp dropped here, temp file deleted
}
