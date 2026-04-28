# entities/enemy.py
"""
敌人实体模块
主要功能：提供用于测试的沙袋/木人模型，管理敌人属性面板（含抗性、失衡、虚弱等）、失衡系统与异常槽。
"""
import logging
from typing import Dict
from pydantic import BaseModel, Field, field_validator, ConfigDict
from .enums import EnemyType, ElementTag

logger = logging.getLogger("zsim.Entities.Enemy")

class EnemyState(BaseModel):
    """
    EnemyState 模型
    主要功能：承载敌人的基础属性与抗性面板，管理失衡值、虚弱期及连携技判定逻辑。
    """
    # 开启严格模式，禁止隐式类型转换
    model_config = ConfigDict(strict=True)
    
    # 基础身份信息
    enemy_id: str
    enemy_type: EnemyType = EnemyType.NORMAL
    
    # 核心战斗属性
    atk: float = Field(default=0.0, description="敌人攻击力")
    hp: float = Field(default=0.0, description="敌人生命值")
    def_val: float = Field(default=0.0, alias="def", description="敌人防御力")
    daze_max: float = Field(default=100.0, description="失衡阈值 (Daze)")
    
    # 战斗修正项
    crit_rate: float = Field(default=0.0, description="敌人暴击率") # 默认为0，一般不暴击，除非真的想被暴击。但是暂不考虑敌人对角色造成伤害。
    crit_dmg: float = Field(default=0.0, description="敌人暴击伤害")
    weaken_duration: float = Field(default=0.0, description="失衡/虚弱持续时间")
    dmg_multiplier: float = Field(default=1.0, description="伤害倍率修正")

    # 抗性系统 
    res_physical: float = Field(default=0.0, description="物理抗性")
    res_fire: float = Field(default=0.0, description="火属性抗性")
    res_ice: float = Field(default=0.0, description="冰属性抗性")
    res_electric: float = Field(default=0.0, description="雷属性抗性")
    res_ether: float = Field(default=0.0, description="以太抗性")
    res_wind: float = Field(default=0.0, description="风属性抗性")
    
    # 实时状态属性
    daze_current: float = Field(default=0.0, description="当前累计失衡值")
    is_stunned: bool = Field(default=False, description="是否处于失衡易伤期")
    
    # 异常积蓄槽
    anomaly_buildup: Dict[ElementTag, float] = Field(default_factory=dict)

    @field_validator('atk', 'hp', 'def_val', 'daze_max', 'daze_current', 
                     'res_physical', 'res_fire', 'res_ice', 'res_electric', 
                     'res_ether', 'res_wind', mode='before')
    @classmethod
    def check_non_negative(cls, v):
        """全局非负数值校验器，拦截负数属性赋值"""
        if isinstance(v, (int, float)) and v < 0:
            raise ValueError("敌人属性数值不能为负数")
        return v

    def get_chain_attack_limit(self) -> int:
        """根据敌人类型枚举返回对应的允许连携技触发次数"""
        mapping = {
            EnemyType.NORMAL: 1,
            EnemyType.ELITE: 2,
            EnemyType.BOSS: 3
        }
        return mapping.get(self.enemy_type, 0)

    def add_daze(self, value: float):
        """增加失衡值并判定是否触发失衡易伤状态"""
        if not self.is_stunned:
            self.daze_current += value
            # 当失衡值达到或超出阈值时，切换状态
            if self.daze_current >= self.daze_max:
                self.is_stunned = True
                logger.info(f"Enemy {self.enemy_id} is STUNNED! Chain attacks allowed: {self.get_chain_attack_limit()}")