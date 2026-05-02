// zsim-pyo3：ZSim 2.0 的 Python 扩展模块。
// 通过 PyO3 将 Parquet 聚合函数暴露为 Python 可调用的接口。
//
// 使用真实 Python 解释器进行集成测试需要：
//   maturin develop（或构建 wheel 后执行 `pip install .`）

use std::path::Path;

use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use zsim_parquet::aggregator::{aggregate as pq_aggregate, AggQuery};

/// 将 anyhow 错误转换为 Python IOError。
fn to_py_err(e: impl Into<anyhow::Error>) -> PyErr {
    PyIOError::new_err(e.into().to_string())
}

/// 提供 Rust-Parquet 聚合功能的 Python 模块。
#[pymodule]
fn zsim_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(aggregate, m)?)?;
    m.add_function(wrap_pyfunction!(summary, m)?)?;
    m.add_function(wrap_pyfunction!(dps_curve, m)?)?;
    m.add_function(wrap_pyfunction!(damage_breakdown, m)?)?;
    Ok(())
}

/// 对 Parquet 文件执行聚合查询。
///
/// 参数：
///     parquet_path — Parquet 文件的路径。
///     query_json — JSON 序列化的 AggQuery，例如 ``"TotalDamage"`` 或
///                  ``{"DPS": {"window_ticks": 60}}``。
///
/// 返回：
///     包含 ``type`` 和 ``data`` 字段的 JSON 序列化 AggResult。
#[pyfunction]
fn aggregate(parquet_path: &str, query_json: &str) -> PyResult<String> {
    let query: AggQuery = serde_json::from_str(query_json)
        .map_err(|e| PyIOError::new_err(format!("Invalid query_json: {}", e)))?;
    let result = pq_aggregate(Path::new(parquet_path), &query).map_err(to_py_err)?;
    serde_json::to_string(&result)
        .map_err(|e| PyIOError::new_err(format!("Serialize error: {}", e)))
}

/// 对 Parquet 文件中所有伤害值进行汇总统计。
///
/// 参数：
///     parquet_path — Parquet 文件的路径。
///
/// 返回：
///     JSON 序列化的 StatsSummaryResult。
#[pyfunction]
fn summary(parquet_path: &str) -> PyResult<String> {
    let result =
        pq_aggregate(Path::new(parquet_path), &AggQuery::StatsSummary).map_err(to_py_err)?;
    serde_json::to_string(&result)
        .map_err(|e| PyIOError::new_err(format!("Serialize error: {}", e)))
}

/// 使用滑动窗口计算 Parquet 文件的 DPS 曲线。
///
/// 参数：
///     parquet_path — Parquet 文件的路径。
///     window_ticks — 滑动窗口的宽度，以 tick 为单位（1 tick = 1/60 秒）。
///
/// 返回：
///     JSON 序列化的 DPS 点数组。
#[pyfunction]
fn dps_curve(parquet_path: &str, window_ticks: u64) -> PyResult<String> {
    let result = pq_aggregate(Path::new(parquet_path), &AggQuery::DPS { window_ticks })
        .map_err(to_py_err)?;
    serde_json::to_string(&result)
        .map_err(|e| PyIOError::new_err(format!("Serialize error: {}", e)))
}

/// 按来源实体统计 Parquet 文件的伤害分布。
///
/// 参数：
///     parquet_path — Parquet 文件的路径。
///
/// 返回：
///     JSON 序列化的 DamageBreakdown 映射。
#[pyfunction]
fn damage_breakdown(parquet_path: &str) -> PyResult<String> {
    let result =
        pq_aggregate(Path::new(parquet_path), &AggQuery::DamageBreakdown).map_err(to_py_err)?;
    serde_json::to_string(&result)
        .map_err(|e| PyIOError::new_err(format!("Serialize error: {}", e)))
}
