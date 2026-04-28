# combat/resource_validator.py
"""
资源与条件校验器模块
主要功能：完整实现 IResourceValidator 接口，校验能量、特殊资源、连携技条件、
技能冷却等资源约束。拒绝时携带详细报错上下文。
"""
import logging
from typing import Dict, Optional, TYPE_CHECKING

from core_control.interfaces import IResourceValidator
from core_control.exceptions import ResourceValidationError
from entities.character import Character
from .skill_data import SkillData, SkillType

if TYPE_CHECKING:
    from core_control.game_state import GameState

logger = logging.getLogger("zsim.Combat.ResourceValidator")


class ResourceValidator(IResourceValidator):
    """
    资源校验器类
    主要功能：作为动作释放前的统一校验入口，检查角色能量、特殊资源、生命值、
    连携技释放条件及技能冷却。若拒绝放行则携带详细上下文抛出 ResourceValidationError。
    """

    def __init__(self, skill_data_map: Optional[Dict[str, SkillData]] = None):
        self.skill_data_map: Dict[str, SkillData] = skill_data_map or {}
        # 技能冷却追踪: {char_id: {action_id: remaining_cooldown_ticks}}
        self.cooldowns: Dict[str, Dict[str, int]] = {}
        self._last_successful_action: str = ""

    def can_execute(self, action_id: str, state: "GameState") -> bool:
        """
        校验给定动作在当前状态下是否允许执行。
        校验项：队伍状态、动作词典、能量、特殊资源、生命值、连携技条件、技能冷却。
        全部通过返回 True，任一不满足抛出 ResourceValidationError。
        """
        team = state.team
        if team is None:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="team",
                current_value=0,
                required_value=1,
                last_successful_action=self._last_successful_action,
            )

        character = team.get_active_character()
        skill_data = self.skill_data_map.get(action_id)

        self._validate_action_dictionary(character, action_id, skill_data)
        self._validate_cooldown(character.char_id, action_id)
        self._validate_hp_cost(character, skill_data, action_id)
        self._validate_energy(character, skill_data, action_id)
        self._validate_special_resource(character, skill_data, action_id)
        self._validate_prerequisite(skill_data, action_id)
        self._validate_decibel(team, skill_data, action_id)
        self._validate_chain_conditions(team, skill_data, action_id, state)

        self._last_successful_action = action_id
        return True

    def _validate_action_dictionary(
        self, character: Character, action_id: str,
        skill_data: Optional[SkillData] = None,
    ) -> None:
        """校验动作是否在角色的动作词典中。已注册 SkillData 的动作跳过词典检查。"""
        if skill_data is not None:
            return  # 校验器中已注册的技能视为合法，无需检查角色词典
        try:
            character.validate_action(action_id)
        except Exception:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="action_in_dictionary",
                current_value=0,
                required_value=1,
                last_successful_action=self._last_successful_action,
            )

    def _validate_cooldown(self, char_id: str, action_id: str) -> None:
        """校验技能冷却是否就绪"""
        char_cooldowns = self.cooldowns.get(char_id, {})
        remaining = char_cooldowns.get(action_id, 0)
        if remaining > 0:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="cooldown",
                current_value=remaining,
                required_value=0,
                last_successful_action=self._last_successful_action,
            )

    def _validate_energy(
        self,
        character: Character,
        skill_data: Optional[SkillData],
        action_id: str,
    ) -> None:
        """校验角色能量是否充足"""
        if skill_data is None or skill_data.energy_cost <= 0:
            return
        if character.energy < skill_data.energy_cost:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="energy",
                current_value=character.energy,
                required_value=skill_data.energy_cost,
                last_successful_action=self._last_successful_action,
            )

    def _validate_special_resource(
        self,
        character: Character,
        skill_data: Optional[SkillData],
        action_id: str,
    ) -> None:
        """校验特殊资源是否满足"""
        if skill_data is None or skill_data.special_resource_cost <= 0:
            return
        if character.special_resource < skill_data.special_resource_cost:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="special_resource",
                current_value=character.special_resource,
                required_value=skill_data.special_resource_cost,
                last_successful_action=self._last_successful_action,
            )

    def _validate_hp_cost(
        self,
        character: Character,
        skill_data: Optional[SkillData],
        action_id: str,
    ) -> None:
        """校验生命值是否足够支付技能消耗"""
        if skill_data is None or skill_data.hp_cost <= 0:
            return
        if character.current_hp < skill_data.hp_cost:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="hp",
                current_value=character.current_hp,
                required_value=skill_data.hp_cost,
                last_successful_action=self._last_successful_action,
            )

    def _validate_prerequisite(
        self,
        skill_data: Optional[SkillData],
        action_id: str,
    ) -> None:
        """校验前置动作依赖是否满足"""
        if skill_data is None or skill_data.prerequisite_action_id is None:
            return
        if skill_data.prerequisite_action_id != self._last_successful_action:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource=f"prerequisite({skill_data.prerequisite_action_id})",
                current_value=0,
                required_value=1,
                last_successful_action=self._last_successful_action,
            )

    def _validate_decibel(
        self,
        team,
        skill_data: Optional[SkillData],
        action_id: str,
    ) -> None:
        """校验喧响值是否足够释放终结技"""
        if skill_data is None or skill_data.action_type != SkillType.ULTIMATE:
            return
        from combat.team_manager import TeamManager

        if team.decibel_value < TeamManager.MAX_DECIBEL:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="decibel",
                current_value=team.decibel_value,
                required_value=float(TeamManager.MAX_DECIBEL),
                last_successful_action=self._last_successful_action,
            )

    def _validate_chain_conditions(
        self,
        team,
        skill_data: Optional[SkillData],
        action_id: str,
        state: "GameState",
    ) -> None:
        """校验连携技释放条件：敌人是否失衡、剩余连携点数是否足够"""
        if skill_data is None or skill_data.action_type != SkillType.CHAIN:
            return

        enemy = state.enemy
        if enemy is None:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="enemy_target",
                current_value=0,
                required_value=1,
                last_successful_action=self._last_successful_action,
            )

        if not enemy.is_stunned:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="enemy_stunned",
                current_value=0,
                required_value=1,
                last_successful_action=self._last_successful_action,
            )

        if team.chain_points < 1:
            raise ResourceValidationError(
                action_id=action_id,
                missing_resource="chain_points",
                current_value=float(team.chain_points),
                required_value=1,
                last_successful_action=self._last_successful_action,
            )

    def set_cooldown(self, char_id: str, action_id: str, duration: int) -> None:
        """为指定角色的指定技能启动冷却计时"""
        if duration <= 0:
            return
        if char_id not in self.cooldowns:
            self.cooldowns[char_id] = {}
        self.cooldowns[char_id][action_id] = duration
        logger.debug(f"设置冷却: {char_id}.{action_id} = {duration} 帧")

    def tick_cooldowns(self) -> None:
        """每帧推进时递减所有技能冷却计时"""
        for char_cds in self.cooldowns.values():
            for aid in list(char_cds.keys()):
                if char_cds[aid] > 0:
                    char_cds[aid] -= 1

    def register_skill_data(self, skill_data: SkillData) -> None:
        """注册一条技能数据到校验器映射中"""
        self.skill_data_map[skill_data.action_id] = skill_data
