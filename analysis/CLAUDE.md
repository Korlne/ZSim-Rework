# analysis/ — 数据分析与报表

## 架构

本模块提供模拟结束后的数据分析功能。加载 JSONL 日志，计算 DPS 曲线、伤害分布、异常统计等指标，
支持控制台 ASCII 报表、JSON 报表文件和 PNG 图表导出。

## 关键约定

### DataAnalyzer

- **加载 JSONL**：`load(jsonl_path)` 将事件流载入内存
- **误差边界**：所有数值返回 `(value, error_margin)` 元组，`FRAME_ERROR_MARGIN = 4`
- **DPS 曲线**：`compute_dps_curve(window_size=60)` 使用滑动窗口

### 可用分析维度

| 方法 | 返回 |
|---|---|
| `compute_dps_curve(window_size)` | `List[(tick, dps, error_margin)]` |
| `compute_damage_distribution()` | `Dict[char_id, {total_damage, percentage}]` |
| `compute_skill_distribution()` | `Dict[action_id, {total_damage, count, percentage}]` |
| `compute_anomaly_summary()` | `Dict[element, {trigger_count, total_damage}]` |
| `compute_energy_curve(char_id)` | `List[(tick, energy)]` |
| `compute_buff_uptime(char_id, buff_id)` | `percentage` |

### PNG 导出

- 使用 `matplotlib` 生成图表
- `export_png_charts(output_dir)` 输出到 `reports/` 目录
- 图表类型：DPS 曲线、伤害占比饼图、技能占比饼图、异常统计图、能量轴曲线

### JSON 报表

- `export_json_report(output_path)` 输出格式化的 JSON 数据
- 报表数据供 PyQt6 GUI 或外部工具消费

## 依赖

- `matplotlib`（PNG 图表生成）
- `json`（标准库，JSONL 解析和报表输出）
- 本模块为独立分析层，不依赖 core_control 或 combat（仅读取 JSONL 文件）

## 测试

```bash
PYTHONPATH="." python tests/analysis/test_data_analyzer.py
```
