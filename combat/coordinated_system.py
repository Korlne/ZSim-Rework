# combat/coordinated_system.py
"""
协同与派生动作系统模块
主要功能：管理后台角色的协同技能注册与触发。满足条件时自动实例化并下发 Reactive 动作，
与前台 APL 动作在同一 Tick 内并行独立结算，不阻塞前摇/后摇。
"""
import logging
from typing import List, Dict, Callable, Optional, TYPE_CHECKING

from .skill_data import SkillData, TriggerType

if TYPE_CHECKING:
    from entities.character import Character

logger = logging.getLogger("zsim.Combat.CoordinatedActionSystem")


class CoordinatedListener:
    """
    协同监听器
    封装一个后台角色的协同技能配置：触发条件、冷却与对应的 SkillData。
    """

    def __init__(
        self,
        character: 'Character',
        skill_data: SkillData,
        trigger_event: str,
        condition: Optional[Callable[..., bool]] = None,
        cooldown_ticks: int = 0,
    ):
        self.character = character
        self.skill_data = skill_data
        self.trigger_event = trigger_event  # 触发事件名（on_action_start / on_damage_dealt / on_dodge）
        self.condition = condition or (lambda **kwargs: True)  # 附加条件谓词
        self.cooldown_ticks = cooldown_ticks  # 触发后的冷却帧数
        self.remaining_cooldown: int = 0  # 当前剩余冷却帧数
        self.enabled: bool = True  # 是否激活（可被临时禁用）

    def is_on_cooldown(self) -> bool:
        """检查是否处于冷却中"""
        return self.remaining_cooldown > 0

    def matches(self, event_name: str, **event_kwargs) -> bool:
        """判定当前事件是否触发该协同技能"""
        if not self.enabled:
            return False
        if self.is_on_cooldown():
            return False
        if event_name != self.trigger_event:
            return False
        try:
            return self.condition(**event_kwargs)
        except Exception:
            return False


class CoordinatedActionSystem:
    """
    CoordinatedActionSystem 协同动作系统
    主要功能：维护后台协同 Listener 注册表，监听 EventBus 事件，
    匹配条件时自动生成 Reactive SkillAction 并通过 on_coordinated_action 信号下发，
    加入并行结算队列。派生动作与前台 APL 动作在同一 Tick 内并行结算。
    """

    def __init__(self):
        # 按事件类型分组的监听器注册表
        self.listeners: Dict[str, List[CoordinatedListener]] = {}
        # 当前 Tick 待执行的派生动作队列（每帧开始时清空）
        self.pending_actions: List['SkillAction'] = []

    def register_listener(self, listener: CoordinatedListener):
        """注册一个协同监听器"""
        event = listener.trigger_event
        if event not in self.listeners:
            self.listeners[event] = []
        self.listeners[event].append(listener)
        logger.debug(
            f"注册协同监听器: {listener.character.char_id}.{listener.skill_data.action_id} "
            f"→ {event}"
        )

    def unregister_character(self, character: 'Character'):
        """注销指定角色的全部协同监听器"""
        for event_list in self.listeners.values():
            event_list[:] = [l for l in event_list if l.character.char_id != character.char_id]

    def unregister_listener(self, character: 'Character', action_id: str):
        """注销指定角色的特定协同技能"""
        for event_list in self.listeners.values():
            event_list[:] = [
                l for l in event_list
                if not (l.character.char_id == character.char_id and l.skill_data.action_id == action_id)
            ]

    def on_event(self, event_name: str, **event_kwargs):
        """
        接收 EventBus 事件，匹配监听器并生成派生动作。
        派生动作加入 pending_actions 队列，将在本 Tick 内并行结算。
        触发后通过 on_coordinated_action 信号广播。
        """
        from core_control.dispatcher import on_coordinated_action

        event_listeners = self.listeners.get(event_name, [])
        if not event_listeners:
            return

        for listener in event_listeners:
            if listener.matches(event_name, **event_kwargs):
                action = SkillAction(listener.skill_data, listener.character)
                self.pending_actions.append(action)

                # 设置冷却
                if listener.cooldown_ticks > 0:
                    listener.remaining_cooldown = listener.cooldown_ticks

                # 广播协同动作事件
                on_coordinated_action.send(
                    self,
                    character=listener.character,
                    action_id=listener.skill_data.action_id,
                    skill_action=action,
                    trigger_event=event_name,
                )

                logger.info(
                    f"协同触发: {listener.character.char_id} → "
                    f"{listener.skill_data.action_id} (事件: {event_name})"
                )

    def get_pending_actions(self) -> List['SkillAction']:
        """获取并清空当前 Tick 的待执行派生动作列表（消费式获取）"""
        actions = self.pending_actions.copy()
        self.pending_actions.clear()
        return actions

    def clear_pending(self):
        """清空本帧的派生动作队列（每帧结算完成后调用）"""
        self.pending_actions.clear()

    def has_pending_actions(self) -> bool:
        """检查是否有待处理的协同动作"""
        return len(self.pending_actions) > 0

    def tick_cooldowns(self):
        """每帧推进时递减所有 Listener 的冷却计时"""
        for event_list in self.listeners.values():
            for listener in event_list:
                if listener.remaining_cooldown > 0:
                    listener.remaining_cooldown -= 1

    def register_event_handlers(self):
        """向 EventBus 注册核心事件处理器"""
        from core_control.dispatcher import (
            on_action_start, on_damage_dealt, on_dodge,
        )

        on_action_start.connect(self._handle_action_start, weak=False)
        on_damage_dealt.connect(self._handle_damage_dealt, weak=False)
        on_dodge.connect(self._handle_dodge, weak=False)

    def unregister_event_handlers(self):
        """注销所有 EventBus 事件处理器"""
        from core_control.dispatcher import (
            on_action_start, on_damage_dealt, on_dodge,
        )

        on_action_start.disconnect(self._handle_action_start)
        on_damage_dealt.disconnect(self._handle_damage_dealt)
        on_dodge.disconnect(self._handle_dodge)

    def _handle_action_start(self, sender, **kwargs):
        self.on_event('on_action_start', **kwargs)

    def _handle_damage_dealt(self, sender, **kwargs):
        self.on_event('on_damage_dealt', **kwargs)

    def _handle_dodge(self, sender, **kwargs):
        self.on_event('on_dodge', **kwargs)


# 避免循环导入
from .skill import SkillAction  # noqa: E402
