"""ZSim 2.0 analysis sidecar.

Long-running process that receives JSON commands via stdin and returns
JSON responses via stdout.  Uses the ``zsim_rs`` Rust extension module
for Parquet aggregation, and generates Plotly.js chart configurations.

Commands (one JSON object per line):
    {"summary": {"parquet_path": "..."}}
    {"dps_curve": {"parquet_path": "...", "window_ticks": 60}}
    {"damage_breakdown": {"parquet_path": "..."}}
    {"anomaly_timeline": {"parquet_path": "..."}}
    {"shutdown": true}

Responses (one JSON object per line):
    {"type": "ready"}                                  — on startup
    {"type": "summary", "data": {...}}                 — summary result
    {"type": "chart", "data": {...}}                   — Plotly chart config
    {"type": "error", "message": "..."}                — on error
"""

from __future__ import annotations

import json
import sys
import traceback
from typing import Any

try:
    import zsim_rs  # type: ignore[import-untyped]
except ImportError:
    zsim_rs = None  # type: ignore[assignment]
    # The module is required at runtime; tests mock sys.modules["zsim_rs"].


# ── Plotly chart builders ──────────────────────────────────────────────────


def _chart_dps_curve(dps_data: dict[str, Any]) -> dict[str, Any]:
    """Build a Plotly scatter trace from DPS points."""
    points = dps_data.get("DPS", [])
    if not points:
        return {"data": [], "layout": {"title": {"text": "DPS Curve (no data)"}}}

    return {
        "data": [
            {
                "x": [p["tick_start"] for p in points],
                "y": [p["dps"] for p in points],
                "type": "scatter",
                "mode": "lines",
                "name": "DPS",
                "line": {"color": "#2196F3", "width": 2},
            }
        ],
        "layout": {
            "title": {"text": "DPS Curve"},
            "xaxis": {"title": {"text": "Tick"}},
            "yaxis": {"title": {"text": "Damage / sec"}},
            "margin": {"t": 40, "b": 40, "l": 60, "r": 20},
            "hovermode": "x",
            "autosize": True,
        },
    }


def _chart_damage_breakdown(bd_data: dict[str, Any]) -> dict[str, Any]:
    """Build a Plotly pie chart from damage breakdown data."""
    bd = bd_data.get("DamageBreakdown", {})
    if not bd:
        return {
            "data": [],
            "layout": {"title": {"text": "Damage Breakdown (no data)"}},
        }

    labels = list(bd.keys())
    values = list(bd.values())
    colors = ["#FF6384", "#36A2EB", "#FFCE56", "#4BC0C0", "#9966FF", "#FF9F40"]

    return {
        "data": [
            {
                "labels": labels,
                "values": values,
                "type": "pie",
                "textinfo": "label+percent",
                "marker": {"colors": colors[: len(labels)]},
            }
        ],
        "layout": {
            "title": {"text": "Damage Breakdown by Source"},
            "margin": {"t": 40, "b": 20, "l": 20, "r": 20},
            "autosize": True,
        },
    }


def _chart_anomaly_stats(anomaly_data: dict[str, Any]) -> dict[str, Any]:
    """Build a grouped bar chart from anomaly stats."""
    stats = anomaly_data.get("AnomalyStats", {})
    per_element = stats.get("per_element", {})
    if not per_element:
        return {
            "data": [],
            "layout": {"title": {"text": "Anomaly Statistics (no data)"}},
        }

    elements = list(per_element.keys())
    damages = [per_element[e]["damage"] for e in elements]
    gauges = [per_element[e]["gauge"] for e in elements]
    triggers = [per_element[e]["triggers"] for e in elements]

    element_colors = {
        "Fire": "#FF5722",
        "Electric": "#FFEB3B",
        "Ice": "#00BCD4",
        "Physical": "#795548",
        "Ether": "#9C27B0",
    }
    bar_colors = [element_colors.get(e, "#9E9E9E") for e in elements]

    return {
        "data": [
            {
                "x": elements,
                "y": damages,
                "type": "bar",
                "name": "Anomaly Damage",
                "marker": {"color": bar_colors},
            },
            {
                "x": elements,
                "y": gauges,
                "type": "bar",
                "name": "Gauge Accumulated",
                "marker": {"color": bar_colors, "opacity": 0.5},
            },
            {
                "x": elements,
                "y": triggers,
                "type": "scatter",
                "mode": "markers",
                "name": "Triggers",
                "marker": {"color": "#F44336", "size": 10, "symbol": "diamond"},
                "yaxis": "y2",
            },
        ],
        "layout": {
            "title": {"text": "Anomaly Statistics by Element"},
            "barmode": "group",
            "xaxis": {"title": {"text": "Element"}},
            "yaxis": {"title": {"text": "Damage / Gauge"}},
            "yaxis2": {
                "title": {"text": "Triggers"},
                "overlaying": "y",
                "side": "right",
            },
            "margin": {"t": 40, "b": 40, "l": 60, "r": 60},
            "autosize": True,
        },
    }


def _chart_summary(summary_data: dict[str, Any]) -> dict[str, Any]:
    """Build summary stats data + percentile bar chart."""
    raw = summary_data.get("StatsSummary", {})
    if not raw:
        return {
            "summary": {},
            "chart": {"data": [], "layout": {"title": {"text": "No data"}}},
        }

    stats = {
        "count": raw.get("count", 0),
        "mean": raw.get("mean", 0.0),
        "std_dev": raw.get("std_dev", 0.0),
        "variance": raw.get("variance", 0.0),
        "min": raw.get("min", 0.0),
        "max": raw.get("max", 0.0),
        "p50": raw.get("p50", 0.0),
        "p90": raw.get("p90", 0.0),
        "p95": raw.get("p95", 0.0),
        "p99": raw.get("p99", 0.0),
    }

    p_labels = ["P50 (Median)", "P90", "P95", "P99"]
    p_values = [stats[p] for p in ["p50", "p90", "p95", "p99"]]
    p_colors = ["#4CAF50", "#FF9800", "#F44336", "#9C27B0"]

    chart = {
        "data": [
            {
                "x": p_labels,
                "y": p_values,
                "type": "bar",
                "marker": {"color": p_colors},
                "text": [f"{v:.1f}" for v in p_values],
                "textposition": "auto",
            }
        ],
        "layout": {
            "title": {"text": "Damage Distribution Percentiles"},
            "xaxis": {"title": {"text": "Percentile"}},
            "yaxis": {"title": {"text": "Damage"}},
            "margin": {"t": 40, "b": 40, "l": 60, "r": 20},
            "autosize": True,
        },
    }

    return {"summary": stats, "chart": chart}


# ── response helpers ────────────────────────────────────────────────────


def emit(obj: Any) -> None:
    """Write a JSON object to stdout, followed by a newline."""
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


# ── command dispatch ─────────────────────────────────────────────────────


def handle_command(cmd: dict[str, Any]) -> dict[str, Any]:
    """Process a single command and return the response dict."""
    try:
        # --- summary ---
        if "summary" in cmd:
            path = cmd["summary"]["parquet_path"]
            result_json = zsim_rs.summary(path)
            result = json.loads(result_json)
            payload = _chart_summary(result)
            return {"type": "summary", "data": payload}

        # --- dps_curve ---
        if "dps_curve" in cmd:
            params = cmd["dps_curve"]
            path = params["parquet_path"]
            window = params.get("window_ticks", 60)
            result_json = zsim_rs.dps_curve(path, window)
            result = json.loads(result_json)
            chart = _chart_dps_curve(result)
            return {"type": "chart", "data": chart}

        # --- damage_breakdown ---
        if "damage_breakdown" in cmd:
            path = cmd["damage_breakdown"]["parquet_path"]
            result_json = zsim_rs.damage_breakdown(path)
            result = json.loads(result_json)
            chart = _chart_damage_breakdown(result)
            return {"type": "chart", "data": chart}

        # --- anomaly_timeline ---
        if "anomaly_timeline" in cmd:
            path = cmd["anomaly_timeline"]["parquet_path"]
            result_json = zsim_rs.aggregate(path, '"AnomalyStats"')
            result = json.loads(result_json)
            chart = _chart_anomaly_stats(result)
            return {"type": "chart", "data": chart}

        # --- shutdown ---
        if "shutdown" in cmd:
            return {"type": "shutdown"}

        return {"type": "error", "message": f"Unknown command: {list(cmd.keys())}"}

    except Exception as exc:
        return {
            "type": "error",
            "message": f"{type(exc).__name__}: {exc}",
            "traceback": traceback.format_exc(),
        }


def main() -> None:
    """Main event loop — read commands, emit responses."""
    emit({"type": "ready"})

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            cmd = json.loads(line)
        except json.JSONDecodeError as exc:
            emit({"type": "error", "message": f"Invalid JSON: {exc}"})
            continue

        response = handle_command(cmd)
        emit(response)

        if response.get("type") == "shutdown":
            break


if __name__ == "__main__":
    main()
