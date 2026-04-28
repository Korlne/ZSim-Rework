# combat/skill.py
"""
动作运行时实例模块
主要功能：定义技能动作的运行时实例类，持有技能数据引用、执行者角色引用、
当前帧计数及完成状态标记。
"""
import logging
from typing import TYPE_CHECKING

from .skill_data import SkillData

if TYPE_CHECKING:
    from entities.character import Character

logger = logging.getLogger("zsim.Combat.SkillAction")


class SkillAction:
    """
    技能动作运行时实例
    主要功能：在战斗模拟中作为动作执行的基本单位，追踪单个动作的执行进度与状态。
    每次执行动作时实例化，动作完成后销毁（或保留用于快照回溯）。
    """

    def __init__(self, skill_data: SkillData, character: "Character"):
        self.skill_data = skill_data
        self.character = character
        self.current_frame: int = 0
        self.is_completed: bool = False

    def advance_frame(self):
        """
        推进一帧
        增加当前帧计数，不检查完成状态（完成判定由外部调度器负责）。
        """
        self.current_frame += 1

    def mark_completed(self):
        """
        标记动作已完成
        """
        logger.debug(
            f"动作 {self.skill_data.action_id} 已完成，"
            f"执行者={self.character.char_id}，总帧数={self.current_frame}"
        )
        self.is_completed = True

    def is_invincible(self) -> bool:
        """
        判断当前帧是否处于无敌窗口内
        """
        for start, end in self.skill_data.invincible_frames:
            if start <= self.current_frame <= end:
                return True
        return False

    def is_hit_frame(self) -> bool:
        """
        判断当前帧是否为命中判定帧
        """
        return self.current_frame in self.skill_data.hit_frames

    def is_interruptible(self) -> bool:
        """
        判断当前帧是否允许打断
        """
        if self.skill_data.interruptible_frame <= 0:
            return False
        return self.current_frame >= self.skill_data.interruptible_frame
