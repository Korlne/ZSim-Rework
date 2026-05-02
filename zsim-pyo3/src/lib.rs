// zsim-pyo3: Python extension module for ZSim 2.0.
// Exposes Parquet aggregation functions as Python-callable via PyO3.
//
// Integration testing with a real Python interpreter requires:
//   maturin develop  (or `pip install .` after a wheel build)

use std::path::Path;

use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use zsim_parquet::aggregator::{aggregate as pq_aggregate, AggQuery};

/// Convert an anyhow error to a Python IOError.
fn to_py_err(e: impl Into<anyhow::Error>) -> PyErr {
    PyIOError::new_err(e.into().to_string())
}

/// Python module providing Rust-parquet aggregation.
#[pymodule]
fn zsim_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(aggregate, m)?)?;
    m.add_function(wrap_pyfunction!(summary, m)?)?;
    m.add_function(wrap_pyfunction!(dps_curve, m)?)?;
    m.add_function(wrap_pyfunction!(damage_breakdown, m)?)?;
    Ok(())
}

/// Run an aggregation query against a Parquet file.
///
/// Args:
///     parquet_path — Path to the Parquet file.
///     query_json — JSON-serialized AggQuery, e.g. ``"TotalDamage"`` or
///                  ``{"DPS": {"window_ticks": 60}}``.
///
/// Returns:
///     JSON-serialized AggResult with ``type`` and ``data`` fields.
#[pyfunction]
fn aggregate(parquet_path: &str, query_json: &str) -> PyResult<String> {
    let query: AggQuery = serde_json::from_str(query_json)
        .map_err(|e| PyIOError::new_err(format!("Invalid query_json: {}", e)))?;
    let result = pq_aggregate(Path::new(parquet_path), &query).map_err(to_py_err)?;
    serde_json::to_string(&result)
        .map_err(|e| PyIOError::new_err(format!("Serialize error: {}", e)))
}

/// Summary statistics over all damage values in a Parquet file.
///
/// Args:
///     parquet_path — Path to the Parquet file.
///
/// Returns:
///     JSON-serialized StatsSummaryResult.
#[pyfunction]
fn summary(parquet_path: &str) -> PyResult<String> {
    let result =
        pq_aggregate(Path::new(parquet_path), &AggQuery::StatsSummary).map_err(to_py_err)?;
    serde_json::to_string(&result)
        .map_err(|e| PyIOError::new_err(format!("Serialize error: {}", e)))
}

/// DPS curve from a Parquet file using a sliding window.
///
/// Args:
///     parquet_path — Path to the Parquet file.
///     window_ticks — Width of the sliding window in ticks (1 tick = 1/60 s).
///
/// Returns:
///     JSON-serialized DPS points array.
#[pyfunction]
fn dps_curve(parquet_path: &str, window_ticks: u64) -> PyResult<String> {
    let result =
        pq_aggregate(Path::new(parquet_path), &AggQuery::DPS { window_ticks }).map_err(to_py_err)?;
    serde_json::to_string(&result)
        .map_err(|e| PyIOError::new_err(format!("Serialize error: {}", e)))
}

/// Damage breakdown by source entity from a Parquet file.
///
/// Args:
///     parquet_path — Path to the Parquet file.
///
/// Returns:
///     JSON-serialized DamageBreakdown map.
#[pyfunction]
fn damage_breakdown(parquet_path: &str) -> PyResult<String> {
    let result =
        pq_aggregate(Path::new(parquet_path), &AggQuery::DamageBreakdown).map_err(to_py_err)?;
    serde_json::to_string(&result)
        .map_err(|e| PyIOError::new_err(format!("Serialize error: {}", e)))
}
