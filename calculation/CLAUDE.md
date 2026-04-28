# calculation/ — 数值结算与系统管理

## 架构

本模块实现 6 大伤害结算公式、RNG 管理、BUFF 系统、异常管理和装备系统。
结算类为纯函数/静态方法，不持有可变状态，确保线程安全和可测试性。

## 关键约定

### 6 大结算模块（calculator.py）
所有方法为 `@staticmethod`，纯函数，无副作用：
- **RegularMul**: 常规直伤 = 基础伤害区 × 增伤区 × 暴击期望 × 防御区 × 抗性区 × 减易伤区 × 失衡易伤区 × 特殊倍率区 × 贯穿伤害区
- **AnomalyMul**: 异常积蓄值计算
- **StunMul**: 失衡值累积计算
- **CalAnomaly**: 异常伤害结算（含属性倍率常数）
- **CalDisorder**: 紊乱伤害结算（重置基础伤害区和异常增伤区）
- **CalPolarityDisorder**: 极性紊乱结算（继承紊乱框架）

### RNGManager
- 基于 `random.Random` 包装，构造时传入 seed
- 同一 seed 保证相同随机序列（用于 E2E 复现测试）
- 方法：`roll_crit()`, `roll_probability()`, `random_float()`, `random_int()`

### BuffManager
- 监听 `on_tick` 信号处理倒计时，到期自动移除
- `apply_buff()` 支持三种刷新策略：REPLACE / STACK / EXTEND
- BUFF 通过 `character.realtime_modifiers` 修饰角色实时属性

### AnomalyDisorderManager
- 异常阈值：`ANOMALY_THRESHOLD = 1000`
- 紊乱后保留比例：`DISORDER_RETENTION_RATIO = 0.3`
- 各属性异常效果：感电/强击/冻结/侵蚀/灼烧/惧风

### EquipmentManager
- 管理音擎（WEngine）、驱动盘（DriveDisc slot 1-6）、套装效果（DriveDiscSet）
- 套装效果自动转换为 BuffData 挂载到角色

## 测试

```bash
PYTHONPATH="." python tests/calculation/test_calculator.py
PYTHONPATH="." python tests/calculation/test_rng_manager.py
```
