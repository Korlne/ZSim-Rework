# entities/ — 领域实体模型

## 架构

本模块定义角色、敌人、属性数据模型和枚举类型。
所有 Pydantic 模型使用严格模式。

## 关键约定

- **BaseStats 使用 Pydantic strict 模式**。全局 `@field_validator('*', mode='before')` 确保所有数值字段非负。
- **`def_val` 使用 alias="def"**。因 `def` 是 Python 关键字，访问时使用 `.def_val` 属性，构造时通过 `model_validate({"def": value})`
- **Character 不是 Pydantic 模型**（plain Python class），因为需要持有活跃引用和可变状态。
- **Character.realtime_modifiers** 是运行时属性修饰字典，BUFF 系统通过此字段实时调整角色属性。
- **EnemyState.anomaly_buildup** 是 `Dict[ElementTag, float]`，记录各属性异常积蓄值。

## 测试

```bash
PYTHONPATH="." python tests/entities/test_models.py
PYTHONPATH="." python tests/entities/test_character.py
PYTHONPATH="." python tests/entities/test_enemy.py
PYTHONPATH="." python tests/entities/test_integration.py
```
