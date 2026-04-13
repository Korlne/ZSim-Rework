# entities/models.py
"""
实体基础属性模型模块
主要功能：基于 Pydantic 提供严格类型检查的 12 项基础属性承载模型，实现数据层面的防御性编程。
"""
from pydantic import BaseModel, Field, field_validator, ConfigDict

class BaseStats(BaseModel):
    """
    角色12项基础属性模型
    主要功能：承载角色的静态基础数值面板，分离“基础属性”与“实时属性”，并对数值合法性进行强制校验。
    """
    # 开启严格模式，禁止隐式的类型转换
    model_config = ConfigDict(strict=True)
    
    # 核心战斗属性
    hp: float = Field(default=0.0, description="基础生命值")
    atk: float = Field(default=0.0, description="基础攻击力")
    def_val: float = Field(default=0.0, alias="def", description="基础防御力")
    impact: float = Field(default=0.0, description="基础冲击力")
    
    # 异常属性
    anomaly_proficiency: float = Field(default=0.0, description="基础异常精通")
    anomaly_mastery: float = Field(default=0.0, description="基础异常掌控")
    
    # 暴击与穿透
    crit_rate: float = Field(default=0.0, description="基础暴击率")
    crit_dmg: float = Field(default=0.0, description="基础额外暴击伤害")
    pen_ratio: float = Field(default=0.0, description="基础穿透率")
    pen_fixed: float = Field(default=0.0, description="基础穿透值")
    
    # 能量系统
    energy_regen: float = Field(default=0.0, description="基础能量自动回复")
    energy_gen_rate: float = Field(default=0.0, description="基础能量获取效率")

    @field_validator('*', mode='before')
    @classmethod
    def check_non_negative(cls, v):
        """
        全局非负数值校验器
        对上述定义的12项属性进行遍历检查，拦截任何小于0的赋值行为。
        """
        # 验证传入值是否为数值类型且小于0
        if isinstance(v, (int, float)) and v < 0:
            raise ValueError("基础属性数值不能为负数")
        return v