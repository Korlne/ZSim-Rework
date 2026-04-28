# calculation/rng_manager.py
"""
全局随机数管理模块
主要功能：统一提供暴击判定、概率触发类效果的随机数。
支持设置固定的 Seed，确保同一套 JSON 排轴在同配置下每次跑出的结果完全一致。
"""
import logging
import random
from typing import Optional

logger = logging.getLogger("zsim.Calculation.RNGManager")


class RNGManager:
    """
    RNGManager 随机数管理器
    主要功能：作为全局随机数中心，封装 Python random.Random 实例，
    提供暴击判定、概率判定和范围随机数。固定 Seed 确保模拟 100% 复现。
    """

    def __init__(self, seed: Optional[int] = None):
        self.seed: Optional[int] = seed
        self._rng = random.Random(seed)
        self._call_count: int = 0  # 用于调试的调用计数

    def reset(self, seed: Optional[int] = None):
        """重置随机数生成器状态（带可选新 Seed）"""
        if seed is not None:
            self.seed = seed
        self._rng = random.Random(self.seed)
        self._call_count = 0
        logger.debug(f"RNG 重置: seed={self.seed}")

    def roll_crit(self, crit_rate: float) -> bool:
        """
        暴击判定
        返回 True 表示暴击。
        """
        self._call_count += 1
        if crit_rate >= 1.0:
            return True
        if crit_rate <= 0.0:
            return False
        return self._rng.random() < crit_rate

    def roll_probability(self, probability: float) -> bool:
        """
        通用概率判定
        probability ∈ [0, 1]，返回 True 表示触发。
        """
        self._call_count += 1
        if probability >= 1.0:
            return True
        if probability <= 0.0:
            return False
        return self._rng.random() < probability

    def random_float(self, min_val: float, max_val: float) -> float:
        """返回 [min_val, max_val) 范围内的随机浮点数"""
        self._call_count += 1
        return self._rng.uniform(min_val, max_val)

    def random_int(self, min_val: int, max_val: int) -> int:
        """返回 [min_val, max_val] 范围内的随机整数"""
        self._call_count += 1
        return self._rng.randint(min_val, max_val)

    def get_state(self) -> tuple:
        """获取当前随机数生成器的内部状态（用于快照恢复）"""
        return self._rng.getstate()

    def set_state(self, state: tuple):
        """恢复随机数生成器的内部状态"""
        self._rng.setstate(state)
