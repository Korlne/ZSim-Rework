# parallel/ — 多线程并行模拟

## 架构

本模块支持同时运行多个不同队伍配置的模拟实例，利用 `concurrent.futures.ThreadPoolExecutor` 并行执行。

## 关键约定

### SimConfig 和 SimResult

- `SimConfig`：包含队伍配置路径、APL 路径、Seed、模拟模式、最大 tick 数
- `SimResult`：包含 config、success、error_message、log_path、total_damage

### 线程安全策略

- **每个线程创建独立的 GameState 实例**
- **每个线程使用独立的 RNG Manager**（独立 Seed）
- **每个线程使用独立的 Blinker Namespace**（信号隔离）
- GameState 不跨线程共享，避免锁竞争

### 使用方式

```python
from parallel.runner import ParallelRunner, SimConfig

configs = [
    SimConfig(name="team_a", team_config_path="...", seed=42, ...),
    SimConfig(name="team_b", team_config_path="...", seed=99, ...),
]
runner = ParallelRunner()
results = runner.run_parallel(configs, max_workers=4)
```

## 测试

```bash
PYTHONPATH="." python tests/parallel/test_runner.py
```
