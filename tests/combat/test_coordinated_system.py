# tests/combat/test_coordinated_system.py
import unittest

from combat.coordinated_system import CoordinatedActionSystem, CoordinatedListener
from combat.skill_data import SkillData, SkillType, TriggerType
from entities.character import Character
from entities.enums import (
    CharacterState,
    ElementTag,
    FactionTag,
    SpecialtyTag,
)
from entities.models import BaseStats


def _make_character(char_id: str, action_dict=None) -> Character:
    if action_dict is None:
        action_dict = {"Attack_Normal", "Coordinated_Shot"}
    stats = BaseStats(hp=1000.0, atk=500.0)
    return Character(
        char_id=char_id,
        faction=FactionTag.GENTLE_HOUSE,
        specialty=SpecialtyTag.ATTACK,
        element=ElementTag.PHYSICAL,
        base_stats=stats,
        action_dict=action_dict,
    )


def _make_skill_data(
    action_id: str = "Coordinated_Shot",
    action_type: SkillType = SkillType.BASIC,
    trigger_type: TriggerType = TriggerType.COORDINATED,
) -> SkillData:
    return SkillData(
        action_id=action_id,
        action_type=action_type,
        trigger_type=trigger_type,
        damage_multipliers=[(1, 1.5)],
        hit_frames=[1],
    )


class TestCoordinatedListener(unittest.TestCase):
    """测试 CoordinatedListener 基本属性与匹配逻辑"""

    def setUp(self):
        self.char = _make_character("TestChar")
        self.skill = _make_skill_data("Coordinated_Shot")
        self.listener = CoordinatedListener(
            character=self.char,
            skill_data=self.skill,
            trigger_event="on_action_start",
        )

    def test_listener_creation(self):
        self.assertEqual(self.listener.character.char_id, "TestChar")
        self.assertEqual(self.listener.skill_data.action_id, "Coordinated_Shot")
        self.assertEqual(self.listener.trigger_event, "on_action_start")
        self.assertTrue(self.listener.enabled)
        self.assertEqual(self.listener.remaining_cooldown, 0)

    def test_matches_correct_event(self):
        self.assertTrue(self.listener.matches("on_action_start"))

    def test_matches_wrong_event(self):
        self.assertFalse(self.listener.matches("on_damage_dealt"))

    def test_matches_when_disabled(self):
        self.listener.enabled = False
        self.assertFalse(self.listener.matches("on_action_start"))

    def test_matches_with_condition(self):
        listener = CoordinatedListener(
            character=self.char,
            skill_data=self.skill,
            trigger_event="on_damage_dealt",
            condition=lambda **kwargs: kwargs.get("damage", 0) > 100,
        )
        self.assertTrue(listener.matches("on_damage_dealt", damage=500))
        self.assertFalse(listener.matches("on_damage_dealt", damage=50))

    def test_matches_with_default_condition(self):
        self.assertTrue(self.listener.matches("on_action_start", arbitrary_kwarg=42))

    def test_cooldown_prevents_match(self):
        listener = CoordinatedListener(
            character=self.char,
            skill_data=self.skill,
            trigger_event="on_action_start",
            cooldown_ticks=30,
        )
        listener.remaining_cooldown = 10
        self.assertFalse(listener.matches("on_action_start"))

    def test_is_on_cooldown(self):
        listener = CoordinatedListener(
            character=self.char,
            skill_data=self.skill,
            trigger_event="on_action_start",
            cooldown_ticks=30,
        )
        self.assertFalse(listener.is_on_cooldown())
        listener.remaining_cooldown = 10
        self.assertTrue(listener.is_on_cooldown())


class TestCoordinatedActionSystemRegistration(unittest.TestCase):
    """测试 CoordinatedActionSystem 注册与注销"""

    def setUp(self):
        self.system = CoordinatedActionSystem()
        self.char_a = _make_character("CharA")
        self.char_b = _make_character("CharB")
        self.skill_a = _make_skill_data("SkillA")
        self.skill_b = _make_skill_data("SkillB")

    def test_register_listener(self):
        listener = CoordinatedListener(self.char_a, self.skill_a, "on_action_start")
        self.system.register_listener(listener)
        self.assertIn("on_action_start", self.system.listeners)
        self.assertEqual(len(self.system.listeners["on_action_start"]), 1)

    def test_register_multiple_listeners_same_event(self):
        l1 = CoordinatedListener(self.char_a, self.skill_a, "on_action_start")
        l2 = CoordinatedListener(self.char_b, self.skill_b, "on_action_start")
        self.system.register_listener(l1)
        self.system.register_listener(l2)
        self.assertEqual(len(self.system.listeners["on_action_start"]), 2)

    def test_register_listeners_different_events(self):
        l1 = CoordinatedListener(self.char_a, self.skill_a, "on_action_start")
        l2 = CoordinatedListener(self.char_b, self.skill_b, "on_damage_dealt")
        self.system.register_listener(l1)
        self.system.register_listener(l2)
        self.assertIn("on_action_start", self.system.listeners)
        self.assertIn("on_damage_dealt", self.system.listeners)

    def test_unregister_character(self):
        l1 = CoordinatedListener(self.char_a, self.skill_a, "on_action_start")
        l2 = CoordinatedListener(self.char_b, self.skill_b, "on_action_start")
        self.system.register_listener(l1)
        self.system.register_listener(l2)
        self.system.unregister_character(self.char_a)
        self.assertEqual(len(self.system.listeners["on_action_start"]), 1)
        self.assertEqual(
            self.system.listeners["on_action_start"][0].character.char_id, "CharB"
        )

    def test_unregister_listener_specific(self):
        l1 = CoordinatedListener(self.char_a, self.skill_a, "on_action_start")
        l2 = CoordinatedListener(self.char_a, self.skill_b, "on_action_start")
        self.system.register_listener(l1)
        self.system.register_listener(l2)
        self.system.unregister_listener(self.char_a, "SkillA")
        self.assertEqual(len(self.system.listeners["on_action_start"]), 1)
        self.assertEqual(
            self.system.listeners["on_action_start"][0].skill_data.action_id, "SkillB"
        )


class TestCoordinatedActionSystemEventTriggering(unittest.TestCase):
    """测试事件触发与派生动作生成"""

    def setUp(self):
        self.system = CoordinatedActionSystem()
        self.char = _make_character("TestChar")
        self.skill = _make_skill_data("Coordinated_Shot")
        listener = CoordinatedListener(self.char, self.skill, "on_action_start")
        self.system.register_listener(listener)

    def test_event_triggers_action_creation(self):
        self.system.on_event("on_action_start")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 1)
        action = pending[0]
        self.assertEqual(action.skill_data.action_id, "Coordinated_Shot")
        self.assertEqual(action.character.char_id, "TestChar")

    def test_wrong_event_does_not_trigger(self):
        self.system.on_event("on_damage_dealt")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 0)

    def test_multiple_matching_listeners(self):
        skill2 = _make_skill_data("Coordinated_Strike")
        self.system.register_listener(
            CoordinatedListener(self.char, skill2, "on_action_start")
        )
        self.system.on_event("on_action_start")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 2)

    def test_disabled_listener_skipped(self):
        self.system.listeners["on_action_start"][0].enabled = False
        self.system.on_event("on_action_start")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 0)

    def test_condition_filters_events(self):
        # Clear default listener and add conditional one
        self.system.listeners.clear()
        listener = CoordinatedListener(
            self.char,
            self.skill,
            "on_damage_dealt",
            condition=lambda **kwargs: kwargs.get("element") == "Fire",
        )
        self.system.register_listener(listener)

        self.system.on_event("on_damage_dealt", element="Fire")
        self.assertEqual(len(self.system.get_pending_actions()), 1)

        self.system.on_event("on_damage_dealt", element="Ice")
        self.assertEqual(len(self.system.get_pending_actions()), 0)


class TestCoordinatedActionSystemCooldown(unittest.TestCase):
    """测试协同动作冷却机制"""

    def setUp(self):
        self.system = CoordinatedActionSystem()
        self.char = _make_character("TestChar")
        self.skill = _make_skill_data("Coordinated_Shot")
        listener = CoordinatedListener(
            self.char, self.skill, "on_action_start", cooldown_ticks=30
        )
        self.listener = listener
        self.system.register_listener(listener)

    def test_first_trigger_sets_cooldown(self):
        self.system.on_event("on_action_start")
        self.assertEqual(len(self.system.get_pending_actions()), 1)
        self.assertEqual(self.listener.remaining_cooldown, 30)

    def test_cooldown_prevents_second_trigger(self):
        self.system.on_event("on_action_start")
        self.system.clear_pending()
        self.system.on_event("on_action_start")
        self.assertEqual(len(self.system.get_pending_actions()), 0)

    def test_cooldown_expires_after_ticks(self):
        self.system.on_event("on_action_start")
        self.system.clear_pending()
        for _ in range(30):
            self.system.tick_cooldowns()
        self.system.on_event("on_action_start")
        self.assertEqual(len(self.system.get_pending_actions()), 1)

    def test_zero_cooldown_allows_consecutive_triggers(self):
        listener = CoordinatedListener(
            self.char, _make_skill_data("NoCooldown"), "on_dodge", cooldown_ticks=0
        )
        self.system.register_listener(listener)
        self.system.on_event("on_dodge")
        self.system.on_event("on_dodge")
        self.assertEqual(len(self.system.get_pending_actions()), 2)


class TestCoordinatedActionSystemPendingActions(unittest.TestCase):
    """测试待处理动作队列管理"""

    def setUp(self):
        self.system = CoordinatedActionSystem()
        self.char = _make_character("TestChar")
        self.skill = _make_skill_data("Coordinated_Shot")
        listener = CoordinatedListener(self.char, self.skill, "on_action_start")
        self.system.register_listener(listener)

    def test_get_pending_actions_returns_copy(self):
        self.system.on_event("on_action_start")
        actions1 = self.system.get_pending_actions()
        actions2 = self.system.get_pending_actions()
        self.assertEqual(len(actions1), 1)
        self.assertEqual(len(actions2), 0)  # Second call gets empty list

    def test_clear_pending_removes_all(self):
        self.system.on_event("on_action_start")
        self.assertEqual(self.system.has_pending_actions(), True)
        self.system.clear_pending()
        self.assertEqual(self.system.has_pending_actions(), False)

    def test_has_pending_actions(self):
        self.assertFalse(self.system.has_pending_actions())
        self.system.on_event("on_action_start")
        self.assertTrue(self.system.has_pending_actions())


class TestCoordinatedActionSystemMultiEvent(unittest.TestCase):
    """测试多事件类型协同触发"""

    def setUp(self):
        self.system = CoordinatedActionSystem()
        self.char = _make_character("TestChar")

    def test_dodge_event(self):
        skill = _make_skill_data("DodgeFollowUp")
        listener = CoordinatedListener(self.char, skill, "on_dodge")
        self.system.register_listener(listener)
        self.system.on_event("on_dodge")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0].skill_data.action_id, "DodgeFollowUp")

    def test_damage_dealt_event(self):
        skill = _make_skill_data("RevengeStrike")
        listener = CoordinatedListener(self.char, skill, "on_damage_dealt")
        self.system.register_listener(listener)
        self.system.on_event("on_damage_dealt", damage=300)
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0].skill_data.action_id, "RevengeStrike")

    def test_action_start_event(self):
        skill = _make_skill_data("TwinAttack")
        listener = CoordinatedListener(self.char, skill, "on_action_start")
        self.system.register_listener(listener)
        self.system.on_event("on_action_start", action_id="Attack_Normal")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0].skill_data.action_id, "TwinAttack")


class TestCoordinatedActionSystemEventHandlers(unittest.TestCase):
    """测试事件处理器注册与 blinker 信号集成"""

    def setUp(self):
        self.system = CoordinatedActionSystem()
        self.char = _make_character("TestChar")
        self.skill = _make_skill_data("Coordinated_Shot")
        listener = CoordinatedListener(self.char, self.skill, "on_action_start")
        self.system.register_listener(listener)

    def test_register_event_handlers_connects_signals(self):
        self.system.register_event_handlers()
        # 验证注册后可通过信号触发
        from core_control.dispatcher import on_action_start

        on_action_start.send(self, action_id="Attack_Normal")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 1)
        self.system.unregister_event_handlers()

    def test_unregister_event_handlers_disconnects(self):
        self.system.register_event_handlers()
        self.system.unregister_event_handlers()
        # 清理任何残留的 pending
        self.system.clear_pending()

        from core_control.dispatcher import on_action_start

        on_action_start.send(self, action_id="Attack_Normal")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 0)

    def test_damage_dealt_signal(self):
        skill = _make_skill_data("RevengeStrike")
        listener = CoordinatedListener(self.char, skill, "on_damage_dealt")
        self.system.register_listener(listener)
        self.system.register_event_handlers()

        from core_control.dispatcher import on_damage_dealt

        on_damage_dealt.send(self, damage=500)
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0].skill_data.action_id, "RevengeStrike")
        self.system.unregister_event_handlers()

    def test_dodge_signal(self):
        skill = _make_skill_data("DodgeCounter")
        listener = CoordinatedListener(self.char, skill, "on_dodge")
        self.system.register_listener(listener)
        self.system.register_event_handlers()

        from core_control.dispatcher import on_dodge

        on_dodge.send(self)
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0].skill_data.action_id, "DodgeCounter")
        self.system.unregister_event_handlers()


class TestCoordinatedActionSystemIntegration(unittest.TestCase):
    """测试协同动作系统与队伍管理器的集成场景"""

    def setUp(self):
        self.system = CoordinatedActionSystem()
        self.char_active = _make_character("CharActive")
        self.char_standby = _make_character("CharStandby")
        self.char_standby.state = CharacterState.STANDBY

    def test_standby_character_coordinated_skill(self):
        """后台角色的协同技能在前台事件触发时生成派生动作"""
        skill = _make_skill_data("Support_Fire", trigger_type=TriggerType.COORDINATED)
        listener = CoordinatedListener(
            self.char_standby,
            skill,
            "on_action_start",
            condition=lambda **kwargs: kwargs.get("action_id", "").startswith("Attack"),
        )
        self.system.register_listener(listener)

        self.system.on_event("on_action_start", action_id="Attack_Normal")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0].skill_data.action_id, "Support_Fire")
        self.assertEqual(pending[0].character.char_id, "CharStandby")

    def test_multiple_characters_coordinated_skills(self):
        """多个后台角色各自注册协同技能"""
        char_c = _make_character("CharC")
        char_c.state = CharacterState.STANDBY

        skill_b = _make_skill_data("Support_B")
        skill_c = _make_skill_data("Support_C")

        self.system.register_listener(
            CoordinatedListener(self.char_standby, skill_b, "on_action_start")
        )
        self.system.register_listener(
            CoordinatedListener(char_c, skill_c, "on_action_start")
        )

        self.system.on_event("on_action_start")
        pending = self.system.get_pending_actions()
        self.assertEqual(len(pending), 2)
        action_ids = {a.skill_data.action_id for a in pending}
        self.assertEqual(action_ids, {"Support_B", "Support_C"})


if __name__ == "__main__":
    unittest.main()
