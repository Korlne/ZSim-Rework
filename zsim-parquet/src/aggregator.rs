//! Parquet aggregation queries for ZSim 2.0 simulation results.
//!
//! Provides columnar read and aggregation over the 15-column Parquet schema
//! defined by [`crate::writer`].  Each query type reads only the columns it
//! needs from the Parquet file and produces a JSON-serializable result.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};
use arrow::array::*;
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::{Deserialize, Serialize};

// ── Query / Result types ─────────────────────────────────────────────────

/// Aggregation query type.
///
/// Each variant selects a different aggregation algorithm.  All queries
/// operate on the 15-column Parquet schema produced by [`ParquetWriter`](crate::writer::ParquetWriter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggQuery {
    /// Sum of all damage dealt.
    TotalDamage,
    /// Damage-per-second time series with sliding window.
    DPS {
        /// Width of the sliding window in ticks (1 tick = 1/60 s).
        window_ticks: u64,
    },
    /// Damage grouped by source entity.
    DamageBreakdown,
    /// Anomaly and disorder statistics.
    AnomalyStats,
    /// Stun / daze damage statistics.
    StunStats,
    /// Critical hit rate statistics.
    CritRate,
    /// Descriptive statistics over all damage values.
    StatsSummary,
}

/// Typed aggregation result.
///
/// Serialised as `{"type": "<VariantName>", "data": <value>}` for easy
/// consumption by Python / Tauri sidecars.
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

/// A single point in a DPS time series.
#[derive(Debug, Clone, Serialize)]
pub struct DpsPoint {
    /// First tick of the window (inclusive).
    pub tick_start: u64,
    /// Last tick of the window (exclusive).
    pub tick_end: u64,
    /// Raw damage sum in this window.
    pub total_damage: f64,
    /// Damage per second (total_damage / window_duration_seconds).
    pub dps: f64,
}

/// Aggregate anomaly statistics across all elements.
#[derive(Debug, Clone, Serialize)]
pub struct AnomalyStatsResult {
    /// Total number of anomaly triggers.
    pub total_triggers: u64,
    /// Sum of anomaly damage.
    pub total_anomaly_damage: f64,
    /// Total anomaly gauge accumulated.
    pub total_gauge: f64,
    /// Breakdown per element.
    pub per_element: HashMap<String, ElementAnomalyStats>,
}

/// Per-element anomaly statistics.
#[derive(Debug, Clone, Serialize)]
pub struct ElementAnomalyStats {
    /// Number of triggers for this element.
    pub triggers: u64,
    /// Total anomaly damage for this element.
    pub damage: f64,
    /// Total gauge accumulated for this element.
    pub gauge: f64,
}

/// Stun / daze damage statistics.
#[derive(Debug, Clone, Serialize)]
pub struct StunStatsResult {
    /// Total stun damage dealt.
    pub total_stun_damage: f64,
    /// Number of events containing stun damage.
    pub total_stun_events: u64,
}

/// Critical hit rate statistics.
#[derive(Debug, Clone, Serialize)]
pub struct CritRateResult {
    /// Total number of damage hits (including non-crits).
    pub total_hits: u64,
    /// Number of critical hits.
    pub crit_hits: u64,
    /// Critical hit rate (crit_hits / total_hits).
    pub crit_rate: f64,
}

/// Descriptive statistics over a set of damage values.
#[derive(Debug, Clone, Serialize)]
pub struct StatsSummaryResult {
    /// Number of damage events.
    pub count: u64,
    /// Arithmetic mean.
    pub mean: f64,
    /// Population variance.
    pub variance: f64,
    /// Population standard deviation.
    pub std_dev: f64,
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// 50th percentile (median).
    pub p50: f64,
    /// 90th percentile.
    pub p90: f64,
    /// 95th percentile.
    pub p95: f64,
    /// 99th percentile.
    pub p99: f64,
}

// ── Aggregator ───────────────────────────────────────────────────────────

/// Columnar aggregator over ZSim Parquet files.
///
/// Stateless — all state is ephemeral per [`aggregate`](ParquetAggregator::aggregate) call.
pub struct ParquetAggregator;

impl ParquetAggregator {
    /// Run an aggregation query against a Parquet file.
    ///
    /// The file must use the 15-column schema produced by
    /// [`ParquetWriter`](crate::writer::ParquetWriter).
    pub fn aggregate(path: &Path, query: &AggQuery) -> Result<AggResult> {
        match query {
            AggQuery::TotalDamage => {
                let batches = read_damage_batches(path)?;
                let total = aggregate_total_damage(&batches);
                Ok(AggResult::TotalDamage(total))
            }
            AggQuery::DPS { window_ticks } => {
                let batches = read_dps_batches(path)?;
                let points = aggregate_dps(&batches, *window_ticks);
                Ok(AggResult::DPS(points))
            }
            AggQuery::DamageBreakdown => {
                let batches = read_damage_breakdown_batches(path)?;
                let breakdown = aggregate_damage_breakdown(&batches);
                Ok(AggResult::DamageBreakdown(breakdown))
            }
            AggQuery::AnomalyStats => {
                let batches = read_anomaly_batches(path)?;
                let stats = aggregate_anomaly_stats(&batches);
                Ok(AggResult::AnomalyStats(stats))
            }
            AggQuery::StunStats => {
                let batches = read_stun_batches(path)?;
                let stats = aggregate_stun_stats(&batches);
                Ok(AggResult::StunStats(stats))
            }
            AggQuery::CritRate => {
                let batches = read_crit_batches(path)?;
                let stats = aggregate_crit_rate(&batches);
                Ok(AggResult::CritRate(stats))
            }
            AggQuery::StatsSummary => {
                let batches = read_damage_batches(path)?;
                let summary = aggregate_stats_summary(&batches);
                Ok(AggResult::StatsSummary(summary))
            }
        }
    }
}

/// Convenience free function wrapping [`ParquetAggregator::aggregate`].
pub fn aggregate(path: &Path, query: &AggQuery) -> Result<AggResult> {
    ParquetAggregator::aggregate(path, query)
}

// ── Parquet reading helpers ──────────────────────────────────────────────

/// Column indices in the 15-column schema.
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

/// Read all row groups from a Parquet file, optionally selecting columns.
fn read_all_batches(path: &Path) -> Result<Vec<RecordBatch>> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open Parquet file: {}", path.display()))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .with_context(|| format!("Failed to create Parquet reader: {}", path.display()))?;
    let reader = builder.build()?;
    let batches: Result<Vec<_>, _> = reader.collect();
    batches.context("Failed to read Parquet row groups")
}

/// Read batches filtering for DamageDealt events (columns: tick, event_type, damage).
fn read_damage_batches(path: &Path) -> Result<Vec<RecordBatch>> {
    read_all_batches(path)
}

/// Read batches for DPS query (columns: tick, event_type, damage).
fn read_dps_batches(path: &Path) -> Result<Vec<RecordBatch>> {
    read_all_batches(path)
}

/// Read batches for DamageBreakdown (columns: event_type, source_id, damage).
fn read_damage_breakdown_batches(path: &Path) -> Result<Vec<RecordBatch>> {
    read_all_batches(path)
}

/// Read batches for AnomalyStats (columns: event_type, element, damage, anomaly_gauge).
fn read_anomaly_batches(path: &Path) -> Result<Vec<RecordBatch>> {
    read_all_batches(path)
}

/// Read batches for StunStats (columns: event_type, stun_dmg).
fn read_stun_batches(path: &Path) -> Result<Vec<RecordBatch>> {
    read_all_batches(path)
}

/// Read batches for CritRate (columns: event_type, crit).
fn read_crit_batches(path: &Path) -> Result<Vec<RecordBatch>> {
    read_all_batches(path)
}

// ── Column access helpers ────────────────────────────────────────────────

/// Return a reference to column `idx` of `batch`, downcast to `T`.
macro_rules! get_col {
    ($batch:expr, $idx:expr, $ty:ty) => {
        $batch
            .column($idx)
            .as_any()
            .downcast_ref::<$ty>()
            .with_context(|| format!("Column {} is not the expected type", $idx))?
    };
}

/// Iterate over rows of a [`StringArray`], yielding `Option<&str>`.
fn iter_string(col_idx: usize, batch: &RecordBatch) -> Result<Vec<Option<String>>> {
    let arr = get_col!(batch, col_idx, StringArray);
    Ok((0..arr.len()).map(|i| {
        if arr.is_null(i) {
            None
        } else {
            Some(arr.value(i).to_string())
        }
    })
    .collect())
}

/// Iterate over rows of a [`Float64Array`], yielding `Option<f64>`.
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

/// Iterate over rows of a [`UInt64Array`], yielding `u64`.
fn iter_u64(col_idx: usize, batch: &RecordBatch) -> Result<Vec<u64>> {
    let arr = get_col!(batch, col_idx, UInt64Array);
    Ok((0..arr.len()).map(|i| arr.value(i)).collect())
}

/// Iterate over rows of a [`BooleanArray`], yielding `Option<bool>`.
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

// ── Aggregation implementations ──────────────────────────────────────────

/// Sum all non-null damage values in DamageDealt events.
fn aggregate_total_damage(batches: &[RecordBatch]) -> f64 {
    let mut total = 0.0_f64;
    for batch in batches {
        let event_types = match iter_string(col::EVENT_TYPE, batch) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let damages = match iter_f64(col::DAMAGE, batch) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                if let Some(dmg) = damages[i] {
                    total += dmg;
                }
            }
        }
    }
    total
}

/// Compute DPS time series over sliding windows.
fn aggregate_dps(batches: &[RecordBatch], window_ticks: u64) -> Vec<DpsPoint> {
    // Collect all (tick, damage) pairs for DamageDealt events
    let mut events: Vec<(u64, f64)> = Vec::new();
    for batch in batches {
        let Ok(event_types) = iter_string(col::EVENT_TYPE, batch) else { continue };
        let Ok(ticks) = iter_u64(col::TICK, batch) else { continue };
        let Ok(damages) = iter_f64(col::DAMAGE, batch) else { continue };

        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                if let Some(dmg) = damages[i] {
                    events.push((ticks[i], dmg));
                }
            }
        }
    }

    if events.is_empty() || window_ticks == 0 {
        return Vec::new();
    }

    // Determine tick range
    let min_tick = events.iter().map(|(t, _)| *t).min().unwrap_or(0);
    let max_tick = events.iter().map(|(t, _)| *t).max().unwrap_or(0);

    // Group into windows
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

    // Convert to DPS points
    let window_duration_secs = window_ticks as f64 / 60.0; // 60 ticks/s
    window_damage
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
        .collect()
}

/// Group damage by source entity.
fn aggregate_damage_breakdown(batches: &[RecordBatch]) -> HashMap<String, f64> {
    let mut breakdown: HashMap<String, f64> = HashMap::new();
    for batch in batches {
        let Ok(event_types) = iter_string(col::EVENT_TYPE, batch) else { continue };
        let Ok(sources) = iter_string(col::SOURCE_ID, batch) else { continue };
        let Ok(damages) = iter_f64(col::DAMAGE, batch) else { continue };

        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                let src = sources[i].as_deref().unwrap_or("unknown");
                if let Some(dmg) = damages[i] {
                    *breakdown.entry(src.to_string()).or_insert(0.0) += dmg;
                }
            }
        }
    }
    breakdown
}

/// Aggregate anomaly statistics.
fn aggregate_anomaly_stats(batches: &[RecordBatch]) -> AnomalyStatsResult {
    let mut total_triggers = 0_u64;
    let mut total_anomaly_damage = 0.0_f64;
    let mut total_gauge = 0.0_f64;
    let mut per_element: HashMap<String, ElementAnomalyStats> = HashMap::new();

    for batch in batches {
        let Ok(event_types) = iter_string(col::EVENT_TYPE, batch) else { continue };
        let Ok(elements) = iter_string(col::ELEMENT, batch) else { continue };
        let Ok(damages) = iter_f64(col::DAMAGE, batch) else { continue };
        let Ok(gauges) = iter_f64(col::ANOMALY_GAUGE, batch) else { continue };

        for (i, et) in event_types.iter().enumerate() {
            let elem = elements[i].as_deref().unwrap_or("unknown").to_string();

            if et.as_deref() == Some("AnomalyTriggered")
                || et.as_deref() == Some("DisorderTriggered")
            {
                total_triggers += 1;
                if let Some(dmg) = damages[i] {
                    // AnomalyTriggered has damage in the damage field; DisorderTriggered also
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

            // Track gauge accumulation — any event with anomaly_gauge set
            if let Some(gauge) = gauges[i] {
                let entry = per_element
                    .entry(elem)
                    .or_insert(ElementAnomalyStats {
                        triggers: 0,
                        damage: 0.0,
                        gauge: 0.0,
                    });
                entry.gauge += gauge;
                total_gauge += gauge;
            }
        }
    }

    AnomalyStatsResult {
        total_triggers,
        total_anomaly_damage,
        total_gauge,
        per_element,
    }
}

/// Aggregate stun damage statistics.
fn aggregate_stun_stats(batches: &[RecordBatch]) -> StunStatsResult {
    let mut total_stun_damage = 0.0_f64;
    let mut total_stun_events = 0_u64;

    for batch in batches {
        let Ok(stun_dmgs) = iter_f64(col::STUN_DMG, batch) else { continue };

        for v in stun_dmgs.iter().flatten() {
            total_stun_damage += v;
            total_stun_events += 1;
        }
    }

    StunStatsResult {
        total_stun_damage,
        total_stun_events,
    }
}

/// Aggregate critical hit rate.
fn aggregate_crit_rate(batches: &[RecordBatch]) -> CritRateResult {
    let mut total_hits = 0_u64;
    let mut crit_hits = 0_u64;

    for batch in batches {
        let Ok(event_types) = iter_string(col::EVENT_TYPE, batch) else { continue };
        let Ok(crits) = iter_bool(col::CRIT, batch) else { continue };

        for (i, et) in event_types.iter().enumerate() {
            if et.as_deref() == Some("DamageDealt") {
                total_hits += 1;
                if let Some(true) = crits[i] {
                    crit_hits += 1;
                }
            }
        }
    }

    CritRateResult {
        total_hits,
        crit_hits,
        crit_rate: if total_hits > 0 {
            crit_hits as f64 / total_hits as f64
        } else {
            0.0
        },
    }
}

/// Compute descriptive statistics over all damage values.
fn aggregate_stats_summary(batches: &[RecordBatch]) -> StatsSummaryResult {
    // Collect all damage values
    let mut values: Vec<f64> = Vec::new();
    for batch in batches {
        let Ok(event_types) = iter_string(col::EVENT_TYPE, batch) else { continue };
        let Ok(damages) = iter_f64(col::DAMAGE, batch) else { continue };

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
        return StatsSummaryResult {
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
        };
    }

    // Welford's online algorithm for mean + variance
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

    // Percentiles: sort values, then index
    values.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p50 = percentile(&values, 50.0);
    let p90 = percentile(&values, 90.0);
    let p95 = percentile(&values, 95.0);
    let p99 = percentile(&values, 99.0);

    StatsSummaryResult {
        count,
        mean,
        variance,
        std_dev,
        min: min_val,
        max: max_val,
        p50,
        p90,
        p95,
        p99,
    }
}

/// Compute the p-th percentile from a sorted slice.
///
/// Uses linear interpolation between adjacent values (type R7, same as
/// numpy / pandas default).
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    // R7 formula: index = p/100 * (n - 1)
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

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::write_results;
    use std::fs;
    use zsim_core::combat::parallel::SimResult;
    use zsim_core::combat::runner::LoggedEvent;
    use zsim_core::events::signals::EventType;

    // ------------------------------------------------------------------
    // Helpers
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
    // TotalDamage
    // ------------------------------------------------------------------

    #[test]
    fn test_total_damage_basic() {
        let path = temp_path("td_basic.parquet");
        let events = vec![
            make_event(0, EventType::ActionStart, Some("a"), None, Some("s1"), None, None, None, None, None),
            make_event(1, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), Some(false), None, None, None),
            make_event(2, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(250.5), Some(true), None, None, None),
            make_event(3, EventType::TickStart, None, None, None, None, None, None, None, None),
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
            make_event(0, EventType::TickStart, None, None, None, None, None, None, None, None),
            make_event(1, EventType::ActionStart, Some("a"), None, Some("s1"), None, None, None, None, None),
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
            AggResult::TotalDamage(dmg) => assert!((dmg - 600.0).abs() < 1e-9), // 100 + 200 + 300
            _ => panic!("expected TotalDamage"),
        }
    }

    // ------------------------------------------------------------------
    // DPS
    // ------------------------------------------------------------------

    #[test]
    fn test_dps_basic() {
        let path = temp_path("dps_basic.parquet");
        // 60 ticks of damage events, 1 event per tick, 100 damage each
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

        // Window = 60 ticks (1 second) → all damage in one window, DPS = 6000
        let result = ParquetAggregator::aggregate(&path, &AggQuery::DPS { window_ticks: 60 }).unwrap();
        match result {
            AggResult::DPS(points) => {
                assert_eq!(points.len(), 1);
                assert_eq!(points[0].tick_start, 0);
                assert_eq!(points[0].tick_end, 60);
                assert!((points[0].total_damage - 6000.0).abs() < 1e-9);
                assert!((points[0].dps - 6000.0).abs() < 1e-9); // 6000 dmg / 1 sec
            }
            _ => panic!("expected DPS"),
        }
    }

    #[test]
    fn test_dps_multiple_windows() {
        let path = temp_path("dps_multi.parquet");
        // 120 ticks (2 seconds) of damage, 50 damage per tick
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

        let result = ParquetAggregator::aggregate(&path, &AggQuery::DPS { window_ticks: 60 }).unwrap();
        match result {
            AggResult::DPS(points) => {
                assert_eq!(points.len(), 2, "should have 2 windows for 120 ticks");
                assert!((points[0].total_damage - 3000.0).abs() < 1e-9); // 60 * 50
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

        let result = ParquetAggregator::aggregate(&path, &AggQuery::DPS { window_ticks: 60 }).unwrap();
        match result {
            AggResult::DPS(points) => assert!(points.is_empty()),
            _ => panic!("expected DPS"),
        }
    }

    // ------------------------------------------------------------------
    // DamageBreakdown
    // ------------------------------------------------------------------

    #[test]
    fn test_damage_breakdown_basic() {
        let path = temp_path("db_basic.parquet");
        let events = vec![
            make_event(0, EventType::DamageDealt, Some("char_0"), Some("e"), Some("atk"), Some(100.0), None, None, None, None),
            make_event(1, EventType::DamageDealt, Some("char_1"), Some("e"), Some("atk"), Some(200.0), None, None, None, None),
            make_event(2, EventType::DamageDealt, Some("char_0"), Some("e"), Some("atk"), Some(50.0), None, None, None, None),
            make_event(3, EventType::ActionStart, Some("char_0"), None, Some("atk"), None, None, None, None, None),
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
        let events = vec![
            make_event(0, EventType::DamageDealt, None, Some("e"), Some("atk"), Some(99.0), None, None, None, None),
        ];
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
    // AnomalyStats
    // ------------------------------------------------------------------

    #[test]
    fn test_anomaly_stats_basic() {
        let path = temp_path("anomaly_basic.parquet");
        let events = vec![
            make_event(0, EventType::AnomalyTriggered, Some("a"), Some("e"), None, Some(5000.0), None, Some("Fire"), Some(100.0), None),
            make_event(10, EventType::DisorderTriggered, Some("a"), Some("e"), None, Some(8000.0), None, Some("Fire"), None, None),
            make_event(20, EventType::AnomalyTriggered, Some("a"), Some("e"), None, Some(3000.0), None, Some("Electric"), Some(100.0), None),
        ];
        write_test_data(&path, vec![make_result(0, 42, 30, "test", events)]);

        let result = ParquetAggregator::aggregate(&path, &AggQuery::AnomalyStats).unwrap();
        match result {
            AggResult::AnomalyStats(stats) => {
                assert_eq!(stats.total_triggers, 3);
                assert!((stats.total_anomaly_damage - 16000.0).abs() < 1e-9);
                assert!((stats.total_gauge - 200.0).abs() < 1e-9);
                assert_eq!(stats.per_element.len(), 2);
                // Fire: 2 triggers, 13000 damage, 100 gauge
                let fire = &stats.per_element["Fire"];
                assert_eq!(fire.triggers, 2);
                assert!((fire.damage - 13000.0).abs() < 1e-9);
                assert!((fire.gauge - 100.0).abs() < 1e-9);
                // Electric: 1 trigger, 3000 damage, 100 gauge
                let elec = &stats.per_element["Electric"];
                assert_eq!(elec.triggers, 1);
                assert!((elec.damage - 3000.0).abs() < 1e-9);
                assert!((elec.gauge - 100.0).abs() < 1e-9);
            }
            _ => panic!("expected AnomalyStats"),
        }
    }

    // ------------------------------------------------------------------
    // StunStats
    // ------------------------------------------------------------------

    #[test]
    fn test_stun_stats_basic() {
        let path = temp_path("stun_basic.parquet");
        let events = vec![
            make_event(0, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), None, None, None, Some(50.0)),
            make_event(1, EventType::DamageDealt, Some("a"), Some("e"), Some("s2"), Some(200.0), None, None, None, Some(75.5)),
            make_event(2, EventType::TickStart, None, None, None, None, None, None, None, None),
            make_event(3, EventType::DamageDealt, Some("a"), Some("e"), Some("s3"), Some(300.0), None, None, None, Some(30.0)),
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
        let events = vec![
            make_event(0, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), None, None, None, None),
        ];
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
    // CritRate
    // ------------------------------------------------------------------

    #[test]
    fn test_crit_rate_basic() {
        let path = temp_path("crit_basic.parquet");
        let events = vec![
            make_event(0, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), Some(true), None, None, None),
            make_event(1, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), Some(false), None, None, None),
            make_event(2, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), Some(true), None, None, None),
            make_event(3, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), Some(false), None, None, None),
            make_event(4, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), Some(false), None, None, None),
            make_event(5, EventType::TickStart, None, None, None, None, None, None, None, None),
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
            make_event(0, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), None, None, None, None),
            make_event(1, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(200.0), Some(true), None, None, None),
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
    // StatsSummary
    // ------------------------------------------------------------------

    #[test]
    fn test_stats_summary_basic() {
        let path = temp_path("ss_basic.parquet");
        // Values: 10, 20, 30, 40, 50, 60, 70, 80, 90, 100
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
                // Percentiles for evenly-spaced {10,20,...,100}
                assert!((stats.p50 - 55.0).abs() < 1e-9); // median of 10 items = avg(5th,6th) = avg(50,60) = 55
                assert!((stats.p90 - 91.0).abs() < 1e-9); // R7: 0.9*9 = 8.1, 90 + 0.1*10 = 91
                assert!((stats.p95 - 95.5).abs() < 1e-9); // R7: 0.95*9 = 8.55, 90 + 0.55*10 = 95.5
            }
            _ => panic!("expected StatsSummary"),
        }
    }

    #[test]
    fn test_stats_summary_single_value() {
        let path = temp_path("ss_single.parquet");
        let events = vec![make_event(0, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(42.0), Some(false), None, None, None)];
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
    // Convenience function
    // ------------------------------------------------------------------

    #[test]
    fn test_convenience_aggregate_fn() {
        let path = temp_path("convenience.parquet");
        let events = vec![
            make_event(0, EventType::DamageDealt, Some("a"), Some("e"), Some("s1"), Some(100.0), Some(false), None, None, None),
        ];
        write_test_data(&path, vec![make_result(0, 42, 5, "test", events)]);

        let result = aggregate(&path, &AggQuery::TotalDamage).unwrap();
        match result {
            AggResult::TotalDamage(dmg) => assert!((dmg - 100.0).abs() < 1e-9),
            _ => panic!("expected TotalDamage"),
        }
    }

    // ------------------------------------------------------------------
    // JSON serialization
    // ------------------------------------------------------------------

    #[test]
    fn test_agg_result_serialization() {
        // Verify that AggResult serializes with type+data tags
        let result = AggResult::TotalDamage(1234.5);
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"TotalDamage\""));
        assert!(json.contains("1234.5"));

        // Verify StatsSummary serialization
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
        // External tagging: unit variant → bare string
        let json = r#""TotalDamage""#;
        let query: AggQuery = serde_json::from_str(json).unwrap();
        assert!(matches!(query, AggQuery::TotalDamage));

        // Struct-like variant → {"VariantName": {field: value}}
        let json = r#"{"DPS": {"window_ticks": 60}}"#;
        let query: AggQuery = serde_json::from_str(json).unwrap();
        match query {
            AggQuery::DPS { window_ticks } => assert_eq!(window_ticks, 60),
            _ => panic!("expected DPS"),
        }
    }

    // ------------------------------------------------------------------
    // File not found
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
    // Consistency: TotalDamage vs StatsSummary sum
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

        // Total should equal mean * count
        assert!((total - summary.mean * summary.count as f64).abs() < 1e-9);
    }
}
