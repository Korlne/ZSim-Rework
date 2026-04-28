# data_io/ — IO 与基础设施

## 重要

目录名为 `data_io/`，**不是 `io/`**。原 `io/` 与 Python 标准库 `io` 模块冲突，已重命名。

## 关键约定

### DataLoader
- 构造时指定 `data_root`（默认 `"data"`），所有路径基于此根目录
- `load_apl_sequence()` 支持多轨道 tracks 结构（与角色/邦布统一 schema）
- JSON 路径使用完整相对路径，如 `"data/apl/sample_apl.json"`
- 加载的枚举字段（如 ElementTag）不会自动转换，需手动处理

### StructuredLogger
- 每条日志为单行 JSON（JSONL 格式）
- 日志文件路径：`logs/simulation_YYYYMMDD_HHMMSS.jsonl`
- `register_event_handlers()` 连接所有 12 个 Blinker 信号
- 模拟结束后**必须调用 `disconnect_event_handlers()`** 清理连接

### ErrorReporter
- 监听 `on_error_raised` 信号
- 错误分类：ENERGY / RESOURCE / CHAIN / VALIDATION / DATA / OVERFLOW / UNKNOWN
- 输出结构化的 `ErrorReport` 含 tick / source_module / message / context

## 数据目录结构

```
data/
  characters/   角色面板 JSON
  enemies/      敌人属性 JSON
  skills/       技能倍率 JSON
  equipment/    音擎/驱动盘 JSON
  apl/          APL 排轴 JSON
```

## 测试

```bash
PYTHONPATH="." python tests/data_io/test_data_loader.py
PYTHONPATH="." python tests/data_io/test_structured_logger.py
PYTHONPATH="." python tests/data_io/test_error_reporter.py
```
