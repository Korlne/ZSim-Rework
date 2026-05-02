"""Tests for analysis_sidecar chart builders and command dispatch.

These tests do NOT require the ``zsim_rs`` Rust extension module.  The chart
builders are pure functions that don't use ``zsim_rs``, and ``handle_command``
is tested by swapping a mock into ``analysis_sidecar.zsim_rs``.
"""

from __future__ import annotations

import json
import os
import sys
import unittest
from unittest import mock

# Ensure the project root is on sys.path so we can import analysis_sidecar.
_project_root = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..")
)
if _project_root not in sys.path:
    sys.path.insert(0, _project_root)

import analysis_sidecar as sidecar


class ChartBuildersTest(unittest.TestCase):
    """Test each Plotly chart builder with known input data."""

    # ------------------------------------------------------------------
    # _chart_dps_curve
    # ------------------------------------------------------------------

    def test_dps_curve_with_data(self) -> None:
        data = {
            "DPS": [
                {"tick_start": 0, "dps": 100.0},
                {"tick_start": 60, "dps": 150.0},
                {"tick_start": 120, "dps": 200.0},
            ]
        }
        result = sidecar._chart_dps_curve(data)
        traces = result["data"]
        self.assertEqual(len(traces), 1)
        self.assertEqual(traces[0]["type"], "scatter")
        self.assertEqual(traces[0]["x"], [0, 60, 120])
        self.assertEqual(traces[0]["y"], [100.0, 150.0, 200.0])
        self.assertEqual(result["layout"]["title"]["text"], "DPS Curve")

    def test_dps_curve_empty(self) -> None:
        result = sidecar._chart_dps_curve({"DPS": []})
        self.assertEqual(result["data"], [])
        self.assertIn("no data", result["layout"]["title"]["text"])

    def test_dps_curve_missing_key(self) -> None:
        result = sidecar._chart_dps_curve({})
        self.assertEqual(result["data"], [])
        self.assertIn("no data", result["layout"]["title"]["text"])

    # ------------------------------------------------------------------
    # _chart_damage_breakdown
    # ------------------------------------------------------------------

    def test_damage_breakdown_with_data(self) -> None:
        data = {"DamageBreakdown": {"normal": 500.0, "skill": 1200.0}}
        result = sidecar._chart_damage_breakdown(data)
        traces = result["data"]
        self.assertEqual(len(traces), 1)
        self.assertEqual(traces[0]["type"], "pie")
        self.assertEqual(traces[0]["labels"], ["normal", "skill"])
        self.assertEqual(traces[0]["values"], [500.0, 1200.0])

    def test_damage_breakdown_empty(self) -> None:
        result = sidecar._chart_damage_breakdown({"DamageBreakdown": {}})
        self.assertEqual(result["data"], [])

    def test_damage_breakdown_missing_key(self) -> None:
        result = sidecar._chart_damage_breakdown({})
        self.assertEqual(result["data"], [])

    # ------------------------------------------------------------------
    # _chart_anomaly_stats
    # ------------------------------------------------------------------

    def test_anomaly_stats_with_data(self) -> None:
        data = {
            "AnomalyStats": {
                "per_element": {
                    "Fire": {"damage": 300.0, "gauge": 80.0, "triggers": 3},
                    "Ice": {"damage": 150.0, "gauge": 40.0, "triggers": 1},
                }
            }
        }
        result = sidecar._chart_anomaly_stats(data)
        traces = result["data"]
        self.assertEqual(len(traces), 3)
        self.assertEqual(traces[0]["type"], "bar")
        self.assertEqual(traces[1]["type"], "bar")
        self.assertEqual(traces[2]["type"], "scatter")

    def test_anomaly_stats_empty(self) -> None:
        result = sidecar._chart_anomaly_stats(
            {"AnomalyStats": {"per_element": {}}}
        )
        self.assertEqual(result["data"], [])

    def test_anomaly_stats_missing_key(self) -> None:
        result = sidecar._chart_anomaly_stats({})
        self.assertEqual(result["data"], [])

    # ------------------------------------------------------------------
    # _chart_summary
    # ------------------------------------------------------------------

    def test_summary_with_data(self) -> None:
        data = {
            "StatsSummary": {
                "count": 100,
                "mean": 5000.0,
                "std_dev": 1000.0,
                "variance": 1_000_000.0,
                "min": 2000.0,
                "max": 8000.0,
                "p50": 4800.0,
                "p90": 6500.0,
                "p95": 7200.0,
                "p99": 7800.0,
            }
        }
        result = sidecar._chart_summary(data)
        self.assertEqual(result["summary"]["count"], 100)
        self.assertEqual(result["summary"]["mean"], 5000.0)
        self.assertEqual(result["summary"]["p50"], 4800.0)
        traces = result["chart"]["data"]
        self.assertEqual(len(traces), 1)
        self.assertEqual(traces[0]["type"], "bar")

    def test_summary_empty(self) -> None:
        result = sidecar._chart_summary({})
        self.assertEqual(result["summary"], {})

    def test_summary_empty_stats(self) -> None:
        result = sidecar._chart_summary({"StatsSummary": {}})
        # Empty inner dict triggers the early-return path (empty summary)
        self.assertEqual(result["summary"], {})
        self.assertEqual(result["chart"]["layout"]["title"]["text"], "No data")


class HandleCommandTest(unittest.TestCase):
    """Test command dispatch with a mocked zsim_rs."""

    def setUp(self) -> None:
        self.zsim_patcher = mock.patch.object(
            sidecar, "zsim_rs", mock.MagicMock(), create=True
        )
        self.mock_zsim = self.zsim_patcher.start()

    def tearDown(self) -> None:
        self.zsim_patcher.stop()

    def test_summary_command(self) -> None:
        self.mock_zsim.summary.return_value = json.dumps(
            {"StatsSummary": {"count": 50, "mean": 3000.0}}
        )
        resp = sidecar.handle_command(
            {"summary": {"parquet_path": "/tmp/test.parquet"}}
        )
        self.assertEqual(resp["type"], "summary")
        self.assertIn("data", resp)
        self.mock_zsim.summary.assert_called_once_with("/tmp/test.parquet")

    def test_dps_curve_command(self) -> None:
        self.mock_zsim.dps_curve.return_value = json.dumps(
            {"DPS": [{"tick_start": 0, "dps": 100.0}]}
        )
        resp = sidecar.handle_command(
            {"dps_curve": {"parquet_path": "/tmp/test.parquet", "window_ticks": 60}}
        )
        self.assertEqual(resp["type"], "chart")
        self.mock_zsim.dps_curve.assert_called_once_with("/tmp/test.parquet", 60)

    def test_dps_curve_default_window(self) -> None:
        self.mock_zsim.dps_curve.return_value = json.dumps({"DPS": []})
        resp = sidecar.handle_command(
            {"dps_curve": {"parquet_path": "/tmp/test.parquet"}}
        )
        self.assertEqual(resp["type"], "chart")
        self.mock_zsim.dps_curve.assert_called_once_with("/tmp/test.parquet", 60)

    def test_damage_breakdown_command(self) -> None:
        self.mock_zsim.damage_breakdown.return_value = json.dumps(
            {"DamageBreakdown": {"skill": 1000.0}}
        )
        resp = sidecar.handle_command(
            {"damage_breakdown": {"parquet_path": "/tmp/test.parquet"}}
        )
        self.assertEqual(resp["type"], "chart")
        self.mock_zsim.damage_breakdown.assert_called_once_with(
            "/tmp/test.parquet"
        )

    def test_anomaly_timeline_command(self) -> None:
        self.mock_zsim.aggregate.return_value = json.dumps(
            {"AnomalyStats": {"per_element": {}}}
        )
        resp = sidecar.handle_command(
            {"anomaly_timeline": {"parquet_path": "/tmp/test.parquet"}}
        )
        self.assertEqual(resp["type"], "chart")
        self.mock_zsim.aggregate.assert_called_once_with(
            "/tmp/test.parquet", '"AnomalyStats"'
        )

    def test_shutdown_command(self) -> None:
        resp = sidecar.handle_command({"shutdown": True})
        self.assertEqual(resp["type"], "shutdown")

    def test_unknown_command(self) -> None:
        resp = sidecar.handle_command({"unknown": {}})
        self.assertEqual(resp["type"], "error")
        self.assertIn("Unknown command", resp["message"])

    def test_exception_handling(self) -> None:
        self.mock_zsim.summary.side_effect = RuntimeError("boom")
        resp = sidecar.handle_command(
            {"summary": {"parquet_path": "/tmp/x.parquet"}}
        )
        self.assertEqual(resp["type"], "error")
        self.assertIn("RuntimeError: boom", resp["message"])
        self.assertIn("traceback", resp)


class MainLoopTest(unittest.TestCase):
    """Test the stdin/stdout event loop."""

    def setUp(self) -> None:
        self.zsim_patcher = mock.patch.object(
            sidecar, "zsim_rs", mock.MagicMock(), create=True
        )
        self.zsim_patcher.start()

    def tearDown(self) -> None:
        self.zsim_patcher.stop()

    def test_main_emits_ready_then_processes_commands(self) -> None:
        commands = [{"shutdown": True}]
        input_data = "\n".join(json.dumps(c) for c in commands) + "\n"

        with (
            mock.patch.object(sidecar.sys, "stdin") as mock_stdin,
            mock.patch.object(sidecar.sys, "stdout") as mock_stdout,
        ):
            mock_stdin.__iter__.return_value = input_data.splitlines(keepends=True)
            mock_stdout.write = mock.MagicMock()
            mock_stdout.flush = mock.MagicMock()

            sidecar.main()

        write_calls = mock_stdout.write.call_args_list
        self.assertGreaterEqual(len(write_calls), 2)
        ready_line = json.loads(write_calls[0][0][0])
        self.assertEqual(ready_line["type"], "ready")
        shutdown_line = json.loads(write_calls[1][0][0])
        self.assertEqual(shutdown_line["type"], "shutdown")

    def test_main_handles_invalid_json(self) -> None:
        commands = ["not valid json\n", '{"shutdown": true}\n']

        with (
            mock.patch.object(sidecar.sys, "stdin") as mock_stdin,
            mock.patch.object(sidecar.sys, "stdout") as mock_stdout,
        ):
            mock_stdin.__iter__.return_value = commands
            mock_stdout.write = mock.MagicMock()
            mock_stdout.flush = mock.MagicMock()

            sidecar.main()

        write_calls = mock_stdout.write.call_args_list
        self.assertGreaterEqual(len(write_calls), 3)
        error_line = json.loads(write_calls[1][0][0])
        self.assertEqual(error_line["type"], "error")
        self.assertIn("Invalid JSON", error_line["message"])

    def test_main_skips_empty_lines(self) -> None:
        commands = ["\n", '{"shutdown": true}\n']

        with (
            mock.patch.object(sidecar.sys, "stdin") as mock_stdin,
            mock.patch.object(sidecar.sys, "stdout") as mock_stdout,
        ):
            mock_stdin.__iter__.return_value = commands
            mock_stdout.write = mock.MagicMock()
            mock_stdout.flush = mock.MagicMock()

            sidecar.main()

        write_calls = mock_stdout.write.call_args_list
        self.assertEqual(len(write_calls), 2)


if __name__ == "__main__":
    unittest.main()
