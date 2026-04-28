# combat/__init__.py
"""
战斗动作与判定系统模块
主要功能：提供技能数据模型、动作运行时实例、队伍管理及派生动作等战斗核心功能。
"""
from .skill_data import SkillData, SkillType, TriggerType
from .skill import SkillAction
from .team_manager import TeamManager
