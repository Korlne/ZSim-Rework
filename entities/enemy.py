# entities/enemy.py
"""
敌人实体模块
主要功能：提供用于测试的沙袋/木人模型，承载基础防御与抗性数据，管理失衡系统与异常槽。
"""
import logging
from typing import Dict
from pydantic import BaseModel, Field, ConfigDict
from .enums import EnemyType, ElementTag

logger = logging.getLogger("zsim.Entities.Enemy")

class EnemyState(BaseModel):
    """
    EnemyState 模型
    主要功能：管理敌人当前的基础防御与抗性，计算并记录失衡值，控制失衡易伤期及连携技判定逻辑。
    """
    model_config = ConfigDict(strict=True)
    
    enemy_id: str
    enemy_type: EnemyType = EnemyType.NORMAL
    base_res: float = Field(default=0.0, description="全属性基础抗性")
    
    # 失衡系统
    daze_current: float = Field(default=0.0, description="当前失衡值")
    daze_max: float = Field(default=100.0, description="失衡阈值")
    is_stunned: bool = Field(default=False, description="是否处于失衡易伤期")
    
    # 异常积蓄槽
    anomaly_buildup: Dict[ElementTag, float] = Field(default_factory=dict)

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