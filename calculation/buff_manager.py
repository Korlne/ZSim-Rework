# calculation/buff_manager.py
"""
BUFF 管理器模块
主要功能：动态修饰器系统。监听 OnTick 处理倒计时，支持叠层/刷新/移除逻辑，
修饰角色的实时属性，并携带来源追溯 Tag。
"""
import logging
from enum import Enum
from typing import Dict, List, Optional, TYPE_CHECKING
from pydantic import BaseModel, Field, ConfigDict

if TYPE_CHECKING:
    from entities.character import Character

logger = logging.getLogger("zsim.Calculation.BuffManager")


class BuffRefreshPolicy(str, Enum):
    """BUFF 刷新策略枚举"""
    REPLACE = "replace"   # 替换：重置持续时间和层数
    STACK = "stack"       # 叠层：增加层数，刷新持续时间
    EXTEND = "extend"     # 延长时间：仅刷新持续时间，不改变层数


class BuffData(BaseModel):
    """
    BUFF 数据模型
    定义单个 BUFF 实例的完整数据，包含修饰目标、来源追溯与刷新策略。
    """
    model_config = ConfigDict(strict=True)

    buff_id: str = Field(description="BUFF 唯一标识符")
    name: str = Field(default="", description="BUFF 显示名称")
    source_tag: str = Field(
        default="unknown",
        description="来源Tag（character_skill / wengine / drive_disc / environment）"
    )

    duration: int = Field(default=0, description="剩余持续帧数")
    max_stacks: int = Field(default=1, description="最大叠层数")
    current_stacks: int = Field(default=1, description="当前叠层数")

    # 属性修饰: {stat_name: value}（可为正数增益或负数减益）
    modifiers: Dict[str, float] = Field(default_factory=dict, description="属性修饰映射")
    refresh_policy: BuffRefreshPolicy = Field(
        default=BuffRefreshPolicy.REPLACE,
        description="刷新策略"
    )


class BuffManager:
    """
    BuffManager 类
    主要功能：作为全局 BUFF 调度中心，管理所有角色身上的 BUFF 实例。
    监听 on_tick 事件以递减倒计时，到期自动移除并还原属性。
    """

    def __init__(self):
        # {char_id: {buff_id: BuffData}}
        self.active_buffs: Dict[str, Dict[str, BuffData]] = {}

    def apply_buff(self, character: 'Character', buff: BuffData):
        """将 BUFF 应用到角色身上"""
        char_id = character.char_id
        if char_id not in self.active_buffs:
            self.active_buffs[char_id] = {}

        existing = self.active_buffs[char_id].get(buff.buff_id)
        if existing is not None:
            self._handle_refresh(existing, buff)
        else:
            self.active_buffs[char_id][buff.buff_id] = buff

        self._apply_modifiers(character, buff.modifiers)
        logger.debug(
            f"BUFF 应用: {buff.buff_id} → {char_id} "
            f"(stacks={buff.current_stacks}, duration={buff.duration})"
        )

    def remove_buff(self, character: 'Character', buff_id: str):
        """移除 BUFF 并还原属性"""
        char_id = character.char_id
        char_buffs = self.active_buffs.get(char_id, {})
        buff = char_buffs.pop(buff_id, None)
        if buff is None:
            return

        # 还原属性：减去 BUFF 的修饰值
        reversed_modifiers = {k: -v for k, v in buff.modifiers.items()}
        self._apply_modifiers(character, reversed_modifiers)
        logger.debug(f"BUFF 移除: {buff_id} ← {char_id}")

    def on_tick(self):
        """每帧调用：递减所有 BUFF 的剩余时间，到期自动移除"""
        expired: List[tuple] = []  # (char_id, buff_id)
        for char_id, char_buffs in self.active_buffs.items():
            for buff_id, buff in list(char_buffs.items()):
                buff.duration -= 1
                if buff.duration <= 0:
                    expired.append((char_id, buff_id))

        # 移除已过期的 BUFF（需要 character 引用，这里采用延迟通知）
        for char_id, buff_id in expired:
            char_buffs = self.active_buffs.get(char_id, {})
            buff = char_buffs.pop(buff_id, None)
            if buff:
                logger.debug(f"BUFF 过期: {buff_id} ← {char_id}")

    def get_character_buffs(self, char_id: str) -> Dict[str, BuffData]:
        """获取指定角色的所有活跃 BUFF"""
        return self.active_buffs.get(char_id, {})

    def register_event_handlers(self):
        """向 EventBus 注册事件处理器"""
        from core_control.dispatcher import on_tick, on_buff_changed

        on_tick.connect(self._on_tick_handler, weak=False)

    def _on_tick_handler(self, sender, **kwargs):
        self.on_tick()

    def _handle_refresh(self, existing: BuffData, incoming: BuffData):
        """按刷新策略处理已有 BUFF 的更新"""
        if incoming.refresh_policy == BuffRefreshPolicy.REPLACE:
            existing.current_stacks = incoming.current_stacks
            existing.duration = incoming.duration
            existing.modifiers = incoming.modifiers
        elif incoming.refresh_policy == BuffRefreshPolicy.STACK:
            existing.current_stacks = min(
                existing.max_stacks,
                existing.current_stacks + incoming.current_stacks
            )
            existing.duration = incoming.duration
        elif incoming.refresh_policy == BuffRefreshPolicy.EXTEND:
            existing.duration = incoming.duration

    @staticmethod
    def _apply_modifiers(character: 'Character', modifiers: Dict[str, float]):
        """将修饰值应用到角色的 realtime_modifiers 容器"""
        for stat_name, value in modifiers.items():
            current = character.realtime_modifiers.get(stat_name, 0.0)
            character.realtime_modifiers[stat_name] = current + value
