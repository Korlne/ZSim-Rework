# entities/enums.py
"""
标签系统枚举模块
主要功能：统一定义游戏实体所需的分类标签，使用标准的枚举类替换硬编码字符串，遵循开闭原则。
"""
from enum import Enum

class FactionTag(str, Enum):
    """
    完整阵营枚举类
    主要功能：定义目前系统支持的15个阵营及预留的扩展项。
    """
    GENTLE_HOUSE = "Gentle_House"                # 狡兔屋
    INVESTIGATION_UNIT = "Criminal_Investigation" # 刑侦特勤组
    HSOS6 = "H.S.O.S.6"                          # H.S.O.S.6
    BELOBOG_HEAVY_IND = "Belobog_Heavy_Ind"      # 白祇重工
    VICTORIA_HOUSEKEEPING = "Victoria_Housekeeping" # 维多利亚家政
    SONS_OF_CALYDON = "Sons_of_Calydon"          # 卡吕冬之子
    LYRA = "Lyra"                                # 天琴座
    OBOLUS_SQUAD = "Obolus_Squad"                # 奥波勒斯小队
    DEFENSE_FORCE_SILVER = "Defense_Force_Silver" # 防卫军·白银小队
    MOCKINGBIRD = "Mockingbird"                  # 反舌鸟
    YUN_KUI_MOUNTAIN = "Yun_Kui_Mountain"        # 云岿山
    ODD_EATER_HOUSE = "Odd_Eater_House"          # 怪啖屋
    KRAMPUS_DARK_BRANCH = "Krampus_Dark_Branch"  # 坎卜斯黑枝
    DELUSIONAL_ANGEL = "Delusional_Angel"        # 妄想天使
    CITY_ORDER_DEPT = "City_Order_Dept"          # 都市秩序部
    OTHER = "Other"                              # 其他/未来新增

class SpecialtyTag(str, Enum):
    """
    角色特性枚举类
    主要功能：定义角色在战斗体系中的定位特性。
    """
    ATTACK = "Attack"     # 强攻
    STUN = "Stun"         # 击破
    SUPPORT = "Support"   # 支援
    RUPTURE = "Rupture"   # 命破
    ANOMALY = "Anomaly"   # 异常
    DEFENSE = "Defense"   # 防护

class ElementTag(str, Enum):
    """
    元素属性枚举类
    主要功能：定义角色及异常状态对应的元素伤害类型。
    """
    ICE = "Ice"
    FIRE = "Fire"
    ETHER = "Ether"
    PHYSICAL = "Physical"
    ELECTRIC = "Electric"
    WIND = "Wind"
    AURIC_INK = "Auric Ink"     # 特殊以太-玄墨
    FROST = "Frost"             # 特殊冰-烈霜
    HONED_EDGE = "Honed Edge"   # 特殊物理-凛刃

class CharacterState(str, Enum):
    """
    角色驻场状态枚举类
    主要功能：标识角色当前处于前台、后台或不可交互状态。
    """
    ACTIVE = "Active"       # 前台
    STANDBY = "Standby"     # 后台
    LOCKED = "Locked"       # 不可交互

class EnemyType(str, Enum):
    """
    敌人类型枚举类
    主要功能：区分不同量级的敌人，后续用于限制连携技次数等逻辑。
    """
    NORMAL = "Normal"       # 连携技1次
    ELITE = "Elite"         # 连携技2次
    BOSS = "Boss"           # 连携技3次