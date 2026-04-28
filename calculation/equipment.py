# calculation/equipment.py
"""
装备系统模块
主要功能：管理音擎（武器）与驱动盘数据，将驱动盘套装效果转换为标准 Buff 挂载到角色。
作为依赖注入层，在角色工厂初始化时注入装备。数据通过 DataLoader 从 JSON 加载。
"""
import logging
from typing import List, Optional
from pydantic import BaseModel, Field, ConfigDict

from .buff_manager import BuffData

logger = logging.getLogger("zsim.Calculation.Equipment")


class WEngine(BaseModel):
    """
    音擎（武器）模型
    承载武器的基础攻击力、副属性与被动效果。
    """
    model_config = ConfigDict(strict=True)

    engine_id: str = Field(description="音擎唯一标识符")
    name: str = Field(default="", description="音擎名称")
    base_atk: float = Field(default=0.0, description="基础攻击力")
    sub_stat_type: str = Field(default="", description="副属性类型")
    sub_stat_value: float = Field(default=0.0, description="副属性值")
    passive_effect: Optional[BuffData] = Field(
        default=None, description="被动效果（转换为 BuffData）"
    )


class DriveDisc(BaseModel):
    """
    驱动盘模型（单个槽位）
    1-6号位各持有一个主属性和最多4个副属性。
    """
    model_config = ConfigDict(strict=True)

    disc_id: str = Field(description="驱动盘唯一标识符")
    slot: int = Field(ge=1, le=6, description="槽位编号（1-6）")
    set_id: str = Field(default="", description="所属套装ID")
    main_stat_type: str = Field(default="", description="主属性类型")
    main_stat_value: float = Field(default=0.0, description="主属性值")
    sub_stats: List['DiscSubStat'] = Field(default_factory=list, description="副属性列表")


class DiscSubStat(BaseModel):
    """驱动盘副属性条目"""
    model_config = ConfigDict(strict=True)
    stat_type: str = Field(description="副属性类型")
    stat_value: float = Field(default=0.0, description="副属性值")


class DriveDiscSet(BaseModel):
    """
    驱动盘套装效果模型
    2件套和4件套分别提供独立的 BuffData 效果。
    """
    model_config = ConfigDict(strict=True)

    set_id: str = Field(description="套装唯一标识符")
    name: str = Field(default="", description="套装名称")
    two_piece_bonus: Optional[BuffData] = Field(default=None, description="2件套效果")
    four_piece_bonus: Optional[BuffData] = Field(default=None, description="4件套效果")


class EquipmentManager:
    """
    EquipmentManager 装备管理器
    主要功能：在角色初始化时注入装备数据，自动解析套装效果并生成对应的 BuffData，
    通过 BuffManager 挂载到角色身上。
    """

    def __init__(self):
        self.wengines: dict = {}       # {char_id: WEngine}
        self.discs: dict = {}          # {char_id: List[DriveDisc]}
        self.set_registry: dict = {}   # {set_id: DriveDiscSet}

    def equip_wengine(self, char_id: str, wengine: WEngine):
        """为角色装备音擎"""
        self.wengines[char_id] = wengine
        logger.debug(f"装备音擎: {char_id} ← {wengine.engine_id}")

    def equip_discs(self, char_id: str, discs: List[DriveDisc]):
        """为角色装备驱动盘（替换现有）"""
        self.discs[char_id] = discs
        logger.debug(f"装备驱动盘: {char_id} ← {len(discs)} 件")

    def register_set(self, disc_set: DriveDiscSet):
        """注册一个套装效果"""
        self.set_registry[disc_set.set_id] = disc_set

    def get_active_buffs(self, char_id: str, buff_manager) -> List[BuffData]:
        """
        根据角色的当前装备计算应激活的 BuffData 列表。
        包含：音擎被动 + 驱动盘主属性 + 驱动盘副属性 + 套装效果（2件套/4件套）。
        """
        buffs: List[BuffData] = []

        # 音擎被动
        wengine = self.wengines.get(char_id)
        if wengine and wengine.passive_effect:
            buffs.append(wengine.passive_effect)

        # 驱动盘统计
        discs = self.discs.get(char_id, [])
        set_counts: dict = {}
        for disc in discs:
            # 副属性转换为独立 BUFF
            for sub in disc.sub_stats:
                sub_buff = BuffData(
                    buff_id=f"disc_{disc.disc_id}_sub_{sub.stat_type}",
                    name=f"disc sub {sub.stat_type}",
                    source_tag="drive_disc",
                    duration=-1,  # 永久
                    modifiers={sub.stat_type: sub.stat_value},
                )
                buffs.append(sub_buff)

            # 统计套装数量
            if disc.set_id:
                set_counts[disc.set_id] = set_counts.get(disc.set_id, 0) + 1

        # 套装效果判定
        for set_id, count in set_counts.items():
            disc_set = self.set_registry.get(set_id)
            if disc_set is None:
                continue
            if count >= 4 and disc_set.four_piece_bonus:
                buffs.append(disc_set.four_piece_bonus)
            elif count >= 2 and disc_set.two_piece_bonus:
                buffs.append(disc_set.two_piece_bonus)

        return buffs
