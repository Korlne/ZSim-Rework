# io/structured_logger.py
"""
结构化日志记录器模块
主要功能：监听 EventBus 所有关键事件，生成结构化 JSON Lines 格式日志，
供下游数据分析使用。每条日志为单行 JSON，包含 tick、event_type、timestamp 与 payload。
"""
import json
import logging
import time
from pathlib import Path
from typing import TextIO, TYPE_CHECKING

if TYPE_CHECKING:
    from core_control.game_state import GameState

logger = logging.getLogger("zsim.IO.StructuredLogger")


class StructuredLogger:
    """
    StructuredLogger 类
    主要功能：作为全局事件监听器，将战斗模拟中的关键事件序列化为 JSON Lines 格式。
    输出到 logs/simulation_YYYYMMDD_HHMMSS.jsonl。
    """

    def __init__(self, output_dir: str = "logs"):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

        timestamp = time.strftime("%Y%m%d_%H%M%S")
        self.log_path = self.output_dir / f"simulation_{timestamp}.jsonl"
        self._file: TextIO = open(self.log_path, 'w', encoding='utf-8')

        # 事件计数
        self.event_count: int = 0

    def log_event(self, event_type: str, tick: int, payload: dict):
        """记录一条事件"""
        entry = {
            "tick": tick,
            "event_type": event_type,
            "timestamp": time.time(),
            "payload": payload,
        }
        self._file.write(json.dumps(entry, ensure_ascii=False) + "\n")
        self.event_count += 1

    def close(self):
        """关闭日志文件"""
        if self._file and not self._file.closed:
            self._file.close()
            logger.info(f"结构化日志已保存: {self.log_path} ({self.event_count} 条)")

    def register_event_handlers(self, game_state: 'GameState'):
        """注册所有事件监听器到 EventBus"""
        from core_control.dispatcher import (
            on_tick, on_action_start, on_damage_dealt, on_damage_applied,
            on_buff_changed, on_anomaly_triggered, on_disorder_triggered,
            on_chain_attack, on_dodge, on_parry, on_coordinated_action,
            on_combat_end,
        )

        def make_handler(event_name: str):
            def handler(sender, **kwargs):
                self.log_event(event_name, game_state.current_tick, kwargs)
            return handler

        self._handlers = []
        signals = [
            (on_tick, 'on_tick'),
            (on_action_start, 'on_action_start'),
            (on_damage_dealt, 'on_damage_dealt'),
            (on_damage_applied, 'on_damage_applied'),
            (on_buff_changed, 'on_buff_changed'),
            (on_anomaly_triggered, 'on_anomaly_triggered'),
            (on_disorder_triggered, 'on_disorder_triggered'),
            (on_chain_attack, 'on_chain_attack'),
            (on_dodge, 'on_dodge'),
            (on_parry, 'on_parry'),
            (on_coordinated_action, 'on_coordinated_action'),
            (on_combat_end, 'on_combat_end'),
        ]
        for signal, name in signals:
            h = make_handler(name)
            self._handlers.append((signal, h))
            signal.connect(h, weak=False)

        logger.info(f"事件监听器已注册，输出: {self.log_path}")

    def disconnect_event_handlers(self):
        """注销所有事件监听器"""
        for signal, handler in getattr(self, '_handlers', []):
            signal.disconnect(handler)
        self._handlers = []
