# ZSim 2.0 — 项目开发文档

## 项目概述

ZSim 2.0 是一款基于 **帧级精确模拟** 的动作游戏（绝区零）战斗数值仿真系统。
系统以 **60 Tick/秒** 的时间线驱动，加载外部 JSON 数据（角色面板、敌人属性、技能倍率、APL 排轴），
通过事件驱动的模块化架构完成伤害结算、异常积蓄、失衡累积、BUFF 管理等全部战斗逻辑，
支持固定 Seed 复现、结构化日志输出、PNG 报表导出和多线程并行模拟。

- **入口**: `main.py` → `run_simulation.run_simulation()`
- **Python**: >= 3.13
- **核心依赖**: `pydantic >= 2.12.5`, `blinker >= 1.9.0`
- **测试框架**: `unittest`（标准库）
- **类型检查**: `mypy`

## 模块架构概览

```
main.py → run_simulation.py ──┬── core_control/   事件总线、游戏状态、APL管理器、信号定义
                               ├── entities/       角色、敌人、基础属性等数据模型
                               ├── combat/         技能数据、队伍管理、资源校验、协同动作
                               ├── calculation/    6大结算模块、RNG、BUFF、异常、装备
                               ├── data_io/        数据加载、结构化日志、错误报告
                               ├── analysis/       数据分析、JSON报表、PNG图表导出
                               └── parallel/       多线程并行模拟
```

### core_control/ — 核心控制层
| 文件 | 关键类/功能 | 职责 |
|---|---|---|
| `game_state.py` | `GameState(BaseModel)`, `SimMode(Enum)` | 全局游戏状态：Tick 计数、队伍引用、运行模式 |
| `event_bus.py` | `EventBus` | 主循环：广播 `on_tick`，推进 Tick，终止检测 |
| `apl_manager.py` | `APLManager` | APL 排轴解析与逐帧动作执行 |
| `dispatcher.py` | 12 个 Blinker 信号 | 全局事件信号定义（单一数据源） |
| `interfaces.py` | `IResourceValidator(ABC)` | 资源校验接口定义 |
| `decorators.py` | `@emit_on_error` | AOP 装饰器：自动广播异常到 `on_error_raised` |
| `exceptions.py` | `ZSimBaseException`, `ResourceValidationError`, `ActionExecutionError` | 异常类型体系 |
| `logger.py` | `setup_logging()` | 日志基础设施 |

### entities/ — 实体数据模型
| 文件 | 关键类 | 职责 |
|---|---|---|
| `models.py` | `BaseStats(BaseModel)` | 12 项战斗属性（ATK/暴击/穿透/异常精通等） |
| `character.py` | `Character` | 角色实例：身份/状态/资源/实时修饰器 |
| `enemy.py` | `EnemyState(BaseModel)` | 敌人状态：生命/失衡/抗性/异常积蓄槽 |
| `enums.py` | `FactionTag`, `SpecialtyTag`, `ElementTag`, 等 | 枚举定义 |

### combat/ — 战斗动作与判定
| 文件 | 关键类 | 职责 |
|---|---|---|
| `skill_data.py` | `SkillData(BaseModel)`, `SkillType`, `TriggerType` | 技能帧级数据模型（倍率/无敌帧/消耗等） |
| `skill.py` | `SkillAction` | 技能运行时实例（帧计数/完成标记） |
| `team_manager.py` | `TeamManager` | 队伍管理：前后台切换/喧响值/连携点 |
| `resource_validator.py` | `ResourceValidator` | 8 维资源校验（能量/CD/HP/喧响/连携等） |
| `coordinated_system.py` | `CoordinatedActionSystem`, `CoordinatedListener` | 后台协同与派生动作系统 |

### calculation/ — 数值结算
| 文件 | 关键类 | 职责 |
|---|---|---|
| `calculator.py` | `RegularMul`, `AnomalyMul`, `StunMul`, `CalAnomaly`, `CalDisorder`, `CalPolarityDisorder` | 6 大结算模块（纯函数，无状态） |
| `rng_manager.py` | `RNGManager` | 全局 RNG：固定 Seed 确保复现 |
| `buff_manager.py` | `BuffManager`, `BuffData` | BUFF 管理器：叠层/刷新/到期移除 |
| `anomaly_manager.py` | `AnomalyDisorderManager`, `AnomalyState` | 异常/紊乱状态管理 |
| `equipment.py` | `EquipmentManager`, `WEngine`, `DriveDisc` | 装备系统：音擎/驱动盘/套装效果 |

### data_io/ — IO 与基础设施
| 文件 | 关键类 | 职责 |
|---|---|---|
| `data_loader.py` | `DataLoader`, `CharacterData`, `EnemyData` | 从 JSON 加载角色/敌人/技能/装备/APL |
| `structured_logger.py` | `StructuredLogger` | JSON Lines 结构化日志输出 |
| `error_reporter.py` | `ErrorReporter`, `ErrorReport` | 统一错误分类与结构化报告 |

> 注意：目录名为 `data_io/`（非 `io/`），因 `io/` 与 Python 标准库 `io` 冲突。

### analysis/ — 数据分析
| 文件 | 关键类 | 职责 |
|---|---|---|
| `data_analyzer.py` | `DataAnalyzer` | 加载 JSONL → DPS曲线/伤害占比/异常统计/PNG导出 |

### parallel/ — 并行模拟
| 文件 | 关键类 | 职责 |
|---|---|---|
| `runner.py` | `ParallelRunner`, `SimConfig`, `SimResult` | ThreadPoolExecutor 多线程并行 |

---

## 事件驱动模式

项目基于 **Blinker Pub-Sub 信号系统** 实现模块间解耦。
所有信号在 `core_control/dispatcher.py` 中统一定义：

```python
from blinker import Namespace
_zsim_signals = Namespace()

on_tick = _zsim_signals.signal('on_tick')
on_action_start = _zsim_signals.signal('on_action_start')
on_damage_dealt = _zsim_signals.signal('on_damage_dealt')
# ... 共 12 个信号
```

### 信号命名规范
- 信号名即 Python 变量名，以 `on_` 为前缀
- 信号对象为 Blinker `NamedSignal`，通过 `Namespace.signal('name')` 创建
- 新增信号必须在 `dispatcher.py` 中定义（单一数据源原则）

### 订阅模式
```python
# 连接处理器（默认 weak=False，防止被 GC 回收后丢失连接）
on_damage_dealt.connect(handler_function, sender=None, weak=False)

# 处理器签名：接收 sender 和 **kwargs 以兼容任意事件载荷
def handler_function(sender, **kwargs):
    ...

# 模拟结束后必须手动 disconnect 清理连接
on_damage_dealt.disconnect(handler_function)
```

### 事件生命周期（每 Tick）
```
EventBus.run_simulation()
  └─ on_tick → [BuffManager/APLManager/cooldown ticks]
       └─ APLManager.process_next_action()
            └─ on_action_start → [damage pipeline / CoordinatedActionSystem]
                 └─ on_damage_dealt → [StructuredLogger / CoordinatedActionSystem]
                 └─ on_damage_applied → [StructuredLogger / AnomalyManager]
  └─ on_combat_end (循环结束时)
```

**关键约束**：
- 同一 Tick 内，on_action_start 的处理在 `process_next_action()` 内部同步完成，不会跨 Tick 延迟
- `blinker` 连接使用 `weak=False`，模拟结束后需手动 disconnect，否则重复运行会累积重复 handler
- 每个线程使用独立的 Blinker Namespace 以保证线程安全

---

## Pydantic 严格模式约定

所有 Pydantic 数据模型统一使用严格模式：

```python
from pydantic import BaseModel, ConfigDict

class MyModel(BaseModel):
    model_config = ConfigDict(strict=True, arbitrary_types_allowed=True)
```

**严格模式的含义**：
- 输入类型必须与字段声明精确匹配（不进行隐式类型转换）
- 例如：`field: int` 不接受字符串 `"123"`
- 枚举字段需手动转换后再传入 `model_validate()`

### 前向引用处理
当 Pydantic 模型中引用了尚未定义的类（循环引用），需在文件末尾导入并调用 `model_rebuild()`：

```python
# 仅用 TYPE_CHECKING 保护不够 — Pydantic 运行时需要该类
from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from combat.team_manager import TeamManager

class GameState(BaseModel):
    team: Optional[TeamManager] = None  # 前向引用

# 文件末尾
from combat.team_manager import TeamManager
GameState.model_rebuild()
```

### 别名处理
`EnemyState.def_val` 使用 `alias="def"` 因 `def` 是 Python 关键字：
```python
enemy = EnemyState.model_validate({"def": 600, ...})
# 或直接赋值属性：enemy.def_val = 600
```

---

## 日志模块命名规范

日志 logger 命名格式：`zsim.Module.ClassName`

```python
import logging
logger = logging.getLogger('zsim.Core.EventBus')
```

- 根命名空间：`zsim`
- 层级：`zsim.{模块}.{类名}`
- `core_control/logger.py` 中的 `setup_logging()` 为全局日志初始化入口

---

## 测试规范

### 目录结构
```
tests/
  __init__.py
  test_e2e_simulation.py          # 端到端集成测试 + Seed 复现测试
  core_control/
    test_event_bus.py             # EventBus Tick 广播测试
    test_apl_manager.py           # APLManager AOP 错误广播测试
  entities/
    test_models.py                # BaseStats 校验测试
    test_character.py             # Character 动作校验测试
    test_enemy.py                 # EnemyState 失衡/连携测试
    test_integration.py           # GameState → TeamManager 集成测试
  combat/
    test_team_manager.py          # TeamManager 测试 (23 tests)
    test_resource_validator.py    # ResourceValidator 测试 (26 tests)
    test_coordinated_system.py    # CoordinatedActionSystem 测试 (34 tests)
  integration/
    (暂无)
```

### 运行方式
```bash
# 单个测试文件（必须设置 PYTHONPATH）
PYTHONPATH="." python tests/combat/test_resource_validator.py

# 注意：unittest discover 从 tests/ 无法正确解析项目根模块
# 请使用 PYTHONPATH 方式运行
```

### 测试要求
- 每个模块必须有对应的测试文件
- 新增模块需同步创建 `tests/{module}/test_{name}.py`
- 提交前确保所有测试通过
- `tests/` 目录在 `.gitignore` 中，提交测试文件需 `git add -f`

---

## 代码风格约定

### 文档字符串
- **全部使用中文** docstring
- 描述类的职责和方法的用途

### 类型标注
- **所有函数/方法** 必须包含完整的参数类型和返回值类型标注
- 使用 `from __future__ import annotations`（如需要）
- 交叉模块类型引用使用 `TYPE_CHECKING` 保护

### 依赖注入
- 高层模块依赖抽象接口，不依赖具体实现
- 例如：`APLManager.__init__(self, validator: IResourceValidator)`
- 运行时通过构造函数注入具体实现

### @emit_on_error 装饰器
```python
from core_control.decorators import emit_on_error

@emit_on_error
def validate_action(self, action_id: str) -> bool:
    ...
```
- 装饰后的方法若抛出异常，会自动通过 `on_error_raised` 信号广播
- 适用于角色动作校验、状态切换等关键路径方法

### 校验器异常模式
- 验证失败时**抛出异常**（而非返回 `False`）
- 异常携带丰富上下文：`action_id`、`missing_resource`、`current_value`、`required_value`
- 上游调用方通过 `try/except` 捕获并处理

### 消费式队列模式
- `get_pending_actions()` 等查询方法在返回时清空内部队列
- 防止同一帧内重复获取

---

## 数据目录结构

```
data/
  characters/   角色面板 JSON
  enemies/      敌人属性 JSON
  skills/       技能倍率 JSON
  equipment/    音擎/驱动盘 JSON
  apl/          APL 排轴 JSON（多轨道 tracks 结构）
```

### APL 格式
APL 排轴使用多轨道 tracks 结构（参考 `data/apl/sample_apl.json`）：
```json
{
  "scenarioList": [{
    "data": {
      "tracks": [
        {"trackId": "char_1", "actions": [{"action_id": "anson_A1"}, ...]}
      ]
    }
  }]
}
```
邦布排轴与角色使用完全相同的 schema。

---

## 已知注意事项

- **`io/` 目录名冲突**: 项目的 IO 模块目录为 `data_io/`，`io/` 与 Python stdlib 冲突
- **Blinker 连接清理**: 模拟结束后必须 disconnect 所有信号连接，否则重复运行会累积 handler
- **前向引用**: Pydantic 模型的循环引用需在文件末尾 `import` + `model_rebuild()`
- **venv 工具缺失**: 项目 venv 无 pip/pytest/mypy，使用系统/Anaconda Python 运行开发工具
- **tests/ 在 .gitignore**: 提交测试文件需使用 `git add -f`
