"""
全链路集成模拟脚本
主要功能：从外部 JSON 数据加载角色/敌人/技能/APL，组装 GameState、EventBus、APLManager、
ResourceValidator、StructuredLogger、RNGManager 等全部模块，运行完整战斗模拟并生成分析报表。
支持固定 Seed 确保 100% 复现。
"""
import logging
import os
from typing import Optional, Tuple

from core_control.logger import setup_logging
from core_control.game_state import GameState, SimMode
from core_control.event_bus import EventBus
from core_control.apl_manager import APLManager
from core_control.dispatcher import (
    on_tick, on_action_start, on_damage_dealt, on_damage_applied,
)

from entities.character import Character
from entities.enemy import EnemyState
from entities.models import BaseStats
from entities.enums import FactionTag, SpecialtyTag, ElementTag, CharacterState, EnemyType

from combat.team_manager import TeamManager
from combat.resource_validator import ResourceValidator
from combat.skill_data import SkillData, SkillType, TriggerType

from calculation.rng_manager import RNGManager
from calculation.calculator import RegularMul, StunMul

from data_io.data_loader import DataLoader
from data_io.structured_logger import StructuredLogger

from analysis.data_analyzer import DataAnalyzer

logger = logging.getLogger("zsim.Main")

# 元素 → 敌人抗性字段名映射
ELEMENT_RES_MAP = {
    "Physical": "res_physical",
    "Fire": "res_fire",
    "Ice": "res_ice",
    "Electric": "res_electric",
    "Ether": "res_ether",
    "Wind": "res_wind",
    "Auric Ink": "res_ether",
    "Frost": "res_ice",
    "Honed Edge": "res_physical",
}


def _get_enemy_resistance(enemy: EnemyState, element: str) -> float:
    """根据元素类型获取对应的敌人抗性值"""
    field = ELEMENT_RES_MAP.get(element)
    if field is None:
        return 0.0
    return getattr(enemy, field, 0.0)


def run_simulation(
    seed: int = 42,
    max_ticks: int = 600,
    data_root: str = "data",
    apl_path: str = "data/apl/sample_apl.json",
) -> Tuple[str, str]:
    """
    运行完整战斗模拟。
    返回 (jsonl_log_path, json_report_path)。
    """
    setup_logging()
    logger.info("=== 初始化 ZSim 模拟引擎 (Seed=%d) ===", seed)

    # 1. 加载外部数据
    loader = DataLoader(data_root)
    char_data = loader.load_characters()[0]
    enemy_data = loader.load_enemies()[0]
    skill_dict = loader.load_skill_data()

    # 2. 构建实体
    base_stats = BaseStats(**char_data.base_stats)
    character = Character(
        char_id=char_data.char_id,
        faction=FactionTag(char_data.faction),
        specialty=SpecialtyTag(char_data.specialty),
        element=ElementTag(char_data.element),
        base_stats=base_stats,
        action_dict=set(char_data.action_dict),
    )
    character.state = CharacterState.ACTIVE

    enemy = EnemyState.model_validate({
        "enemy_id": enemy_data.enemy_id,
        "enemy_type": EnemyType(enemy_data.enemy_type),
        "atk": enemy_data.atk,
        "hp": enemy_data.hp,
        "def": enemy_data.def_val,
        "daze_max": enemy_data.daze_max,
        "res_physical": enemy_data.res_physical,
        "res_fire": enemy_data.res_fire,
        "res_ice": enemy_data.res_ice,
        "res_electric": enemy_data.res_electric,
        "res_ether": enemy_data.res_ether,
        "res_wind": enemy_data.res_wind,
    })

    # 3. 组建队伍
    team = TeamManager([character])
    team.register_listeners()

    # 4. 初始化全局状态
    state = GameState(
        max_ticks=max_ticks,
        sim_mode=SimMode.FULL,
        enemy=enemy,
        team=team,
    )

    # 5. 注册技能数据到校验器（JSON → Pydantic 严格模式类型转换）
    validator = ResourceValidator()
    for action_id, data_dict in skill_dict.items():
        converted = dict(data_dict)
        converted["action_type"] = SkillType(converted["action_type"])
        converted["trigger_type"] = TriggerType(converted["trigger_type"])
        converted["damage_multipliers"] = [tuple(p) for p in converted["damage_multipliers"]]
        converted["invincible_frames"] = [tuple(p) for p in converted["invincible_frames"]]
        sd = SkillData(**converted)
        validator.register_skill_data(sd)

    # 6. 初始化 RNG
    rng = RNGManager(seed)

    # 7. 加载 APL 排轴
    apl = APLManager(validator)
    apl.load_from_json(apl_path)
    logger.info("APL 加载完成: %d 个动作", len(apl.action_queue))

    # 8. 结构化日志记录器
    slog = StructuredLogger()
    slog.register_event_handlers(state)

    # 9. 事件总线
    bus = EventBus(state)

    # 10. 动作执行追踪
    action_complete: dict = {"remaining_ticks": 0}

    def handle_tick(sender, tick):
        validator.tick_cooldowns()
        team.tick_cooldowns()

        if action_complete["remaining_ticks"] <= 0:
            try:
                apl.process_next_action(state)
            except Exception:
                state.is_running = False
        else:
            action_complete["remaining_ticks"] -= 1

    on_tick.connect(handle_tick)

    # 11. 伤害结算管线
    def handle_action_start(sender, action_id):
        sd = validator.skill_data_map.get(action_id)
        if sd is None:
            return

        char = team.get_active_character()
        element_val = char.element.value

        # 扣除资源
        if sd.energy_cost > 0:
            char.energy = max(0.0, char.energy - sd.energy_cost)
        if sd.hp_cost > 0:
            char.current_hp = max(0.0, char.current_hp - sd.hp_cost)
        if sd.special_resource_cost > 0:
            char.special_resource = max(0.0, char.special_resource - sd.special_resource_cost)

        # 设置冷却
        if sd.cooldown_ticks > 0:
            validator.set_cooldown(char.char_id, action_id, sd.cooldown_ticks)

        # 计算伤害
        for mult_frame, mult in sd.damage_multipliers:
            hit_count = max(len(sd.hit_frames), 1)
            base_dmg = RegularMul.calc_base_damage(
                mult, hit_count, 0.0, char.base_stats.atk, 0.0, 0.0,
            )
            dmg_bonus = RegularMul.calc_dmg_bonus_zone()
            crit = rng.roll_crit(char.base_stats.crit_rate)
            crit_effective = char.base_stats.crit_dmg if crit else 0.0
            crit_expect = 1.0 + crit_effective

            def_zone = RegularMul.calc_def_zone(
                enemy.def_val, char.base_stats.pen_ratio, char.base_stats.pen_fixed,
            )
            res = _get_enemy_resistance(enemy, element_val)
            res_zone = RegularMul.calc_res_zone(res)
            vul_zone = RegularMul.calc_vulnerable_zone()
            stun_zone = RegularMul.calc_stun_zone(enemy.is_stunned)
            special_zone = RegularMul.calc_special_zone()
            piercing_zone = RegularMul.calc_piercing_zone()

            raw = base_dmg
            final = raw * dmg_bonus * crit_expect * def_zone * res_zone * vul_zone * stun_zone * special_zone * piercing_zone

            damage_payload = {
                "source_char": char.char_id,
                "target_enemy": enemy.enemy_id,
                "action_id": action_id,
                "raw_damage": final,
                "final_damage": final,
                "crit": crit,
                "element": element_val,
            }
            on_damage_dealt.send(sender, **damage_payload)
            on_damage_applied.send(sender, **damage_payload)

        # 计算失衡值
        if sd.daze_multiplier > 0:
            daze = StunMul.calc_daze(
                char.base_stats.impact, sd.daze_multiplier,
                hit_count=max(len(sd.hit_frames), 1),
            )
            enemy.add_daze(daze)

        action_complete["remaining_ticks"] = sd.interruptible_frame

    on_action_start.connect(handle_action_start)

    # 12. 启动模拟
    logger.info("=== 开始执行时间轴 (max_ticks=%d) ===", max_ticks)
    bus.run_simulation()
    logger.info("=== 模拟结束 (tick=%d) ===", state.current_tick)

    # 13. 清理
    on_tick.disconnect(handle_tick)
    on_action_start.disconnect(handle_action_start)
    slog.disconnect_event_handlers()
    slog.close()
    team.unregister_listeners()

    # 14. 数据分析
    logs_dir = os.path.join(os.getcwd(), "logs")
    reports_dir = os.path.join(os.getcwd(), "reports")
    os.makedirs(reports_dir, exist_ok=True)

    analyzer = DataAnalyzer(str(slog.log_path))
    report_path = os.path.join(reports_dir, "simulation_report.json")
    analyzer.export_json_report(report_path)
    logger.info("JSON 报表已导出: %s", report_path)

    chart_paths = analyzer.export_png_charts(reports_dir)
    if chart_paths:
        logger.info("PNG 图表已导出: %s 张", len(chart_paths))

    return str(slog.log_path), report_path


def main():
    """命令行入口"""
    log_path, report_path = run_simulation()
    print(f"\n模拟完成!")
    print(f"  日志: {log_path}")
    print(f"  报表: {report_path}")


if __name__ == "__main__":
    main()
