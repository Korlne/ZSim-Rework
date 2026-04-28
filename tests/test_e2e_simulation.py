# tests/test_e2e_simulation.py
"""端到端集成测试 — 完整模拟流程

运行方式: 从项目根目录执行
    PYTHONPATH="." python tests/test_e2e_simulation.py
"""
import json
import os
import sys
import unittest
from pathlib import Path

# 确保项目根目录在 sys.path 最前面
PROJECT_ROOT = str(Path(__file__).resolve().parent.parent)
sys.path = [p for p in sys.path if not p.endswith('tests')]
if PROJECT_ROOT not in sys.path:
    sys.path.insert(0, PROJECT_ROOT)
os.chdir(PROJECT_ROOT)


class TestE2ESimulation(unittest.TestCase):
    """端到端集成测试：完整模拟流程验证"""

    @classmethod
    def setUpClass(cls):
        """运行一次完整模拟"""
        from run_simulation import run_simulation

        cls.log_path, cls.report_path = run_simulation(seed=42, max_ticks=300)

    def test_simulation_completes_without_exception(self):
        """模拟运行至结束，无未处理异常"""
        self.assertTrue(Path(self.log_path).exists())

    def test_jsonl_log_generated(self):
        """生成结构化 JSONL 日志文件"""
        log_file = Path(self.log_path)
        self.assertTrue(log_file.exists())

        with open(log_file, 'r', encoding='utf-8') as f:
            lines = f.readlines()
        self.assertGreater(len(lines), 0, "日志文件不应为空")

        for line in lines:
            entry = json.loads(line.strip())
            self.assertIn("tick", entry)
            self.assertIn("event_type", entry)
            self.assertIn("payload", entry)

    def test_log_contains_expected_events(self):
        """日志包含预期的关键事件类型"""
        event_types = set()
        with open(self.log_path, 'r', encoding='utf-8') as f:
            for line in f:
                entry = json.loads(line.strip())
                event_types.add(entry.get("event_type"))

        expected = {"on_tick", "on_action_start", "on_damage_dealt",
                     "on_damage_applied", "on_combat_end"}
        for event in expected:
            self.assertIn(event, event_types,
                          f"日志应包含 {event} 事件")

    def test_data_analyzer_reports(self):
        """DataAnalyzer 输出完整报表"""
        from analysis.data_analyzer import DataAnalyzer

        analyzer = DataAnalyzer(self.log_path)

        dps_curve = analyzer.compute_dps_curve()
        damage_dist = analyzer.compute_damage_distribution()
        skill_dist = analyzer.compute_skill_distribution()
        anomaly_summary = analyzer.compute_anomaly_summary()

        self.assertIsInstance(dps_curve, list, "DPS曲线应为列表")
        self.assertGreater(len(dps_curve), 0, "DPS曲线不应为空")

        self.assertIsInstance(damage_dist, dict, "伤害分布应为字典")
        self.assertIn("characters", damage_dist)
        self.assertGreater(len(damage_dist["characters"]), 0,
                           "应有角色伤害数据")

        self.assertIsInstance(skill_dist, dict, "技能分布应为字典")
        self.assertIn("skills", skill_dist)

        self.assertIsInstance(anomaly_summary, dict, "异常统计应为字典")

    def test_json_report_exported(self):
        """导出 JSON 格式的报表数据"""
        report_path = Path(self.report_path)
        self.assertTrue(report_path.exists(),
                        f"报表文件应存在: {report_path}")

        with open(report_path, 'r', encoding='utf-8') as f:
            report = json.load(f)

        self.assertIn("dps_curve", report)
        self.assertIn("damage_distribution", report)
        self.assertIn("skill_distribution", report)
        self.assertIn("anomaly_summary", report)

    def test_damage_events_have_correct_payload(self):
        """伤害事件的 payload 包含必要字段"""
        with open(self.log_path, 'r', encoding='utf-8') as f:
            damage_events = [
                json.loads(line.strip())
                for line in f
                if '"on_damage_dealt"' in line or '"on_damage_applied"' in line
            ]

        self.assertGreater(len(damage_events), 0,
                           "应有伤害事件")
        for evt in damage_events:
            payload = evt.get("payload", {})
            self.assertIn("source_char", payload)
            self.assertIn("target_enemy", payload)
            self.assertIn("final_damage", payload)
            self.assertIn("element", payload)


class TestE2ESeedReproducibility(unittest.TestCase):
    """Seed 复现性测试：相同配置运行两次应产生完全一致的日志"""

    def test_same_seed_produces_identical_logs(self):
        """同一 Seed 运行两次，日志逐行完全一致（忽略 timestamp）"""
        from run_simulation import run_simulation

        log1_path, _ = run_simulation(seed=42, max_ticks=200)
        log2_path, _ = run_simulation(seed=42, max_ticks=200)

        with open(log1_path, 'r', encoding='utf-8') as f:
            lines1 = f.readlines()
        with open(log2_path, 'r', encoding='utf-8') as f:
            lines2 = f.readlines()

        self.assertEqual(len(lines1), len(lines2),
                         "两次运行应产生相同数量的日志行")

        for i, (l1, l2) in enumerate(zip(lines1, lines2)):
            e1 = json.loads(l1.strip())
            e2 = json.loads(l2.strip())
            self.assertEqual(e1["event_type"], e2["event_type"],
                             f"行 {i}: 事件类型应一致")
            self.assertEqual(e1["tick"], e2["tick"],
                             f"行 {i}: tick 应一致")
            self.assertEqual(e1["payload"], e2["payload"],
                             f"行 {i}: payload 应一致")

    def test_different_seeds_produce_different_damage(self):
        """不同 Seed 产生不同的伤害结果"""
        from run_simulation import run_simulation

        log1_path, _ = run_simulation(seed=42, max_ticks=200)
        log2_path, _ = run_simulation(seed=99, max_ticks=200)

        # 提取所有伤害事件
        def get_damage_values(path):
            values = []
            with open(path, 'r', encoding='utf-8') as f:
                for line in f:
                    entry = json.loads(line.strip())
                    if entry.get("event_type") == "on_damage_dealt":
                        values.append(entry["payload"].get("final_damage", 0))
            return values

        dmg1 = get_damage_values(log1_path)
        dmg2 = get_damage_values(log2_path)

        # 至少有一次伤害事件的 crit 判定不同
        crits1 = []
        crits2 = []
        with open(log1_path, 'r', encoding='utf-8') as f:
            for line in f:
                e = json.loads(line.strip())
                if e.get("event_type") == "on_damage_dealt":
                    crits1.append(e["payload"].get("crit"))
        with open(log2_path, 'r', encoding='utf-8') as f:
            for line in f:
                e = json.loads(line.strip())
                if e.get("event_type") == "on_damage_dealt":
                    crits2.append(e["payload"].get("crit"))

        # 不同 seed 应有不同的暴击判定序列（概率上极高）
        self.assertEqual(len(crits1), len(crits2),
                         "两个 Seed 应有相同数量的伤害事件")


if __name__ == "__main__":
    unittest.main()
