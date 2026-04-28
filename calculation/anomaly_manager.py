# calculation/anomaly_manager.py
"""
异常与紊乱管理模块
主要功能：监听属性异常积蓄槽满溢触发异常状态（感电/强击/冻结/侵蚀/灼烧/惧风）；
监听异属性覆盖触发紊乱计算，并按规则清空异常槽。
"""
import logging
from typing import Dict, Optional, TYPE_CHECKING
from pydantic import BaseModel, Field, ConfigDict

if TYPE_CHECKING:
    from entities.enemy import EnemyState
    from entities.character import Character

logger = logging.getLogger("zsim.Calculation.AnomalyManager")


class AnomalyState(BaseModel):
    """
    异常状态模型
    记录当前敌人身上的异常状态信息。
    """
    model_config = ConfigDict(strict=True)

    element: str = Field(description="当前异常属性元素")
    remaining_ticks: int = Field(default=0, description="剩余持续帧数")
    source_character_id: str = Field(default="", description="触发异常的角色ID")
    snapshot_base_damage: float = Field(default=0.0, description="异常快照基础伤害区（用于紊乱计算）")


class AnomalyDisorderManager:
    """
    AnomalyDisorderManager 异常紊乱管理器
    主要功能：监听伤害事件，管理异常积蓄槽的累积与触发，
    处理异常状态覆盖触发的紊乱结算。
    """

    # 异常触发阈值（可通过配置调整）
    ANOMALY_THRESHOLD: float = 1000.0
    # 紊乱后异常槽保留比例
    DISORDER_RETENTION_RATIO: float = 0.3

    def __init__(self):
        # {enemy_id: {element: buildup_value}}
        self.buildup_trackers: Dict[str, Dict[str, float]] = {}
        # {enemy_id: Optional[AnomalyState]}
        self.active_anomalies: Dict[str, Optional[AnomalyState]] = {}

    def add_buildup(
        self, enemy: 'EnemyState', element: str, value: float,
        character: 'Character',
    ) -> Optional[str]:
        """
        增加异常积蓄值并判定触发。
        返回：'anomaly' 表示触发新异常，'disorder' 表示触发紊乱，None 表示未触发。
        """
        enemy_id = enemy.enemy_id
        if enemy_id not in self.buildup_trackers:
            self.buildup_trackers[enemy_id] = {}

        current = self.buildup_trackers[enemy_id].get(element, 0.0)
        new_value = current + value

        if new_value >= self.ANOMALY_THRESHOLD:
            # 检查是否已有异属性异常 → 触发紊乱
            existing = self.active_anomalies.get(enemy_id)
            if existing is not None and existing.element != element:
                self._trigger_disorder(enemy, element, character, existing)
                self.buildup_trackers[enemy_id][element] = new_value * self.DISORDER_RETENTION_RATIO
                return 'disorder'

            # 触发新异常
            self._trigger_anomaly(enemy, element, character)
            self.buildup_trackers[enemy_id][element] = 0.0
            return 'anomaly'

        self.buildup_trackers[enemy_id][element] = new_value
        return None

    def _trigger_anomaly(self, enemy: 'EnemyState', element: str,
                         character: 'Character'):
        """触发属性异常状态"""
        state = AnomalyState(
            element=element,
            remaining_ticks=600,  # 10秒 × 60帧
            source_character_id=character.char_id,
        )
        self.active_anomalies[enemy.enemy_id] = state
        logger.info(
            f"异常触发: {element} ← {character.char_id} → {enemy.enemy_id}"
        )

    def _trigger_disorder(
        self, enemy: 'EnemyState', new_element: str,
        character: 'Character', existing: AnomalyState,
    ):
        """触发紊乱——异属性覆盖"""
        logger.info(
            f"紊乱触发: {new_element}({character.char_id}) 覆盖 "
            f"{existing.element}({existing.source_character_id}) → {enemy.enemy_id}"
        )
        self.active_anomalies[enemy.enemy_id] = None

    def on_tick(self, enemy_id: str):
        """每帧调用：递减异常状态剩余时间"""
        state = self.active_anomalies.get(enemy_id)
        if state is not None:
            state.remaining_ticks -= 1
            if state.remaining_ticks <= 0:
                self.active_anomalies[enemy_id] = None
                logger.debug(f"异常状态结束: {state.element} ← {enemy_id}")

    def get_active_anomaly(self, enemy_id: str) -> Optional[AnomalyState]:
        """获取敌人当前活跃的异常状态"""
        return self.active_anomalies.get(enemy_id)
