# entities/character.py
"""
角色实体模型模块
主要功能：定义角色实体，承载基础属性、实时修饰器(Buff)与动作词典。
结合核心层的 @emit_on_error 装饰器，实现状态机与动作合法性的安全验证。
"""
import logging
from typing import Set, Dict
from core_control.decorators import emit_on_error
from core_control.exceptions import ActionExecutionError
from .enums import FactionTag, ElementTag, SpecialtyTag, CharacterState
from .models import BaseStats

logger = logging.getLogger("zsim.Entities.Character")

class Character:
    """
    角色实体类
    主要功能：作为单体角色的数据承载与工厂，管理角色独立的状态机、命座/潜能及面板计算接口。
    """
    def __init__(self, char_id: str, faction: FactionTag, specialty: SpecialtyTag, 
                 element: ElementTag, base_stats: BaseStats, action_dict: Set[str]):
        # 初始化核心标签与静态属性
        self.char_id = char_id
        self.faction = faction
        self.specialty = specialty
        self.element = element
        self.base_stats = base_stats
        
        # 动作词典，包含该角色允许执行的所有合法动作ID
        self.action_dict = action_dict
        
        # 初始化战斗状态与附加属性
        self.state = CharacterState.STANDBY
        self.constellations = {i: False for i in range(1, 7)} # 1-6命座/潜能开关
        self.realtime_modifiers: Dict[str, float] = {} # 实时Buff修饰器容器

    @emit_on_error
    def change_state(self, new_state: CharacterState):
        """
        变更角色驻场状态。
        受 @emit_on_error 切面装饰器保护，任何异常都会被广播。
        """
        logger.debug(f"Character {self.char_id} state changing: {self.state.value} -> {new_state.value}")
        self.state = new_state

    @emit_on_error
    def validate_action(self, action_id: str):
        """
        校验动作是否合法。
        检查传入的 action_id 是否存在于当前角色的动作词典中。如果不在，则抛出 ActionExecutionError。
        """
        if action_id not in self.action_dict:
            raise ActionExecutionError(f"动作 {action_id} 不在角色 {self.char_id} 的词典内")
        return True
        
    def get_realtime_stat(self, stat_name: str) -> float:
        """
        获取受Buff影响后的最终属性面板。
        将基础属性与 realtime_modifiers 容器中的对应增益进行计算并返回。
        """
        base_val = getattr(self.base_stats, stat_name, 0.0)
        modifier = self.realtime_modifiers.get(stat_name, 0.0)
        return base_val + modifier