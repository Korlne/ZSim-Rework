# combat/team_manager.py
"""
队伍与派系管理模块
主要功能：管理1-3名角色加1个邦布的编队，协调前后台切换、换人冷却、
喧响值和连携技点数的共享资源。邦布与角色使用相同API但无能量机制。
"""
import logging
from typing import List, Optional, Dict

from core_control.decorators import emit_on_error
from core_control.exceptions import ActionExecutionError
from entities.character import Character
from entities.enums import CharacterState

logger = logging.getLogger("zsim.Combat.TeamManager")


class TeamManager:
    """
    队伍管理器类
    主要功能：作为编队数据中枢，管理角色列表、当前前台角色、邦布引用及共享战斗资源。
    邦布系统与角色使用完全相同的 API（SkillAction 模型），但无能量机制。
    """

    SWITCH_COOLDOWN_TICKS: int = 60
    MAX_DECIBEL: float = 3000.0

    def __init__(self, characters: List[Character], bangboo: Optional[Character] = None):
        if len(characters) < 1 or len(characters) > 3:
            raise ValueError("队伍人数必须为1-3人")

        self.characters: List[Character] = characters
        self.bangboo: Optional[Character] = bangboo
        self.active_index: int = 0
        self.decibel_value: float = 0.0
        self.chain_points: int = 0
        self.switch_cooldowns: Dict[int, int] = {}

        for i, char in enumerate(self.characters):
            if i == 0:
                char.state = CharacterState.ACTIVE
            else:
                char.state = CharacterState.STANDBY

    def get_active_character(self) -> Character:
        """获取当前前台角色"""
        return self.characters[self.active_index]

    def get_standby_characters(self) -> List[Character]:
        """获取所有后台待命角色列表（不含邦布）"""
        return [char for i, char in enumerate(self.characters) if i != self.active_index]

    @emit_on_error
    def switch_character(self, target_index: int) -> bool:
        """
        切换前台角色到目标索引。
        检查换人冷却，若冷却中则抛出 ActionExecutionError。
        """
        if target_index < 0 or target_index >= len(self.characters):
            raise ActionExecutionError(f"切换目标索引 {target_index} 超出队伍范围")

        if target_index == self.active_index:
            raise ActionExecutionError(
                f"目标角色 {self.characters[target_index].char_id} 已是前台角色"
            )

        remaining = self.switch_cooldowns.get(target_index, 0)
        if remaining > 0:
            raise ActionExecutionError(
                f"角色 {self.characters[target_index].char_id} 换人冷却中，剩余 {remaining} 帧"
            )

        old_char = self.characters[self.active_index]
        new_char = self.characters[target_index]

        old_char.change_state(CharacterState.STANDBY)
        new_char.change_state(CharacterState.ACTIVE)

        self.switch_cooldowns[target_index] = self.SWITCH_COOLDOWN_TICKS
        self.active_index = target_index

        logger.debug(f"角色切换: {old_char.char_id} -> {new_char.char_id}")
        return True

    def tick_cooldowns(self):
        """每帧推进时递减所有换人冷却计时"""
        for idx in list(self.switch_cooldowns.keys()):
            if self.switch_cooldowns[idx] > 0:
                self.switch_cooldowns[idx] -= 1

    def add_decibel(self, amount: float):
        """增加喧响值，上限3000"""
        self.decibel_value = min(self.decibel_value + amount, self.MAX_DECIBEL)

    def consume_decibel(self, amount: float) -> bool:
        """消耗喧响值，不足时返回 False"""
        if self.decibel_value < amount:
            return False
        self.decibel_value -= amount
        return True

    def add_chain_points(self, amount: int = 1):
        """增加连携技点数"""
        self.chain_points += amount

    def consume_chain_points(self, amount: int = 1) -> bool:
        """消耗连携技点数，不足时返回 False"""
        if self.chain_points < amount:
            return False
        self.chain_points -= amount
        return True

    def on_quick_assist(self, sender, **kwargs):
        """
        快速支援事件监听器：按顺位逻辑自动切换角色。
        优先切换到列表中第一个非冷却中的待命角色。
        """
        for i in range(len(self.characters)):
            if i == self.active_index:
                continue
            remaining = self.switch_cooldowns.get(i, 0)
            if remaining <= 0:
                logger.info(f"快速支援触发：自动切换到角色 {self.characters[i].char_id}")
                self.switch_character(i)
                return

        logger.warning("快速支援触发但无可用待命角色（所有角色均在冷却中）")

    def register_listeners(self):
        """
        注册事件监听器到 EventBus。
        连接快速支援与相关战斗信号。
        """
        from core_control.dispatcher import on_coordinated_action

        on_coordinated_action.connect(self.on_quick_assist, weak=False)

    def unregister_listeners(self):
        """
        注销事件监听器，防止重复订阅。
        """
        from core_control.dispatcher import on_coordinated_action

        on_coordinated_action.disconnect(self.on_quick_assist)

    def get_all_members(self) -> List[Character]:
        """获取队伍所有成员（角色 + 邦布），邦布置于列表末尾"""
        members: List[Character] = list(self.characters)
        if self.bangboo is not None:
            members.append(self.bangboo)
        return members


# TeamManager 定义完毕后，重建 GameState 以解析前向引用
from core_control.game_state import GameState
GameState.model_rebuild()
