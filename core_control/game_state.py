"""
全局状态快照模块
主要功能：基于 Pydantic 构建强类型的全局数据中心，提供带有严格校验机制的上下文快照。
"""
from typing import List, Optional
from pydantic import BaseModel, Field, field_validator, ConfigDict
from entities.character import Character
from entities.enemy import EnemyState
# from entities.bangboo import Bangboo  # 邦布实体预留

class GameState(BaseModel):
    """
    GameState 全局状态模型
    主要功能：承载当前Tick数、最大Tick数与运行状态，作为各系统间传递的数据对象,并以受控列表的形式管理编队角色与目标敌人。
    所有模拟、建模将基于游戏内锁定60帧的表现，遵循1秒60Tick推进。
    """
    
    # 严格模式与允许任意类型（Arbitrary Types）的配置显式合并
    model_config = ConfigDict(strict=True, arbitrary_types_allowed=True)
    
    # 使用 Field 声明默认值与字段描述
    current_tick: int = Field(default=0, description="当前模拟Tick数")
    max_ticks: int = Field(default=18000, description="最大限制Tick数")
    is_running: bool = Field(default=True, description="模拟器运行状态")
    
    # 实体列表集成
    characters: List[Character] = Field(default_factory=list, description="参战角色列表")
    enemy: Optional[EnemyState] = Field(default=None, description="当前目标敌人状态")
    # bangboo: Optional[Bangboo] = Field(default=None, description="队伍邦布预留")

    @field_validator('current_tick', 'max_ticks')
    @classmethod
    def check_non_negative(cls, value: int) -> int:
        """
        字段校验装饰器：拦截负数Tick赋值
        对 current_tick 和 max_ticks 字段进行检查。
        """
        # 如果传入的数值小于0，主动抛出 ValueError 以阻断实例化过程
        if value < 0:
            raise ValueError("Tick数不能为负数")
        return value