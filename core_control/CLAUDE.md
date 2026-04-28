# core_control/ — 核心控制层

## 架构

本模块是 ZSim 的中枢神经系统，负责事件广播、游戏状态管理和 APL 排轴执行。
所有其他模块通过 Blinker 信号与本模块解耦通信。

## 关键约定

- **dispatcher.py 是全局信号的单一数据源**。所有新增信号必须在此文件定义，使用共享的 `_zsim_signals` Namespace 实例。
- **GameState 使用 Pydantic 严格模式**。当引用尚未导入的类型（如 `TeamManager`），必须在文件末尾导入并调用 `model_rebuild()`，仅用 `TYPE_CHECKING` 不够。
- **APLManager 通过依赖注入接收 validator**（`IResourceValidator` 接口），方便测试时替换为 Mock。
- **EventBus.run_simulation()** 是主循环入口，每 Tick 广播 `on_tick` 信号，由各订阅模块自行处理。

## 依赖关系

- `core_control` → `entities`（直接引用 `EnemyState`, `Character`）
- `core_control` → `combat`（通过前向引用引用 `TeamManager`）
- 其他所有模块 → `core_control`（通过 Blinker 信号订阅）

## 测试

```bash
PYTHONPATH="." python tests/core_control/test_event_bus.py
PYTHONPATH="." python tests/core_control/test_apl_manager.py
```
