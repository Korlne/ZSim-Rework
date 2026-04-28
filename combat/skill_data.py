# combat/skill_data.py
"""
技能数据模型模块
主要功能：定义动作的帧级详细数据模型，含命中帧、无敌帧、伤害倍率、失衡倍率、
异常倍率、打断窗口、快照标记、蓄力分支、前置动作依赖等全部维度。
"""
from enum import Enum
from typing import Dict, List, Optional, Tuple
from pydantic import BaseModel, Field, ConfigDict


class SkillType(str, Enum):
    """技能类型枚举类"""
    BASIC = "Basic"                   # 普通攻击
    SPECIAL = "Special"               # 特殊技
    EX_SPECIAL = "EXSpecial"          # 强化特殊技
    ULTIMATE = "Ultimate"             # 终结技
    CHAIN = "Chain"                   # 连携技
    QUICK_ASSIST = "QuickAssist"      # 快速支援
    DODGE = "Dodge"                   # 闪避
    DODGE_COUNTER = "DodgeCounter"    # 闪避反击
    PARRY = "Parry"                   # 招架/弹刀
    BANGBOO = "Bangboo"               # 邦布动作


class TriggerType(str, Enum):
    """触发类型枚举类"""
    ACTIVE = "Active"             # 前台主动触发
    REACTIVE = "Reactive"         # 条件响应触发（闪避/弹刀）
    COORDINATED = "Coordinated"   # 后台协同触发


class SkillData(BaseModel):
    """
    技能数据模型
    主要功能：承载单个动作的完整帧级数据定义，包含命中窗口、伤害倍率、无敌帧、
    蓄力分支、快照标记等全部战术维度信息。
    """
    model_config = ConfigDict(strict=True)

    # 基础标识
    action_id: str = Field(description="动作唯一标识符")
    action_type: SkillType = Field(description="技能类型（普攻/特殊技/终结技/连携技等）")
    trigger_type: TriggerType = Field(default=TriggerType.ACTIVE, description="触发方式（主动/响应/协同）")

    # 伤害倍率（按帧映射，List[(帧号, 倍率), ...]）
    damage_multipliers: List[Tuple[int, float]] = Field(default_factory=list, description="伤害倍率列表（帧号→倍率）")

    # 失衡与异常
    daze_multiplier: float = Field(default=0.0, description="失衡倍率")
    anomaly_multiplier: float = Field(default=0.0, description="异常积蓄倍率")

    # 帧级时间窗口
    hit_frames: List[int] = Field(default_factory=list, description="命中帧列表（伤害判定的具体帧号）")
    invincible_frames: List[Tuple[int, int]] = Field(default_factory=list, description="无敌帧窗口列表（起始帧, 结束帧）")
    interruptible_frame: int = Field(default=0, description="可打断帧号（0表示不可打断）")

    # 战术特性
    allows_background_completion: bool = Field(default=False, description="是否允许后台完成（切换角色后继续执行）")
    is_snapshot: bool = Field(default=False, description="是否为快照动作（动作开始时锁定属性面板）")

    # 蓄力与分支
    charge_branches: Dict[int, str] = Field(default_factory=dict, description="蓄力分支映射（蓄力时间→派生动作ID）")

    # 前置条件
    prerequisite_action_id: Optional[str] = Field(default=None, description="前置动作ID（必须完成此前置动作后才可执行）")

    # 消耗
    hp_cost: float = Field(default=0.0, description="生命值消耗（>0表示施放需要消耗HP）")
    energy_cost: float = Field(default=0.0, description="能量消耗（>0表示施放需要消耗能量）")
    cooldown_ticks: int = Field(default=0, description="技能冷却帧数（>0表示施放后进入冷却）")
    special_resource_cost: float = Field(default=0.0, description="特殊资源消耗（>0表示施放需要消耗特殊资源）")
