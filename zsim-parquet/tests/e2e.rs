//! ZSim 模拟管道的端到端集成测试。
//!
//! 这些测试加载真实游戏数据，运行模拟，将结果写入 Parquet，
//! 并验证聚合器能够读取并生成合理的统计数据。
//! 它们还验证种子的可复现性。

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

/// 辅助函数：解析项目根目录（工作区根目录）。
fn project_root() -> PathBuf {
    // CARGO_MANIFEST_DIR 指向 zsim-parquet/
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().to_path_buf()
}

fn data_dir() -> PathBuf {
    project_root().join("data")
}

/// 从真实数据目录加载角色。
fn load_characters() -> Vec<zsim_core::entities::character::Character> {
    let path = data_dir().join("characters");
    zsim_core::data::loader::DataLoader::load_characters(&path)
        .expect("load_characters should succeed with real data")
}

/// 从真实数据目录加载敌人。
fn load_enemies() -> Vec<zsim_core::entities::enemy::EnemyState> {
    let path = data_dir().join("enemies");
    zsim_core::data::loader::DataLoader::load_enemies(&path)
        .expect("load_enemies should succeed with real data")
}

/// 从真实数据目录加载技能，返回以 action_id 为键的 HashMap。
fn load_skills_map() -> HashMap<String, zsim_core::combat::skill::SkillData> {
    let path = data_dir().join("skills");
    let skills: Vec<_> = zsim_core::data::loader::DataLoader::load_skills(&path)
        .expect("load_skills should succeed with real data");
    skills
        .into_iter()
        .map(|s| (s.action_id.clone(), s))
        .collect()
}

/// 加载示例 APL 文件。
fn load_apl() -> zsim_core::data::apl::APLData {
    let path = data_dir().join("apl").join("sample_apl.json");
    zsim_core::data::loader::DataLoader::load_apl(&path)
        .expect("load_apl should succeed with sample file")
}

/// 使用给定种子运行 N 次模拟，收集结果。
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

/// 辅助函数：将模拟结果写入 NamedTempFile 并返回 (tmp, path)。
/// 调用者在从 `path` 读取时必须保持 `tmp` 存活。
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
// 测试
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_data_loading() {
    // 验证所有真实数据文件可以端到端加载。
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
    // 使用并行运行器路径运行单次模拟并验证完成。
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
    // 使用真实数据并行运行 10 次模拟。
    let results = run_simulations(10, 42, 500);

    assert_eq!(results.len(), 10, "should return 10 results");

    // 所有模拟结果都应有终止原因。
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
    // 相同种子应在 10 次运行中产生相同的结果。
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
    // 运行模拟，写入 Parquet，用聚合器读回。
    use zsim_parquet::aggregator::{AggQuery, AggResult, ParquetAggregator};

    let results = run_simulations(5, 42, 200);
    let (_tmp, path) = write_parquet_temp(&results);

    // TotalDamage（如果没有事件可能为 0，但不应报错）
    let total = ParquetAggregator::aggregate(&path, &AggQuery::TotalDamage)
        .expect("TotalDamage aggregation");
    assert!(
        matches!(&total, AggResult::TotalDamage(v) if *v >= 0.0),
        "TotalDamage should be non-negative, got {:?}",
        total
    );

    // StatsSummary（统计摘要）
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

    // DPS（如果没有事件可能为空）
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

    // DamageBreakdown（如果没有事件可能为空）
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
    // 相同种子应产生相同的 Parquet 文件（哈希检查）。
    fn parquet_hash(seed: u64) -> Vec<u8> {
        let results = run_simulations(3, seed, 100);
        let (_tmp, path) = write_parquet_temp(&results);

        // 在 _tmp 存活时读取文件并计算哈希
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
    // 验证聚合结果在预期范围内。
    use zsim_parquet::aggregator::{AggQuery, AggResult, ParquetAggregator};

    let results = run_simulations(5, 42, 300);
    let (_tmp, path) = write_parquet_temp(&results);

    // CritRate 应在 0 到 1 之间
    let crit = ParquetAggregator::aggregate(&path, &AggQuery::CritRate)
        .expect("CritRate aggregation");
    if let AggResult::CritRate(r) = &crit {
        assert!(
            r.crit_rate >= 0.0 && r.crit_rate <= 1.0,
            "CritRate should be in [0, 1], got {}",
            r.crit_rate
        );
    }

    // AnomalyStats 应为非负
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

    // StunStats 应为非负
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
