# combat/ — 战斗动作与判定系统

## 架构

本模块实现技能数据模型、队伍管理、资源校验和协同动作系统。
依赖 `core_control`（信号/装饰器/异常）和 `entities`（角色/敌人模型）。

## 关键约定

- **SkillData 是技能的完整帧级定义**，包含倍率、消耗、无敌帧、命中帧等全部维度。所有技能数据从外部 JSON 加载。
- **SkillAction 是技能运行时实例**，持有 SkillData 引用 + Character 引用 + 帧计数器。它不继承 Pydantic BaseModel（因为持有活跃引用）。
- **ResourceValidator 校验失败时抛出异常**（不是返回 False）。异常携带 action_id、missing_resource、current_value、required_value 等上下文。
- **ResourceValidator 内部维护 cooldowns 字典**（`{char_id: {action_id: remaining_ticks}}`），每帧需调用 `tick_cooldowns()`。
- **CoordinatedActionSystem.get_pending_actions() 是消费式的**：返回副本后清空内部队列。同一帧多次调用不会重复获取。

## TeamManager

- `SWITCH_COOLDOWN_TICKS = 60`，`MAX_DECIBEL = 3000`
- 邦布作为可选的 Character 引用存储，使用相同 API 但无能量机制
- `register_listeners()` 连接 `on_coordinated_action` 信号以响应快速支援
- **必须在 GameState 中使用前向引用**：`team: Optional[TeamManager]`

## 前向引用处理

由于 `GameState` 和 `TeamManager` 存在循环引用：
1. `team_manager.py` 末尾调用 `GameState.model_rebuild()`
2. `game_state.py` 末尾也调用 `model_rebuild()`

## 测试

```bash
PYTHONPATH="." python tests/combat/test_team_manager.py
PYTHONPATH="." python tests/combat/test_resource_validator.py
PYTHONPATH="." python tests/combat/test_coordinated_system.py
```
