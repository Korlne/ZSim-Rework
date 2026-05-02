//! ZSim 2.0 模拟结果的 Parquet 聚合查询。
//!
//! 提供对 [`crate::writer`] 定义的 15 列 Parquet 模式的列式读取和聚合。
//! 每种查询类型使用 **投影下推**，仅从文件中读取它需要的列，从而最小化 I/O。
//!
//! | 查询                  | 读取的列                                           |
//! |----------------------|----------------------------------------------------|
//! | `TotalDamage`        | event_type, damage                                 |
//! | `DPS`                | tick, event_type, damage                           |
//! | `DamageBreakdown`    | event_type, source_id, damage                      |
//! | `AnomalyStats`       | event_type, element, damage, anomaly_gauge         |
//! | `StunStats`          | stun_dmg                                           |
//! | `CritRate`           | event_type, crit                                   |
//! | `StatsSummary`       | event_type, damage                                 |

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};
use arrow::array::*;
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ProjectionMask;
use serde::{Deserialize, Serialize};

// ── 查询 / 结果类型 ──────────────────────────────────────────────────

/// 聚合查询类型。
///
/// 每个变体选择不同的聚合算法。所有查询
/// 都作用于 [`ParquetWriter`](crate::writer::ParquetWriter) 产生的 15 列 Parquet 模式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggQuery {
    /// 所有造成伤害的总和。
    TotalDamage,
    /// 带滑动窗口的每秒伤害时间序列。
    DPS {
        /// 滑动窗口的宽度，以 tick 为单位（1 tick = 1/60 秒）。
        window_ticks: u64,
    },
    /// 按来源实体分组的伤害。
    DamageBreakdown,
    /// 异常与紊乱统计。
    AnomalyStats,
    /// 眩晕 / 昏迷伤害统计。
    StunStats,
    /// 暴击率统计。
    CritRate,
    /// 所有伤害值的描述性统计。
    StatsSummary,
}

/// 类型化的聚合结果。
///
/// 序列化为 `{"type": "<VariantName>", "data": <value>}` 格式，以便
/// Python / Tauri 侧车进程轻松消费。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum AggResult {
    TotalDamage(f64),
    DPS(Vec<DpsPoint>),
    DamageBreakdown(HashMap<String, f64>),
    AnomalyStats(AnomalyStatsResult),
    StunStats(StunStatsResult),
    CritRate(CritRateResult),
    StatsSummary(StatsSummaryResult),
}

/// DPS 时间序列中的单个数据点。
#[derive(Debug, Clone, Serialize)]
pub struct DpsPoint {
    /// 窗口的起始 tick（包含）。
    pub tick_start: u64,
    /// 窗口的结束 tick（不包含）。
    pub tick_end: u64,
    /// 此窗口中的原始伤害总和。
    pub total_damage: f64,
    /// 每秒伤害（total_damage / window_duration_seconds）。
    pub dps: f64,
}

/// 跨所有元素的聚合异常统计。
#[derive(Debug, Clone, Serialize)]
pub struct AnomalyStatsResult {
    /// 异常触发总次数。
    pub total_triggers: u64,
    /// 异常伤害总和。
    pub total_anomaly_damage: f64,
    /// 累计异常能量总值。
    pub total_gauge: f64,
    /// 按元素的细分。
    pub per_element: HashMap<String, ElementAnomalyStats>,
}

/// 按元素的异常统计。
#[derive(Debug, Clone, Serialize)]
pub struct ElementAnomalyStats {
    /// 此元素的触发次数。
    pub triggers: u64,
    /// 此元素的异常伤害总和。
    pub damage: f64,
    /// 此元素累计的能量总值。
    pub gauge: f64,
}

/// 眩晕 / 昏迷伤害统计。
#[derive(Debug, Clone, Serialize)]
pub struct StunStatsResult {
    /// 造成的总眩晕伤害。
    pub total_stun_damage: f64,
    /// 包含眩晕伤害的事件数量。
    pub total_stun_events: u64,
}

/// 暴击率统计。
#[derive(Debug, Clone, Serialize)]
pub struct CritRateResult {
    /// 伤害命中总数（包括非暴击）。
    pub total_hits: u64,
    /// 暴击命中数。
    pub crit_hits: u64,
    /// 暴击率（crit_hits / total_hits）。
    pub crit_rate: f64,
}

/// 一组伤害值的描述性统计。
#[derive(Debug, Clone, Serialize)]
pub struct StatsSummaryResult {
    /// 伤害事件数量。
    pub count: u64,
    /// 算术平均值。
    pub mean: f64,
    /// 总体方差。
    pub variance: f64,
    /// 总体标准差。
    pub std_dev: f64,
    /// 最小值。
    pub min: f64,
    /// 最大值。
    pub max: f64,
    /// 第 50 百分位数（中位数）。
    pub p50: f64,
    /// 第 90 百分位数。
    pub p90: f64,
    /// 第 95 百分位数。
    pub p95: f64,
    /// 第 99 百分位数。
    pub p99: f64,
}

// ── 聚合器 ───────────────────────────────────────────────────────────

/// ZSim Parquet 文件的列式聚合器。
///
/// 无状态——所有状态在每次 [`aggregate`](ParquetAggregator::aggregate) 调用中都是临时的。
pub struct ParquetAggregator;

impl ParquetAggregator {
    /// 对 Parquet 文件运行聚合查询。
    ///
    /// 文件必须使用 [`ParquetWriter`](crate::writer::ParquetWriter) 产生的 15 列模式。
    pub fn aggregate(path: &Path, query: &AggQuery) -> Result<AggResult> {
        match query {
            AggQuery::TotalDamage => Ok(AggResult::TotalDamage(aggregate_total_damage(path)?)),
            AggQuery::DPS { window_ticks } => {
                Ok(AggResult::DPS(aggregate_dps(path, *window_ticks)?))
            }
            AggQuery::DamageBreakdown => Ok(AggResult::DamageBreakdown(
                aggregate_damage_breakdown(path)?,
            )),
            AggQuery::AnomalyStats => Ok(AggResult::AnomalyStats(aggregate_anomaly_stats(path)?)),
            AggQuery::StunStats => Ok(AggResult::StunStats(aggregate_stun_stats(path)?)),
            AggQuery::CritRate => Ok(AggResult::CritRate(aggregate_crit_rate(path)?)),
            AggQuery::StatsSummary => Ok(AggResult::StatsSummary(aggregate_stats_summary(path)?)),
        }
    }
}

/// 包装 [`ParquetAggregator::aggregate`] 的便捷自由函数。
pub fn aggregate(path: &Path, query: &AggQuery) -> Result<AggResult> {
    ParquetAggregator::aggregate(path, query)
}

// ── 列索引（原始 15 列模式）──────────────────────────────────────────

/// 15 列模式中的列索引。
#[allow(dead_code)]
mod col {
    pub const SIM_INDEX: usize = 0;
    pub const TICK: usize = 1;
    pub const EVENT_TYPE: usize = 2;
    pub const SOURCE_ID: usize = 3;
    pub const TARGET_ID: usize = 4;
    pub const ACTION_ID: usize = 5;
    pub const DAMAGE: usize = 6;
    pub const CRIT: usize = 7;
    pub const ELEMENT: usize = 8;
    pub const ANOMALY_GAUGE: usize = 9;
    pub const STUN_DMG: usize = 10;
    pub const BUFF_ID: usize = 11;
    pub const BUFF_VALUE: usize = 12;
    pub const COORDINATED_FLAG: usize = 13;
    pub const TIMESTAMP: usize = 14;
}

// ── 投影读取器 ────────────────────────────────────────────────────────

/// 使用列投影打开 Parquet 文件并返回所有行组。
///
/// 只有 `columns` 中列出的列（原始 15 列模式的基于 0 的索引）会从文件中反序列化。
/// 返回的批次仅包含投影后的列，按 `columns` 指定的顺序通过基于 0 的索引访问。
fn read_projected(path: &Path, columns: &[usize]) -> Result<Vec<RecordBatch>> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open Parquet file: {}", path.display()))?;

    let mut builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .with_context(|| format!("Failed to create Parquet reader: {}", path.display()))?;

    if !columns.is_empty() {
        let parquet_schema = builder.metadata().file_metadata().schema_descr();
        let mask = ProjectionMask::leaves(parquet_schema, columns.iter().copied());
        builder = builder.with_projection(mask);
    }

    let reader = builder.build()?;
    let batches: Result<Vec<_>, _> = reader.collect();
    batches.context("Failed to read Parquet row groups")
}

// ── 列访问辅助函数 ────────────────────────────────────────────────────

/// 返回 `batch` 中第 `idx` 列的引用，向下转换为 `T` 类型。
macro_rules! get_col {
    ($batch:expr, $idx:expr, $ty:ty) => {
        $batch
            .column($idx)
            .as_any()
            .downcast_ref::<$ty>()
            .with_context(|| format!("Column {} is not the expected type", $idx))?
    };
}

/// 遍历 [`StringArray`] 的行，生成 `Option<String>`。
fn iter_string(col_idx: usize, batch: &RecordBatch) -> Result<Vec<Option<String>>> {
    let arr = get_col!(batch, col_idx, StringArray);
    Ok((0..arr.len())
        .map(|i| {
            if arr.is_null(i) {
                None
            } else {
                Some(arr.value(i).to_string())
            }
        })
        .collect())
}

/// 遍历 [`Float64Array`] 的行，生成 `Option<f64>`。
fn iter_f64(col_idx: usize, batch: &RecordBatch) -> Result<Vec<Option<f64>>> {
    let arr = get_col!(batch, col_idx, Float64Array);
    Ok((0..arr.len())
        .map(|i| {
            if arr.is_null(i) {
                None
            } else {
                Some(arr.value(i))
            }
        })
        .collect())
}

/// 遍历 [`UInt64Array`] 的行，生成 `u64`。
fn iter_u64(col_idx: usize, batch: &RecordBatch) -> Result<Vec<u64>> {
    let arr = get_col!(batch, col_idx, UInt64Array);
    Ok((0..arr.len()).map(|i| arr.value(i)).collect())
}

/// 遍历 [`BooleanArray`] 的行，生成 `Option<bool>`。
fn iter_bool(col_idx: usize, batch: &RecordBatch) -> Result<Vec<Option<bool>>> {
    let arr = get_col!(batch, col_idx, BooleanArray);
    Ok((0..arr.len())
        .map(|i| {
            if arr.is_null(i) {
                None
            } else {
                Some(arr.value(i))
            }
        })
        .collect())
}

// ── 聚合实现 ──────────────────────────────────────────────────────────

/// 对 DamageDealt 事件中所有非空伤害值求和。
///
/// 投影：[EVENT_TYPE(2), DAMAGE(6)] → 投影后 [0 = et, 1 = dmg]。
fn aggregate_total_damage(path: &Path) -> Result<f64> {
    let batches = read_projected(path, &[col::EVENT_TYPE, col::DAMAGE])?;
    let mut total = 0.0_f64;
    for batch in &batches {
        let event_types = iter_string(0, batch)?; // 投影列 0 = EVENT_TYPE
        let damages = iter_f64(1, batch)?; // 投影列 1 = DAMAGE
        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                if let Some(dmg) = damages[i] {
                    total += dmg;
                }
            }
        }
    }
    Ok(total)
}

/// 计算滑动窗口上的 DPS 时间序列。
///
/// 投影：[TICK(1), EVENT_TYPE(2), DAMAGE(6)] → 投影后 [0 = tick, 1 = et, 2 = dmg]。
fn aggregate_dps(path: &Path, window_ticks: u64) -> Result<Vec<DpsPoint>> {
    if window_ticks == 0 {
        return Ok(Vec::new());
    }

    let batches = read_projected(path, &[col::TICK, col::EVENT_TYPE, col::DAMAGE])?;

    let mut events: Vec<(u64, f64)> = Vec::new();
    for batch in &batches {
        let ticks = iter_u64(0, batch)?; // 投影列 0 = TICK
        let event_types = iter_string(1, batch)?; // 投影列 1 = EVENT_TYPE
        let damages = iter_f64(2, batch)?; // 投影列 2 = DAMAGE

        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                if let Some(dmg) = damages[i] {
                    events.push((ticks[i], dmg));
                }
            }
        }
    }

    if events.is_empty() {
        return Ok(Vec::new());
    }

    let min_tick = events.iter().map(|(t, _)| *t).min().unwrap_or(0);
    let max_tick = events.iter().map(|(t, _)| *t).max().unwrap_or(0);

    let first_window = min_tick / window_ticks;
    let last_window = max_tick / window_ticks;
    let num_windows = (last_window - first_window + 1) as usize;
    let mut window_damage = vec![0.0_f64; num_windows];

    for (tick, dmg) in &events {
        let idx = (tick / window_ticks - first_window) as usize;
        if idx < window_damage.len() {
            window_damage[idx] += dmg;
        }
    }

    let window_duration_secs = window_ticks as f64 / 60.0;
    Ok(window_damage
        .into_iter()
        .enumerate()
        .map(|(i, total)| {
            let win_idx = first_window + i as u64;
            let tick_start = win_idx * window_ticks;
            let tick_end = tick_start + window_ticks;
            DpsPoint {
                tick_start,
                tick_end,
                total_damage: total,
                dps: if window_duration_secs > 0.0 {
                    total / window_duration_secs
                } else {
                    0.0
                },
            }
        })
        .collect())
}

/// 按来源实体分组伤害。
///
/// 投影：[EVENT_TYPE(2), SOURCE_ID(3), DAMAGE(6)] → 投影后 [0 = et, 1 = src, 2 = dmg]。
fn aggregate_damage_breakdown(path: &Path) -> Result<HashMap<String, f64>> {
    let batches = read_projected(path, &[col::EVENT_TYPE, col::SOURCE_ID, col::DAMAGE])?;
    let mut breakdown: HashMap<String, f64> = HashMap::new();

    for batch in &batches {
        let event_types = iter_string(0, batch)?; // 投影列 0 = EVENT_TYPE
        let sources = iter_string(1, batch)?; // 投影列 1 = SOURCE_ID
        let damages = iter_f64(2, batch)?; // 投影列 2 = DAMAGE

        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                let src = sources[i].as_deref().unwrap_or("unknown");
                if let Some(dmg) = damages[i] {
                    *breakdown.entry(src.to_string()).or_insert(0.0) += dmg;
                }
            }
        }
    }

    Ok(breakdown)
}

/// 聚合异常统计。
///
/// 投影：[EVENT_TYPE(2), DAMAGE(6), ELEMENT(8), ANOMALY_GAUGE(9)]
///             （按原始模式顺序排序）
///             → 投影后 [0 = et, 1 = dmg, 2 = el, 3 = ag]。
fn aggregate_anomaly_stats(path: &Path) -> Result<AnomalyStatsResult> {
    let batches = read_projected(
        path,
        &[
            col::EVENT_TYPE,
            col::ELEMENT,
            col::DAMAGE,
            col::ANOMALY_GAUGE,
        ],
    )?;

    let mut total_triggers = 0_u64;
    let mut total_anomaly_damage = 0.0_f64;
    let mut total_gauge = 0.0_f64;
    let mut per_element: HashMap<String, ElementAnomalyStats> = HashMap::new();

    for batch in &batches {
        let event_types = iter_string(0, batch)?; // 投影列 0 = EVENT_TYPE
        let damages = iter_f64(1, batch)?; // 投影列 1 = DAMAGE
        let elements = iter_string(2, batch)?; // 投影列 2 = ELEMENT
        let gauges = iter_f64(3, batch)?; // 投影列 3 = ANOMALY_GAUGE

        for (i, et) in event_types.iter().enumerate() {
            let elem = elements[i].as_deref().unwrap_or("unknown").to_string();

            if et.as_deref() == Some("AnomalyTriggered")
                || et.as_deref() == Some("DisorderTriggered")
            {
                total_triggers += 1;
                if let Some(dmg) = damages[i] {
                    total_anomaly_damage += dmg;
                    let entry = per_element
                        .entry(elem.clone())
                        .or_insert(ElementAnomalyStats {
                            triggers: 0,
                            damage: 0.0,
                            gauge: 0.0,
                        });
                    entry.triggers += 1;
                    entry.damage += dmg;
                }
            }

            if let Some(gauge) = gauges[i] {
                let entry = per_element.entry(elem).or_insert(ElementAnomalyStats {
                    triggers: 0,
                    damage: 0.0,
                    gauge: 0.0,
                });
                entry.gauge += gauge;
                total_gauge += gauge;
            }
        }
    }

    Ok(AnomalyStatsResult {
        total_triggers,
        total_anomaly_damage,
        total_gauge,
        per_element,
    })
}

/// 聚合眩晕伤害统计。
///
/// 投影：[STUN_DMG(10)] → 投影后 [0 = stun]。
fn aggregate_stun_stats(path: &Path) -> Result<StunStatsResult> {
    let batches = read_projected(path, &[col::STUN_DMG])?;

    let mut total_stun_damage = 0.0_f64;
    let mut total_stun_events = 0_u64;

    for batch in &batches {
        let stun_dmgs = iter_f64(0, batch)?; // 投影列 0 = STUN_DMG

        for v in stun_dmgs.iter().flatten() {
            total_stun_damage += v;
            total_stun_events += 1;
        }
    }

    Ok(StunStatsResult {
        total_stun_damage,
        total_stun_events,
    })
}

/// 聚合暴击率。
///
/// 投影：[EVENT_TYPE(2), CRIT(7)] → 投影后 [0 = et, 1 = crit]。
fn aggregate_crit_rate(path: &Path) -> Result<CritRateResult> {
    let batches = read_projected(path, &[col::EVENT_TYPE, col::CRIT])?;

    let mut total_hits = 0_u64;
    let mut crit_hits = 0_u64;

    for batch in &batches {
        let event_types = iter_string(0, batch)?; // 投影列 0 = EVENT_TYPE
        let crits = iter_bool(1, batch)?; // 投影列 1 = CRIT

        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                total_hits += 1;
                if let Some(true) = crits[i] {
                    crit_hits += 1;
                }
            }
        }
    }

    Ok(CritRateResult {
        total_hits,
        crit_hits,
        crit_rate: if total_hits > 0 {
            crit_hits as f64 / total_hits as f64
        } else {
            0.0
        },
    })
}

/// 计算所有伤害值的描述性统计。
///
/// 投影：[EVENT_TYPE(2), DAMAGE(6)] → 投影后 [0 = et, 1 = dmg]。
fn aggregate_stats_summary(path: &Path) -> Result<StatsSummaryResult> {
    let batches = read_projected(path, &[col::EVENT_TYPE, col::DAMAGE])?;

    let mut values: Vec<f64> = Vec::new();
    for batch in &batches {
        let event_types = iter_string(0, batch)?; // 投影列 0 = EVENT_TYPE
        let damages = iter_f64(1, batch)?; // 投影列 1 = DAMAGE

        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                if let Some(dmg) = damages[i] {
                    values.push(dmg);
                }
            }
        }
    }

    let count = values.len() as u64;

    if count == 0 {
        return Ok(StatsSummaryResult {
            count: 0,
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
            p50: 0.0,
            p90: 0.0,
            p95: 0.0,
            p99: 0.0,
        });
    }

    // Welford 在线算法，用于均值和方差（单次遍历）
    let mut n = 0.0_f64;
    let mut mean = 0.0_f64;
    let mut m2 = 0.0_f64;
    let mut min_val = f64::MAX;
    let mut max_val = f64::NEG_INFINITY;

    for &x in &values {
        n += 1.0;
        let delta = x - mean;
        mean += delta / n;
        let delta2 = x - mean;
        m2 += delta * delta2;
        if x < min_val {
            min_val = x;
        }
        if x > max_val {
            max_val = x;
        }
    }

    let variance = if count > 1 { m2 / count as f64 } else { 0.0 };
    let std_dev = variance.sqrt();

    // 百分位数：排序值，然后索引（R7 方法）
    values.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    Ok(StatsSummaryResult {
        count,
        mean,
        variance,
        std_dev,
        min: min_val,
        max: max_val,
        p50: percentile(&values, 50.0),
        p90: percentile(&values, 90.0),
        p95: percentile(&values, 95.0),
        p99: percentile(&values, 99.0),
    })
}

/// 从已排序的切片中计算第 p 个百分位数。
///
/// 使用相邻值之间的线性插值（类型 R7，与
/// numpy / pandas 默认值相同）。
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    // R7 公式：index = p/100 * (n - 1)
    let n = sorted.len();
    let idx = p / 100.0 * (n - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    let frac = idx - lo as f64;

    if lo >= n - 1 || hi >= n {
        sorted[n - 1]
    } else if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] + frac * (sorted[hi] - sorted[lo])
    }
}

// ── 测试 ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::write_results;
    use std::fs;
    use zsim_core::combat::parallel::SimResult;
    use zsim_core::combat::runner::LoggedEvent;
    use zsim_core::events::signals::EventType;

    // ------------------------------------------------------------------
    // 辅助函数
    // ------------------------------------------------------------------

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("zsim_aggregator_test");
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    fn make_event(
        tick: u64,
        event_type: EventType,
        source_id: Option<&str>,
        target_id: Option<&str>,
        action_id: Option<&str>,
        damage: Option<f64>,
        is_crit: Option<bool>,
        element: Option<&str>,
        anomaly_gauge: Option<f64>,
        stun_dmg: Option<f64>,
    ) -> LoggedEvent {
        LoggedEvent {
            tick,
            event_type,
            source_id: source_id.map(String::from),
            target_id: target_id.map(String::from),
            action_id: action_id.map(String::from),
            damage,
            is_crit,
            element: element.map(String::from),
            anomaly_gauge,
            stun_dmg,
            buff_id: None,
            buff_value: None,
            coordinated_flag: None,
        }
    }

    fn make_result(
        sim_index: usize,
        seed: u64,
        total_ticks: u64,
        reason: &str,
        events: Vec<LoggedEvent>,
    ) -> SimResult {
        SimResult {
            sim_index,
            seed,
            total_ticks,
            termination_reason: Some(reason.to_string()),
            events,
        }
    }

    fn write_test_data(path: &Path, results: Vec<SimResult>) {
        write_results(results, path).expect("write test data");
    }

    // ------------------------------------------------------------------
    // TotalDamage（总伤害）
    // ------------------------------------------------------------------

    #[test]
    fn test_total_damage_basic() {
        let path = temp_path("td_basic.parquet");
        let events = vec![
            make_event(
                0,
                EventType::ActionStart,
                Some("a"),
                None,
                Some("s1"),
                None,
                None,
                None,
                None,
                None,
            ),
            make_event(
                1,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(100.0),
                Some(false),
                None,
                None,
                None,
            ),
            make_event(
                2,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(250.5),
                Some(true),
                None,
                None,
                None,
            ),
            make_event(
                3,
                EventType::TickStart,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
        ];
        write_test_data(&path, vec![make_result(0, 42, 10, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::TotalDamage).unwrap();
        match result {
            AggResult::TotalDamage(dmg) => assert!((dmg - 350.5).abs() < 1e-9),
            _ => panic!("expected TotalDamage"),
        }
    }

    #[test]
    fn test_total_damage_no_damage_events() {
        let path = temp_path("td_none.parquet");
        let events = vec![
            make_event(
                0,
                EventType::TickStart,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            make_event(
                1,
                EventType::ActionStart,
                Some("a"),
                None,
                Some("s1"),
                None,
                None,
                None,
                None,
                None,
            ),
        ];
        write_test_data(&path, vec![make_result(0, 42, 5, "no_dmg", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::TotalDamage).unwrap();
        match result {
            AggResult::TotalDamage(dmg) => assert!((dmg - 0.0).abs() < 1e-9),
            _ => panic!("expected TotalDamage"),
        }
    }

    #[test]
    fn test_total_damage_multiple_results() {
        let path = temp_path("td_multi.parquet");
        let results: Vec<SimResult> = (0..3)
            .map(|i| {
                make_result(
                    i,
                    42 + i as u64,
                    5,
                    "test",
                    vec![make_event(
                        0,
                        EventType::DamageDealt,
                        Some("a"),
                        Some("e"),
                        Some("s1"),
                        Some(100.0 * (i + 1) as f64),
                        Some(false),
                        None,
                        None,
                        None,
                    )],
                )
            })
            .collect();
        write_test_data(&path, results);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::TotalDamage).unwrap();
        match result {
            AggResult::TotalDamage(dmg) => assert!((dmg - 600.0).abs() < 1e-9),
            _ => panic!("expected TotalDamage"),
        }
    }

    // ------------------------------------------------------------------
    // DPS（每秒伤害）
    // ------------------------------------------------------------------

    #[test]
    fn test_dps_basic() {
        let path = temp_path("dps_basic.parquet");
        let events: Vec<LoggedEvent> = (0..60)
            .map(|t| {
                make_event(
                    t,
                    EventType::DamageDealt,
                    Some("a"),
                    Some("e"),
                    Some("s1"),
                    Some(100.0),
                    Some(false),
                    None,
                    None,
                    None,
                )
            })
            .collect();
        write_test_data(&path, vec![make_result(0, 42, 60, "test", events)]);

        let result =
            ParquetAggregator::aggregate(&path, &AggQuery::DPS { window_ticks: 60 }).unwrap();
        match result {
            AggResult::DPS(points) => {
                assert_eq!(points.len(), 1);
                assert_eq!(points[0].tick_start, 0);
                assert_eq!(points[0].tick_end, 60);
                assert!((points[0].total_damage - 6000.0).abs() < 1e-9);
                assert!((points[0].dps - 6000.0).abs() < 1e-9);
            }
            _ => panic!("expected DPS"),
        }
    }

    #[test]
    fn test_dps_multiple_windows() {
        let path = temp_path("dps_multi.parquet");
        let events: Vec<LoggedEvent> = (0..120)
            .map(|t| {
                make_event(
                    t,
                    EventType::DamageDealt,
                    Some("a"),
                    Some("e"),
                    Some("s1"),
                    Some(50.0),
                    Some(false),
                    None,
                    None,
                    None,
                )
            })
            .collect();
        write_test_data(&path, vec![make_result(0, 42, 120, "test", events)]);

        let result =
            ParquetAggregator::aggregate(&path, &AggQuery::DPS { window_ticks: 60 }).unwrap();
        match result {
            AggResult::DPS(points) => {
                assert_eq!(points.len(), 2, "should have 2 windows for 120 ticks");
                assert!((points[0].total_damage - 3000.0).abs() < 1e-9);
                assert!((points[0].dps - 3000.0).abs() < 1e-9);
                assert!((points[1].total_damage - 3000.0).abs() < 1e-9);
                assert!((points[1].dps - 3000.0).abs() < 1e-9);
            }
            _ => panic!("expected DPS"),
        }
    }

    #[test]
    fn test_dps_no_events() {
        let path = temp_path("dps_empty.parquet");
        write_test_data(&path, vec![make_result(0, 42, 0, "empty", vec![])]);

        let result =
            ParquetAggregator::aggregate(&path, &AggQuery::DPS { window_ticks: 60 }).unwrap();
        match result {
            AggResult::DPS(points) => assert!(points.is_empty()),
            _ => panic!("expected DPS"),
        }
    }

    // ------------------------------------------------------------------
    // DamageBreakdown（伤害明细）
    // ------------------------------------------------------------------

    #[test]
    fn test_damage_breakdown_basic() {
        let path = temp_path("db_basic.parquet");
        let events = vec![
            make_event(
                0,
                EventType::DamageDealt,
                Some("char_0"),
                Some("e"),
                Some("atk"),
                Some(100.0),
                None,
                None,
                None,
                None,
            ),
            make_event(
                1,
                EventType::DamageDealt,
                Some("char_1"),
                Some("e"),
                Some("atk"),
                Some(200.0),
                None,
                None,
                None,
                None,
            ),
            make_event(
                2,
                EventType::DamageDealt,
                Some("char_0"),
                Some("e"),
                Some("atk"),
                Some(50.0),
                None,
                None,
                None,
                None,
            ),
            make_event(
                3,
                EventType::ActionStart,
                Some("char_0"),
                None,
                Some("atk"),
                None,
                None,
                None,
                None,
                None,
            ),
        ];
        write_test_data(&path, vec![make_result(0, 42, 10, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::DamageBreakdown).unwrap();
        match result {
            AggResult::DamageBreakdown(breakdown) => {
                assert_eq!(breakdown.len(), 2);
                assert!((breakdown["char_0"] - 150.0).abs() < 1e-9);
                assert!((breakdown["char_1"] - 200.0).abs() < 1e-9);
            }
            _ => panic!("expected DamageBreakdown"),
        }
    }

    #[test]
    fn test_damage_breakdown_unknown_source() {
        let path = temp_path("db_unknown.parquet");
        let events = vec![make_event(
            0,
            EventType::DamageDealt,
            None,
            Some("e"),
            Some("atk"),
            Some(99.0),
            None,
            None,
            None,
            None,
        )];
        write_test_data(&path, vec![make_result(0, 42, 5, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::DamageBreakdown).unwrap();
        match result {
            AggResult::DamageBreakdown(breakdown) => {
                assert!((breakdown.get("unknown").copied().unwrap_or(0.0) - 99.0).abs() < 1e-9);
            }
            _ => panic!("expected DamageBreakdown"),
        }
    }

    // ------------------------------------------------------------------
    // AnomalyStats（异常统计）
    // ------------------------------------------------------------------

    #[test]
    fn test_anomaly_stats_basic() {
        let path = temp_path("anomaly_basic.parquet");
        let events = vec![
            make_event(
                0,
                EventType::AnomalyTriggered,
                Some("a"),
                Some("e"),
                None,
                Some(5000.0),
                None,
                Some("Fire"),
                Some(100.0),
                None,
            ),
            make_event(
                10,
                EventType::DisorderTriggered,
                Some("a"),
                Some("e"),
                None,
                Some(8000.0),
                None,
                Some("Fire"),
                None,
                None,
            ),
            make_event(
                20,
                EventType::AnomalyTriggered,
                Some("a"),
                Some("e"),
                None,
                Some(3000.0),
                None,
                Some("Electric"),
                Some(100.0),
                None,
            ),
        ];
        write_test_data(&path, vec![make_result(0, 42, 30, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::AnomalyStats).unwrap();
        match result {
            AggResult::AnomalyStats(stats) => {
                assert_eq!(stats.total_triggers, 3);
                assert!((stats.total_anomaly_damage - 16000.0).abs() < 1e-9);
                assert!((stats.total_gauge - 200.0).abs() < 1e-9);
                assert_eq!(stats.per_element.len(), 2);
                let fire = &stats.per_element["Fire"];
                assert_eq!(fire.triggers, 2);
                assert!((fire.damage - 13000.0).abs() < 1e-9);
                assert!((fire.gauge - 100.0).abs() < 1e-9);
                let elec = &stats.per_element["Electric"];
                assert_eq!(elec.triggers, 1);
                assert!((elec.damage - 3000.0).abs() < 1e-9);
                assert!((elec.gauge - 100.0).abs() < 1e-9);
            }
            _ => panic!("expected AnomalyStats"),
        }
    }

    // ------------------------------------------------------------------
    // StunStats（眩晕统计）
    // ------------------------------------------------------------------

    #[test]
    fn test_stun_stats_basic() {
        let path = temp_path("stun_basic.parquet");
        let events = vec![
            make_event(
                0,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(100.0),
                None,
                None,
                None,
                Some(50.0),
            ),
            make_event(
                1,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s2"),
                Some(200.0),
                None,
                None,
                None,
                Some(75.5),
            ),
            make_event(
                2,
                EventType::TickStart,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            make_event(
                3,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s3"),
                Some(300.0),
                None,
                None,
                None,
                Some(30.0),
            ),
        ];
        write_test_data(&path, vec![make_result(0, 42, 10, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::StunStats).unwrap();
        match result {
            AggResult::StunStats(stats) => {
                assert!((stats.total_stun_damage - 155.5).abs() < 1e-9);
                assert_eq!(stats.total_stun_events, 3);
            }
            _ => panic!("expected StunStats"),
        }
    }

    #[test]
    fn test_stun_stats_no_stun() {
        let path = temp_path("stun_none.parquet");
        let events = vec![make_event(
            0,
            EventType::DamageDealt,
            Some("a"),
            Some("e"),
            Some("s1"),
            Some(100.0),
            None,
            None,
            None,
            None,
        )];
        write_test_data(&path, vec![make_result(0, 42, 5, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::StunStats).unwrap();
        match result {
            AggResult::StunStats(stats) => {
                assert!((stats.total_stun_damage - 0.0).abs() < 1e-9);
                assert_eq!(stats.total_stun_events, 0);
            }
            _ => panic!("expected StunStats"),
        }
    }

    // ------------------------------------------------------------------
    // CritRate（暴击率）
    // ------------------------------------------------------------------

    #[test]
    fn test_crit_rate_basic() {
        let path = temp_path("crit_basic.parquet");
        let events = vec![
            make_event(
                0,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(100.0),
                Some(true),
                None,
                None,
                None,
            ),
            make_event(
                1,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(100.0),
                Some(false),
                None,
                None,
                None,
            ),
            make_event(
                2,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(100.0),
                Some(true),
                None,
                None,
                None,
            ),
            make_event(
                3,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(100.0),
                Some(false),
                None,
                None,
                None,
            ),
            make_event(
                4,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(100.0),
                Some(false),
                None,
                None,
                None,
            ),
            make_event(
                5,
                EventType::TickStart,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
        ];
        write_test_data(&path, vec![make_result(0, 42, 10, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::CritRate).unwrap();
        match result {
            AggResult::CritRate(stats) => {
                assert_eq!(stats.total_hits, 5);
                assert_eq!(stats.crit_hits, 2);
                assert!((stats.crit_rate - 0.4).abs() < 1e-9);
            }
            _ => panic!("expected CritRate"),
        }
    }

    #[test]
    fn test_crit_rate_no_hits() {
        let path = temp_path("crit_none.parquet");
        write_test_data(&path, vec![make_result(0, 42, 0, "empty", vec![])]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::CritRate).unwrap();
        match result {
            AggResult::CritRate(stats) => {
                assert_eq!(stats.total_hits, 0);
                assert_eq!(stats.crit_hits, 0);
                assert!((stats.crit_rate - 0.0).abs() < 1e-9);
            }
            _ => panic!("expected CritRate"),
        }
    }

    #[test]
    fn test_crit_rate_null_crit() {
        let path = temp_path("crit_null.parquet");
        let events = vec![
            make_event(
                0,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(100.0),
                None,
                None,
                None,
                None,
            ),
            make_event(
                1,
                EventType::DamageDealt,
                Some("a"),
                Some("e"),
                Some("s1"),
                Some(200.0),
                Some(true),
                None,
                None,
                None,
            ),
        ];
        write_test_data(&path, vec![make_result(0, 42, 5, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::CritRate).unwrap();
        match result {
            AggResult::CritRate(stats) => {
                assert_eq!(stats.total_hits, 2);
                assert_eq!(stats.crit_hits, 1);
                assert!((stats.crit_rate - 0.5).abs() < 1e-9);
            }
            _ => panic!("expected CritRate"),
        }
    }

    // ------------------------------------------------------------------
    // StatsSummary（统计摘要）
    // ------------------------------------------------------------------

    #[test]
    fn test_stats_summary_basic() {
        let path = temp_path("ss_basic.parquet");
        let events: Vec<LoggedEvent> = (1..=10)
            .map(|i| {
                make_event(
                    i as u64,
                    EventType::DamageDealt,
                    Some("a"),
                    Some("e"),
                    Some("s1"),
                    Some(i as f64 * 10.0),
                    Some(false),
                    None,
                    None,
                    None,
                )
            })
            .collect();
        write_test_data(&path, vec![make_result(0, 42, 20, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::StatsSummary).unwrap();
        match result {
            AggResult::StatsSummary(stats) => {
                assert_eq!(stats.count, 10);
                assert!((stats.mean - 55.0).abs() < 1e-9);
                assert!((stats.min - 10.0).abs() < 1e-9);
                assert!((stats.max - 100.0).abs() < 1e-9);
                assert!(stats.variance > 0.0);
                assert!(stats.std_dev > 0.0);
                assert!((stats.p50 - 55.0).abs() < 1e-9);
                assert!((stats.p90 - 91.0).abs() < 1e-9);
                assert!((stats.p95 - 95.5).abs() < 1e-9);
            }
            _ => panic!("expected StatsSummary"),
        }
    }

    #[test]
    fn test_stats_summary_single_value() {
        let path = temp_path("ss_single.parquet");
        let events = vec![make_event(
            0,
            EventType::DamageDealt,
            Some("a"),
            Some("e"),
            Some("s1"),
            Some(42.0),
            Some(false),
            None,
            None,
            None,
        )];
        write_test_data(&path, vec![make_result(0, 42, 1, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::StatsSummary).unwrap();
        match result {
            AggResult::StatsSummary(stats) => {
                assert_eq!(stats.count, 1);
                assert!((stats.mean - 42.0).abs() < 1e-9);
                assert!((stats.variance - 0.0).abs() < 1e-9);
                assert!((stats.std_dev - 0.0).abs() < 1e-9);
                assert!((stats.min - 42.0).abs() < 1e-9);
                assert!((stats.max - 42.0).abs() < 1e-9);
                assert!((stats.p50 - 42.0).abs() < 1e-9);
                assert!((stats.p90 - 42.0).abs() < 1e-9);
            }
            _ => panic!("expected StatsSummary"),
        }
    }

    #[test]
    fn test_stats_summary_empty() {
        let path = temp_path("ss_empty.parquet");
        write_test_data(&path, vec![make_result(0, 42, 0, "empty", vec![])]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::StatsSummary).unwrap();
        match result {
            AggResult::StatsSummary(stats) => {
                assert_eq!(stats.count, 0);
                assert!((stats.mean - 0.0).abs() < 1e-9);
            }
            _ => panic!("expected StatsSummary"),
        }
    }

    // ------------------------------------------------------------------
    // 便捷函数
    // ------------------------------------------------------------------

    #[test]
    fn test_convenience_aggregate_fn() {
        let path = temp_path("convenience.parquet");
        let events = vec![make_event(
            0,
            EventType::DamageDealt,
            Some("a"),
            Some("e"),
            Some("s1"),
            Some(100.0),
            Some(false),
            None,
            None,
            None,
        )];
        write_test_data(&path, vec![make_result(0, 42, 5, "test", events)]);

        let result = aggregate(&path, &AggQuery::TotalDamage).unwrap();
        match result {
            AggResult::TotalDamage(dmg) => assert!((dmg - 100.0).abs() < 1e-9),
            _ => panic!("expected TotalDamage"),
        }
    }

    // ------------------------------------------------------------------
    // JSON 序列化
    // ------------------------------------------------------------------

    #[test]
    fn test_agg_result_serialization() {
        let result = AggResult::TotalDamage(1234.5);
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"TotalDamage\""));
        assert!(json.contains("1234.5"));

        let summary = AggResult::StatsSummary(StatsSummaryResult {
            count: 100,
            mean: 50.0,
            variance: 25.0,
            std_dev: 5.0,
            min: 10.0,
            max: 100.0,
            p50: 50.0,
            p90: 90.0,
            p95: 95.0,
            p99: 99.0,
        });
        let j2 = serde_json::to_string(&summary).unwrap();
        assert!(j2.contains("\"StatsSummary\""));
        assert!(j2.contains("\"mean\":50.0"));
        assert!(j2.contains("\"p99\":99.0"));
    }

    #[test]
    fn test_agg_query_deserialization() {
        let json = r#""TotalDamage""#;
        let query: AggQuery = serde_json::from_str(json).unwrap();
        assert!(matches!(query, AggQuery::TotalDamage));

        let json = r#"{"DPS": {"window_ticks": 60}}"#;
        let query: AggQuery = serde_json::from_str(json).unwrap();
        match query {
            AggQuery::DPS { window_ticks } => assert_eq!(window_ticks, 60),
            _ => panic!("expected DPS"),
        }
    }

    // ------------------------------------------------------------------
    // 文件未找到
    // ------------------------------------------------------------------

    #[test]
    fn test_aggregate_nonexistent_file() {
        let result = ParquetAggregator::aggregate(
            Path::new("/nonexistent/file.parquet"),
            &AggQuery::TotalDamage,
        );
        assert!(result.is_err(), "should error on missing file");
    }

    // ------------------------------------------------------------------
    // 一致性检查：TotalDamage 与 StatsSummary 求和
    // ------------------------------------------------------------------

    #[test]
    fn test_total_damage_matches_stats_summary_mean_times_count() {
        let path = temp_path("consistency.parquet");
        let events: Vec<LoggedEvent> = (0..10)
            .map(|i| {
                make_event(
                    i,
                    EventType::DamageDealt,
                    Some("a"),
                    Some("e"),
                    Some("atk"),
                    Some((i * 10) as f64),
                    Some(false),
                    None,
                    None,
                    None,
                )
            })
            .collect();
        write_test_data(&path, vec![make_result(0, 42, 20, "test", events)]);

        let total = match ParquetAggregator::aggregate(&path, &AggQuery::TotalDamage).unwrap() {
            AggResult::TotalDamage(d) => d,
            _ => panic!("expected TotalDamage"),
        };
        let summary = match ParquetAggregator::aggregate(&path, &AggQuery::StatsSummary).unwrap() {
            AggResult::StatsSummary(s) => s,
            _ => panic!("expected StatsSummary"),
        };

        assert!((total - summary.mean * summary.count as f64).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // 投影正确性：验证列投影工作正常
    // ------------------------------------------------------------------

    #[test]
    fn test_projection_reduces_column_count() {
        // StunStats 仅读取 1 列（stun_dmg）而非全部 15 列
        let path = temp_path("projection_col_count.parquet");
        let events = vec![make_event(
            0,
            EventType::DamageDealt,
            Some("a"),
            Some("e"),
            Some("s1"),
            Some(100.0),
            Some(true),
            Some("Fire"),
            Some(50.0),
            Some(25.0),
        )];
        write_test_data(&path, vec![make_result(0, 42, 5, "test", events)]);

        // 使用投影打开并验证内部批次仅有 1 列
        let batches = read_projected(&path, &[col::STUN_DMG]).unwrap();
        assert!(!batches.is_empty());
        assert_eq!(
            batches[0].num_columns(),
            1,
            "projection should yield 1 column for stun"
        );

        let result = ParquetAggregator::aggregate(&path, &AggQuery::StunStats).unwrap();
        match result {
            AggResult::StunStats(stats) => {
                assert!((stats.total_stun_damage - 25.0).abs() < 1e-9);
                assert_eq!(stats.total_stun_events, 1);
            }
            _ => panic!("expected StunStats"),
        }
    }

    #[test]
    fn test_projection_event_type_and_damage() {
        let path = temp_path("projection_et_dmg.parquet");
        let events = vec![make_event(
            0,
            EventType::DamageDealt,
            Some("a"),
            Some("e"),
            Some("s1"),
            Some(100.0),
            Some(false),
            None,
            None,
            None,
        )];
        write_test_data(&path, vec![make_result(0, 42, 5, "test", events)]);

        let batches = read_projected(&path, &[col::EVENT_TYPE, col::DAMAGE]).unwrap();
        assert!(!batches.is_empty());
        assert_eq!(
            batches[0].num_columns(),
            2,
            "projection should yield 2 columns"
        );
    }

    #[test]
    fn test_projection_tick_damage_dps() {
        let path = temp_path("projection_dps.parquet");
        let events = vec![make_event(
            0,
            EventType::DamageDealt,
            Some("a"),
            Some("e"),
            Some("s1"),
            Some(60.0),
            Some(false),
            None,
            None,
            None,
        )];
        write_test_data(&path, vec![make_result(0, 42, 1, "test", events)]);

        let batches = read_projected(&path, &[col::TICK, col::EVENT_TYPE, col::DAMAGE]).unwrap();
        assert_eq!(
            batches[0].num_columns(),
            3,
            "DPS projection should yield 3 columns"
        );
    }
}
