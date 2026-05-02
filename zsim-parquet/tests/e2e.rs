//! ZSim 模拟管道的端到端集成测试。
//!
//! 这些测试将游戏数据从 JSON 导入到 SQLite，然后加载数据，
//! 运行模拟，将结果写入 Parquet，并验证聚合器能够读取并生成合理的统计数据。
//! 它们还验证种子的可复现性。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;
use zsim_core::data::loader::DataLoader;

/// 辅助函数：解析项目根目录（工作区根目录）。
fn project_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().to_path_buf()
}

fn data_dir() -> PathBuf {
    project_root().join("data")
}

/// 创建一个 DataLoader，从真实数据目录的 JSON 文件导入到临时 SQLite 数据库。
fn make_loader() -> (DataLoader, TempDir) {
    let tmp = TempDir::new().expect("create temp dir");
    let db_path = tmp.path().join("zsim.db");
    let loader = DataLoader::from_json_dir(&db_path, &data_dir())
        .expect("DataLoader::from_json_dir should succeed with real data");
    (loader, tmp)
}

/// 使用真实数据运行 N 次模拟，返回结果。
fn run_simulations(
    sim_count: usize,
    base_seed: u64,
    max_tick: u64,
) -> Vec<zsim_core::combat::parallel::SimResult> {
    let (loader, _tmp) = make_loader();

    let characters = Arc::new(
        loader.load_characters().expect("load_characters should succeed"),
    );
    let enemies = Arc::new(
        loader.load_enemies().expect("load_enemies should succeed"),
    );
    let skills: Arc<HashMap<String, zsim_core::combat::skill::SkillData>> = Arc::new(
        loader
            .load_all_skills()
            .expect("load_all_skills should succeed")
            .into_iter()
            .map(|s| (s.action_id.clone(), s))
            .collect(),
    );
    let apl = Arc::new(
        loader
            .load_apl("sample_apl")
            .expect("load_apl should succeed with sample file"),
    );

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

/// 辅助函数：将模拟结果写入 NamedTempFile 并返回 (tmp, path)。
fn write_parquet_temp(
    results: &[zsim_core::combat::parallel::SimResult],
) -> (tempfile::NamedTempFile, PathBuf) {
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    let path = tmp.path().to_path_buf();
    let mut writer = zsim_parquet::writer::ParquetWriter::new(&path).expect("create writer");
    writer.write_batch(results).expect("write batch");
    writer.close().expect("close writer");
    std::io::Write::flush(&mut tmp).expect("flush tempfile");
    (tmp, path)
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_data_loading() {
    let (loader, _tmp) = make_loader();

    let characters = loader.load_characters().expect("load characters");
    assert!(!characters.is_empty(), "at least one character");

    let enemies = loader.load_enemies().expect("load enemies");
    assert!(!enemies.is_empty(), "at least one enemy");

    let skills = loader.load_all_skills().expect("load all skills");
    assert!(!skills.is_empty(), "at least one skill");

    let apl = loader.load_apl("sample_apl").expect("load APL");
    assert!(!apl.tracks.is_empty(), "at least one APL track");
}

#[test]
fn test_e2e_single_simulation_with_real_data() {
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
    let results = run_simulations(10, 42, 500);
    assert_eq!(results.len(), 10, "should return 10 results");
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
    use zsim_parquet::aggregator::{AggQuery, AggResult, ParquetAggregator};

    let results = run_simulations(5, 42, 200);
    let (_tmp, path) = write_parquet_temp(&results);

    let total = ParquetAggregator::aggregate(&path, &AggQuery::TotalDamage)
        .expect("TotalDamage aggregation");
    assert!(
        matches!(&total, AggResult::TotalDamage(v) if *v >= 0.0),
        "TotalDamage should be non-negative, got {:?}",
        total
    );

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

    let breakdown = ParquetAggregator::aggregate(&path, &AggQuery::DamageBreakdown)
        .expect("DamageBreakdown aggregation");
    assert!(
        matches!(&breakdown, AggResult::DamageBreakdown(_)),
        "Expected DamageBreakdown result, got {:?}",
        breakdown
    );
}

#[test]
fn test_e2e_parquet_seed_reproducibility() {
    fn parquet_hash(seed: u64) -> Vec<u8> {
        let results = run_simulations(3, seed, 100);
        let (_tmp, path) = write_parquet_temp(&results);

        let bytes = std::fs::read(&path).expect("read parquet file");
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish().to_le_bytes().to_vec()
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
    use zsim_parquet::aggregator::{AggQuery, AggResult, ParquetAggregator};

    let results = run_simulations(5, 42, 300);
    let (_tmp, path) = write_parquet_temp(&results);

    let crit = ParquetAggregator::aggregate(&path, &AggQuery::CritRate)
        .expect("CritRate aggregation");
    if let AggResult::CritRate(r) = &crit {
        assert!(
            r.crit_rate >= 0.0 && r.crit_rate <= 1.0,
            "CritRate should be in [0, 1], got {}",
            r.crit_rate
        );
    }

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

    let stun = ParquetAggregator::aggregate(&path, &AggQuery::StunStats)
        .expect("StunStats aggregation");
    if let AggResult::StunStats(s) = &stun {
        assert!(
            s.total_stun_damage >= 0.0,
            "stun damage should be non-negative"
        );
    }
}
