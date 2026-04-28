# calculation/calculator.py
"""
数值计算器模块
主要功能：实现完整的6大结算模块——常规伤害、异常积蓄、失衡值、异常伤害、紊乱、极性紊乱。
所有结算类为纯函数/静态方法，不持有可变状态。参考 Docs/伤害与战斗数值计算公式提取.md。
"""
import logging
from typing import Dict, Optional

logger = logging.getLogger("zsim.Calculation.Calculator")

# 属性异常伤害倍率常量
ANOMALY_RATIOS = {
    "Physical": 7.13,
    "Fire": 0.5,
    "Ice": 5.0,
    "Frost": 5.0,
    "Electric": 1.25,
    "Ether": 0.625,
    "Auric Ink": 0.625,
    "Wind": 0.5,
}

# 攻击方等级基数（玩家侧固定值）
ATTACKER_LEVEL_BASE = 794.0


class RegularMul:
    """
    常规直伤结算类
    公式：基础伤害区 × 增伤区 × 暴击期望 × 防御区 × 抗性区 × 减易伤区 × 失衡易伤区 × 特殊倍率区 × 贯穿伤害区
    """

    @staticmethod
    def calc_base_damage(
        skill_multiplier: float, hit_count: int,
        extra_dmg_multiplier: float,
        panel_attr: float, percent_increase: float, flat_increase: float,
    ) -> float:
        """基础伤害区 = ((技能倍率/攻击次数) + 额外倍率) × 面板 × (1 + 百分比) + 固定"""
        return ((skill_multiplier / max(hit_count, 1)) + extra_dmg_multiplier) * \
               (panel_attr * (1.0 + percent_increase) + flat_increase)

    @staticmethod
    def calc_dmg_bonus_zone(
        element_bonus: float = 0.0, skill_type_bonus: float = 0.0,
        tag_bonus: float = 0.0, universal_bonus: float = 0.0,
    ) -> float:
        """增伤区 = 1 + 属性增伤 + 技能类型增伤 + 标签增伤 + 全类型增伤"""
        return 1.0 + element_bonus + skill_type_bonus + tag_bonus + universal_bonus

    @staticmethod
    def calc_crit_expect(crit_rate: float, crit_dmg: float) -> float:
        """暴击期望 = 1 + min(1, 暴击率) × 暴击伤害"""
        return 1.0 + min(1.0, crit_rate) * crit_dmg

    @staticmethod
    def calc_def_zone(def_val: float, pen_ratio: float, pen_fixed: float,
                      is_piercing: bool = False) -> float:
        """防御区 = 等级基数 / (有效防御 + 等级基数)；贯穿伤害该乘区固定为1"""
        if is_piercing:
            return 1.0
        effective_def = max(0.0, def_val * (1.0 - pen_ratio) - pen_fixed)
        return ATTACKER_LEVEL_BASE / (effective_def + ATTACKER_LEVEL_BASE)

    @staticmethod
    def calc_res_zone(
        monster_res: float, res_reduce: float = 0.0,
        res_penetration: float = 0.0, universal_res_reduce: float = 0.0,
        universal_penetration: float = 0.0, snapshot_penetration: float = 0.0,
    ) -> float:
        """抗性区 = 1 - (怪物抗性 - 抗性降低 - 抗性穿透) + 全伤害抗性降低 + 全抗性穿透 + 快照穿透"""
        return 1.0 - (monster_res - res_reduce - res_penetration) + \
               universal_res_reduce + universal_penetration + snapshot_penetration

    @staticmethod
    def calc_vulnerable_zone(element_vul: float = 0.0, universal_vul: float = 0.0) -> float:
        """减易伤区 = 1 + 属性易伤 + 全属性易伤"""
        return 1.0 + element_vul + universal_vul

    @staticmethod
    def calc_stun_zone(is_stunned: bool, monster_stun_vul: float = 0.0,
                       stun_vul_inc: float = 0.0, constant_stun_vul: float = 0.0) -> float:
        """失衡易伤区：失衡时 = 1 + 怪物失衡易伤 + 增幅 + 全时段；非失衡 = 1 + 全时段"""
        if is_stunned:
            return 1.0 + monster_stun_vul + stun_vul_inc + constant_stun_vul
        return 1.0 + constant_stun_vul

    @staticmethod
    def calc_special_zone(special_mult: float = 0.0) -> float:
        """特殊倍率区 = 1 + 特殊乘区加成"""
        return 1.0 + special_mult

    @staticmethod
    def calc_piercing_zone(piercing_bonus: float = 0.0) -> float:
        """贯穿伤害区 = 1 + 贯穿伤害增加"""
        return 1.0 + piercing_bonus


class AnomalyMul:
    """
    异常积蓄值结算类
    公式：基础积蓄值 × (异常掌控/100) × (1 + 积蓄效率提升) × 积蓄抗性乘区 × 元素比例 / 攻击次数
    """

    @staticmethod
    def calc_buildup(
        base_buildup: float, mastery: float,
        buildup_efficiency: float = 0.0,
        type_efficiency: float = 0.0,
        buildup_res_reduce: float = 0.0,
        monster_buildup_res: float = 0.0,
        element_ratio: float = 1.0,
        hit_count: int = 1,
    ) -> float:
        """计算异常积蓄值"""
        effective_mastery = mastery / 100.0
        efficiency_zone = 1.0 + buildup_efficiency + type_efficiency
        res_zone = 1.0 - buildup_res_reduce - monster_buildup_res
        return base_buildup * effective_mastery * efficiency_zone * res_zone * \
               element_ratio / max(hit_count, 1)


class StunMul:
    """
    失衡值结算类
    公式：冲击力 × (技能失衡倍率/攻击次数) × 失衡抗性乘区 × 失衡值增幅 × 受到失衡值提升
    """

    @staticmethod
    def calc_daze(
        impact: float, skill_daze_multiplier: float,
        hit_count: int = 1, daze_res_reduce: float = 0.0,
        extra_daze_res_reduce: float = 0.0,
        monster_daze_res: float = 0.0,
        type_daze_inc: float = 0.0,
        universal_daze_inc: float = 0.0,
        tag_daze_inc: float = 0.0,
        daze_taken_inc: float = 0.0,
        extra_daze_taken_inc: float = 0.0,
    ) -> float:
        """计算失衡值累积"""
        res_zone = 1.0 - daze_res_reduce - extra_daze_res_reduce - monster_daze_res
        daze_inc_zone = 1.0 + type_daze_inc + universal_daze_inc + tag_daze_inc
        taken_zone = 1.0 + daze_taken_inc + extra_daze_taken_inc
        return impact * (skill_daze_multiplier / max(hit_count, 1)) * \
               res_zone * daze_inc_zone * taken_zone


class CalAnomaly:
    """
    异常伤害结算类
    公式：异常基础伤害区 × 增伤区 × 异常精通区 × 等级区系数 × 异常增伤区 ×
          激活型暴击区 × 防御区 × 抗性区 × 减易伤区 × 失衡易伤区 × 特殊倍率区 × 缩放比例因子
    """

    @staticmethod
    def calc_anomaly_base(atk: float, element: str) -> float:
        """异常基础伤害区 = 攻击力 × 属性异常伤害倍率"""
        ratio = ANOMALY_RATIOS.get(element, 1.0)
        return atk * ratio

    @staticmethod
    def calc_mastery_zone(proficiency: float, percent_inc: float = 0.0,
                          flat_inc: float = 0.0) -> float:
        """异常精通区 = (面板异常精通 × (1 + 百分比) + 固定值) / 100"""
        return (proficiency * (1.0 + percent_inc) + flat_inc) / 100.0

    @staticmethod
    def calc_anomaly_bonus_zone(element_bonus: float = 0.0,
                                 universal_bonus: float = 0.0) -> float:
        """异常增伤区 = 1 + 属性异常额外增伤 + 全属性异常额外增伤"""
        return 1.0 + element_bonus + universal_bonus

    @staticmethod
    def calc_activated_crit_zone(crit_rate_inc: float = 0.0,
                                   crit_dmg_inc: float = 0.0) -> float:
        """激活型异常暴击区 = 1 + 暴击率增加 × 暴击伤害增加"""
        if crit_rate_inc <= 0 or crit_dmg_inc <= 0:
            return 1.0
        return 1.0 + crit_rate_inc * crit_dmg_inc


class CalDisorder:
    """
    紊乱伤害结算类
    基于异常伤害框架，重置基础伤害区和异常增伤区。
    紊乱基础伤害区 = (原异常快照基础伤害区 / 属性异常伤害倍率) × 紊乱倍率
    """

    @staticmethod
    def calc_disorder_base(
        original_anomaly_base: float, element: str,
        disorder_multiplier: float,
    ) -> float:
        """紊乱基础伤害区"""
        ratio = ANOMALY_RATIOS.get(element, 1.0)
        return (original_anomaly_base / ratio) * disorder_multiplier

    @staticmethod
    def calc_disorder_multiplier(
        remaining_time_bonus: float = 0.0,
        element_const: float = 0.0,
        element_disorder_inc: float = 0.0,
        universal_disorder_inc: float = 0.0,
    ) -> float:
        """紊乱倍率 = 剩余时间倍率 + 属性紊乱基础常数 + 属性紊乱增幅 + 全局紊乱增幅"""
        return remaining_time_bonus + element_const + element_disorder_inc + universal_disorder_inc

    @staticmethod
    def calc_disorder_bonus_zone(disorder_dmg_inc: float = 0.0) -> float:
        """紊乱异常增伤区 = 1 + 紊乱额外伤害增幅"""
        return 1.0 + disorder_dmg_inc


class CalPolarityDisorder:
    """
    极性紊乱伤害结算类
    继承紊乱伤害框架，二次修改基础伤害区。
    极性紊乱基础伤害区 = (原紊乱基础伤害区 × 极性紊乱比例系数) + (异常精通 × 附加比例系数)
    """

    @staticmethod
    def calc_polarity_base(
        original_disorder_base: float,
        polarity_ratio: float,
        proficiency: float,
        additional_ratio: float,
    ) -> float:
        """极性紊乱基础伤害区"""
        return (original_disorder_base * polarity_ratio) + (proficiency * additional_ratio)
