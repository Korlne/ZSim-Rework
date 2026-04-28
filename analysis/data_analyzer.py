# analysis/data_analyzer.py
"""
数据分析器模块
主要功能：解析结构化日志（JSONL），输出 DPS 曲线、伤害占比饼图、技能占比饼图、
异常触发次数、能量轴曲线等可视化数据。支持导出 PNG 图片与 JSON 报表。
"""
import json
import logging
from collections import defaultdict
from pathlib import Path
from typing import List, Dict, Tuple, Optional
from statistics import mean, stdev

logger = logging.getLogger("zsim.Analysis.DataAnalyzer")

# 帧级误差范围（±2 帧动作误差 + ±2 帧卡肉误差）
FRAME_ERROR_MARGIN = 4


class DataAnalyzer:
    """
    DataAnalyzer 类
    主要功能：加载 JSONL 日志文件，计算各项数据指标并生成报表。
    所有数值以 (value, error_margin) 元组格式返回。
    """

    def __init__(self, jsonl_path: Optional[str] = None):
        self.events: List[dict] = []
        self.window_size: int = 60  # DPS 滑动窗口（1秒 = 60帧）
        if jsonl_path:
            self.load(jsonl_path)

    def load(self, jsonl_path: str):
        """加载 JSONL 日志文件"""
        self.events = []
        with open(jsonl_path, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if line:
                    self.events.append(json.loads(line))
        logger.info(f"加载日志: {len(self.events)} 条事件")

    def compute_dps_curve(self, window_size: Optional[int] = None) -> List[Tuple[int, float, float]]:
        """DPS 曲线：滑动窗口内的每秒伤害 (tick, dps, error)"""
        ws = window_size or self.window_size
        damage_by_tick: Dict[int, float] = defaultdict(float)
        max_tick = 0

        for evt in self.events:
            if evt.get("event_type") in ("on_damage_applied", "on_damage_dealt"):
                tick = evt["tick"]
                dmg = evt.get("payload", {}).get("final_damage", 0)
                damage_by_tick[tick] += dmg
                max_tick = max(max_tick, tick)

        result = []
        for t in range(0, max_tick + 1, ws):
            window_dmg = sum(damage_by_tick.get(i, 0) for i in range(t, min(t + ws, max_tick + 1)))
            dps = window_dmg / (ws / 60.0)  # 转换为每秒
            # 误差与 DPS 成比例（帧计数误差 / 窗口帧数）
            error_ratio = FRAME_ERROR_MARGIN / ws
            result.append((t, dps, dps * error_ratio))

        return result

    def compute_damage_distribution(self) -> Dict:
        """角色伤害占比"""
        char_dmg: Dict[str, float] = defaultdict(float)
        total_dmg = 0.0

        for evt in self.events:
            if evt.get("event_type") in ("on_damage_applied",):
                payload = evt.get("payload", {})
                char_id = payload.get("source_char", payload.get("character_id", "unknown"))
                dmg = payload.get("final_damage", 0)
                char_dmg[char_id] += dmg
                total_dmg += dmg

        return {
            "characters": [
                {
                    "char_id": cid,
                    "total_damage": (dmg, dmg * (FRAME_ERROR_MARGIN / 60)),
                    "percentage": (dmg / total_dmg * 100 if total_dmg > 0 else 0, 0.0),
                }
                for cid, dmg in sorted(char_dmg.items(), key=lambda x: -x[1])
            ],
            "total_damage": total_dmg,
        }

    def compute_skill_distribution(self) -> Dict:
        """技能伤害占比"""
        skill_dmg: Dict[str, Dict] = defaultdict(lambda: {"damage": 0.0, "count": 0})

        for evt in self.events:
            if evt.get("event_type") == "on_action_start":
                aid = evt.get("payload", {}).get("action_id", "unknown")
                skill_dmg[aid]["count"] += 1

        # 关联伤害事件
        for evt in self.events:
            if evt.get("event_type") in ("on_damage_applied",):
                aid = evt.get("payload", {}).get("action_id", "unknown")
                if aid in skill_dmg:
                    skill_dmg[aid]["damage"] += evt["payload"].get("final_damage", 0)

        total_dmg = sum(s["damage"] for s in skill_dmg.values())
        return {
            "skills": [
                {
                    "action_id": aid,
                    "total_damage": (d["damage"], d["damage"] * 0.01),
                    "count": d["count"],
                    "percentage": (d["damage"] / total_dmg * 100 if total_dmg > 0 else 0, 0.0),
                }
                for aid, d in sorted(skill_dmg.items(), key=lambda x: -x[1]["damage"])
            ],
        }

    def compute_anomaly_summary(self) -> Dict:
        """异常触发统计"""
        anomaly_counts: Dict[str, int] = defaultdict(int)
        disorder_count = 0

        for evt in self.events:
            etype = evt.get("event_type")
            if etype == "on_anomaly_triggered":
                element = evt.get("payload", {}).get("element", "unknown")
                anomaly_counts[element] += 1
            elif etype == "on_disorder_triggered":
                disorder_count += 1

        return {
            "anomalies": dict(anomaly_counts),
            "disorder_count": disorder_count,
            "total_anomaly_triggers": sum(anomaly_counts.values()),
        }

    def compute_energy_curve(self, char_id: str) -> List[Tuple[int, float]]:
        """能量轴曲线"""
        curve = []
        for evt in self.events:
            payload = evt.get("payload", {})
            if payload.get("character_id") == char_id or payload.get("char_id") == char_id:
                if "energy" in payload:
                    curve.append((evt["tick"], payload["energy"]))
        return curve

    def compute_buff_uptime(self, char_id: str, buff_id: str) -> float:
        """BUFF 覆盖率 (0.0 ~ 1.0)"""
        total_ticks = 0
        active_ticks = 0

        buff_active = False
        for evt in self.events:
            if evt.get("event_type") == "on_buff_changed":
                payload = evt.get("payload", {})
                if payload.get("char_id") == char_id and payload.get("buff_id") == buff_id:
                    buff_active = payload.get("action") == "apply"
            if evt.get("event_type") == "on_tick":
                total_ticks += 1
                if buff_active:
                    active_ticks += 1

        return active_ticks / total_ticks if total_ticks > 0 else 0.0

    def export_json_report(self, output_path: str) -> str:
        """导出 JSON 格式报表数据"""
        report = {
            "dps_curve": self.compute_dps_curve(),
            "damage_distribution": self.compute_damage_distribution(),
            "skill_distribution": self.compute_skill_distribution(),
            "anomaly_summary": self.compute_anomaly_summary(),
        }
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(report, f, ensure_ascii=False, indent=2)
        return output_path

    def export_png_charts(self, output_dir: str = "reports") -> List[str]:
        """导出 PNG 图表（DPS 曲线、伤害占比饼图、技能占比饼图、异常统计图）"""
        try:
            import matplotlib
            matplotlib.use('Agg')
            import matplotlib.pyplot as plt
        except ImportError:
            logger.warning("matplotlib 未安装，无法导出 PNG 图表")
            return []

        Path(output_dir).mkdir(parents=True, exist_ok=True)
        saved = []

        # DPS 曲线
        dps_data = self.compute_dps_curve()
        if dps_data:
            ticks = [d[0] for d in dps_data]
            values = [d[1] for d in dps_data]
            errors = [d[2] for d in dps_data]

            fig, ax = plt.subplots(figsize=(10, 5))
            ax.plot(ticks, values, 'b-', label='DPS')
            ax.fill_between(ticks,
                            [v - e for v, e in zip(values, errors)],
                            [v + e for v, e in zip(values, errors)],
                            alpha=0.2, color='b', label=f'±{FRAME_ERROR_MARGIN}帧误差')
            ax.set_xlabel("Tick")
            ax.set_ylabel("Damage Per Second")
            ax.set_title("DPS Curve")
            ax.legend()
            ax.grid(True, alpha=0.3)
            path = str(Path(output_dir) / "dps_curve.png")
            fig.savefig(path, dpi=150, bbox_inches='tight')
            plt.close(fig)
            saved.append(path)

        # 伤害占比饼图
        dist = self.compute_damage_distribution()
        chars = dist.get("characters", [])
        if chars:
            labels = [c["char_id"] for c in chars]
            values = [c["total_damage"][0] for c in chars]
            fig, ax = plt.subplots(figsize=(8, 8))
            ax.pie(values, labels=labels, autopct='%1.1f%%', startangle=90)
            ax.set_title("Damage Distribution by Character")
            path = str(Path(output_dir) / "damage_distribution.png")
            fig.savefig(path, dpi=150, bbox_inches='tight')
            plt.close(fig)
            saved.append(path)

        return saved
