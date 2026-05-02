# PRD: ZSim 2.0 — 完整战斗模拟系统

## 1. Introduction/Overview

基于现有核心控制层（EventBus、APLManager、GameState）与实体模型（Character、EnemyState），完整实现绝区零战斗模拟系统的剩余模块：战斗动作与判定系统、数值结算与装备系统、IO与基础设施。最终交付一个支持多线程并行、可复现、输出结构化分析数据的精确帧级战斗模拟器。

系统以 60 tick/s 逐帧推进，基于 APL 排轴 JSON 严格还原实战动作序列，模拟精度可达单帧级别，误差控制在 ±4 帧以内。

## 2. Goals

- 实现全部 19 个大纲组件的开发、测试与集成
- 支持从外部 JSON 文件加载角色面板、敌人数据、装备数据、技能倍率
- 输出结构化 JSON Lines 日志，含每帧的动作流转、伤害数值、BUFF 变更
- 提供数据分析模块，生成 DPS 曲线、伤害占比饼图、异常触发统计、能量轴曲线，支持导出为 PNG 图片
- 提供 PyQt6 桌面 GUI，支持配置模拟参数、启动模拟、查看可视化报表
- 支持固定 Seed 的全局随机数，确保同一排轴多次模拟结果完全一致
- 支持多线程并行模拟不同队伍配置
- 所有数值误差以"模拟值 ± 误差值"格式呈现

## 3. User Stories

---

### Phase 3: 战斗动作与判定系统

#### US-001: 完善事件信号体系，新增战斗相关全局事件
**Description:** 作为核心层，需要在现有 5 个信号基础上，新增伤害结算、BUFF 变更、异常触发、协同动作等事件信号，为下游模块提供完整的订阅基础设施。

**Acceptance Criteria:**
- [ ] 新增 `on_damage_dealt`（伤害结算）、`on_damage_applied`（伤害应用）、`on_buff_changed`（BUFF 变更）、`on_anomaly_triggered`（异常触发）、`on_disorder_triggered`（紊乱触发）、`on_chain_attack`（连携技）、`on_dodge`（闪避）、`on_parry`（招架）信号
- [ ] 新增 `on_coordinated_action`（协同/派生动作）信号
- [ ] 所有信号在 `dispatcher.py` 中统一定义
- [ ] Typecheck passes

#### US-002: 实现 SkillAction 动作数据模型
**Description:** 作为战斗系统核心，需要定义动作的帧级详细数据模型，含命中帧、无敌帧、伤害倍率、失衡倍率、异常倍率、打断窗口、快照标记等全部维度。

**Acceptance Criteria:**
- [ ] 创建 `combat/` 模块目录，含 `__init__.py`、`skill.py`、`skill_data.py`
- [ ] `SkillData` Pydantic 模型：action_id, action_type（枚举：普攻/特殊技/强化特殊技/终结技/快速支援/招架/闪避/闪避反击）, trigger_type（Active/Reactive）, damage_multipliers（List[frame→multiplier]）, daze_multiplier, anomaly_multiplier, hit_frames（List[int]）, invincible_frames（List[frame_range]）, interruptible_frame（int, 0=不可打断）, allows_background_completion（bool）, is_snapshot（bool）, charge_branches（Dict[charge_time→variant_id]）, prerequisite_action_id（Optional）, hp_cost（float）
- [ ] `SkillAction` 运行时实例类：持有 SkillData 引用、执行者角色引用、当前帧计数、是否已完成
- [ ] Typecheck passes

#### US-003: 实现队伍/派系管理类 (TeamManager)
**Description:** 作为战斗系统，需要管理 1-3 名角色加 1 个邦布的编队，协调前后台切换、换人冷却、喧响值（大招能量）和连携技点数的共享资源。

**Acceptance Criteria:**
- [ ] 创建 `combat/team_manager.py`
- [ ] `TeamManager` 类：管理 characters 列表、bangboo 引用、当前前台角色索引
- [ ] 邦布系统与角色使用完全相同的 API（SkillAction 模型），但无能量机制（邦布动作不消耗能量，通过独立预设排轴触发）
- [ ] 实现 `switch_character(target_index)` — 切换前台角色，检查换人冷却
- [ ] 实现共享资源管理：`decibel_value`（喧响值，上限 3000）、`chain_points`（连携技点数）
- [ ] 实现 `get_active_character()`、`get_standby_characters()` 查询方法
- [ ] 监听快速支援事件，按顺位逻辑自动切人
- [ ] 集成到 GameState，替代当前 `characters: List[Character]` 为 `team: TeamManager`
- [ ] Typecheck passes
- [ ] Tests pass

#### US-004: 实现完整资源与条件校验器 (ResourceValidator)
**Description:** 作为动作释放的"海关"，需要完整实现 `IResourceValidator` 接口，校验能量、特殊资源、连携技条件、技能 CD 等，拒绝时携带详细报错上下文。

**Acceptance Criteria:**
- [ ] 创建 `combat/resource_validator.py`
- [ ] `ResourceValidator` 实现 `IResourceValidator.can_execute(action_id, state)` 方法
- [ ] 校验项：角色能量是否充足、特殊资源是否满足、连携技释放条件（敌人是否失衡、剩余点数）、技能冷却是否就绪
- [ ] 拒绝时抛出 `ResourceValidationError`（继承 ZSimBaseException），包含：action_id、缺失资源类型、当前值、需求值、上一成功指令
- [ ] 集成到 APLManager 替换当前的抽象 validator
- [ ] Typecheck passes
- [ ] Tests pass

#### US-005: 实现协同与派生动作系统
**Description:** 作为战斗系统的并行轴，后台角色满足条件时自动触发派生动作，不阻塞前台动作时间线，在同一 Tick 内并行独立结算。

**Acceptance Criteria:**
- [ ] 创建 `combat/coordinated_system.py`
- [ ] `CoordinatedActionSystem` 类：维护后台协同 Listener 注册表
- [ ] 角色初始化时，具有协同能力的角色注册其协同技能到 EventBus
- [ ] 监听 `on_action_start`、`on_damage_dealt`、`on_dodge` 等事件，满足条件时自动实例化 Reactive Action 并下发执行
- [ ] 派生动作与前台 APL 动作在同一 Tick 内并行结算，不阻塞前摇/后摇
- [ ] Typecheck passes
- [ ] Tests pass

---

### Phase 4: 数值、结算与装备系统

#### US-006: 实现 BUFF 管理器 (BuffManager)
**Description:** 作为动态修饰器系统，需监听 OnTick 处理倒计时，支持叠层/刷新/移除逻辑，修饰角色实时属性，并携带来源追溯 Tag。

**Acceptance Criteria:**
- [ ] 创建 `calculation/buff_manager.py`
- [ ] `BuffData` 模型：buff_id, name, source_tag（角色技能/武器/驱动盘/环境）, duration（剩余 tick）, max_stacks, current_stacks, modifiers（Dict[stat_name→value]）, refresh_policy（replace/stack/extend）
- [ ] `BuffManager` 类：监听 `on_tick` 减少 duration，到期自动移除
- [ ] `apply_buff(character, buff_data)` — 将 buff 应用到角色 realtime_modifiers
- [ ] `remove_buff(character, buff_id)` — 移除 buff 并还原属性
- [ ] 监听 `on_damage_dealt` / `on_action_start` 等事件以支持触发式 buff
- [ ] Typecheck passes
- [ ] Tests pass

#### US-007: 实现全局 RNG 管理器 (RNGManager)
**Description:** 作为复现性保障，统一提供暴击判定、概率触发的随机数，支持固定 Seed 确保同配置同结果。

**Acceptance Criteria:**
- [ ] 创建 `calculation/rng_manager.py`
- [ ] `RNGManager` 类：基于 Python `random.Random` 实例，构造时接收 seed
- [ ] `roll_crit(crit_rate)` → bool：暴击判定
- [ ] `roll_probability(probability)` → bool：通用概率判定
- [ ] `random_float(min, max)` → float：范围随机数
- [ ] `random_int(min, max)` → int：范围随机整数
- [ ] 同一 seed 下多次调用产生完全相同的随机序列
- [ ] Typecheck passes
- [ ] Tests pass（验证 seed 复现性）

#### US-008: 实现计算器 (Calculator — 6大结算模块)
**Description:** 作为纯粹的数值计算模块，按现游戏版本实际公式实现完整的伤害、异常、失衡、紊乱、极性紊乱结算。参考 `Docs/伤害与战斗数值计算公式提取.md` 及 ZSim 开源项目的 `Calculator.py`、`CalAnomaly.py`。

**Acceptance Criteria:**
- [ ] 创建 `calculation/calculator.py`，包含以下独立结算类：

##### 1. 常规伤害结算 (`RegularMul`)
- [ ] 常规直伤期望 = 基础伤害区 × 增伤区 × 暴击期望 × 防御区 × 抗性区 × 减易伤区 × 失衡易伤区 × 特殊倍率区 × 贯穿伤害区
- [ ] **基础伤害区** = ((技能伤害倍率 / 攻击次数) + 局内额外伤害倍率) × 对应面板属性 × (1 + 局内百分比基础伤害增加) + 局内固定基础伤害增加
  - 对应面板属性（以攻击力为例）= 面板攻击力 × (1 + 局内百分比攻击力加成) + 局内固定攻击力加成
- [ ] **增伤区** = 1 + 属性增伤 + 技能触发类型增伤 + 标签增伤 + 全类型增伤
- [ ] **暴击期望** = 1 + min(1, 暴击率) × 暴击伤害
- [ ] **防御区** = 攻击方等级基数 / (受击方有效防御 + 攻击方等级基数)（贯穿伤害该乘区固定为1）
  - 受击方有效防御 = max(0, 受击方防御 × (1 - 攻击方穿透率) - 攻击方穿透值)
- [ ] **抗性区** = 1 - (怪物对应属性抗性 - 属性抗性降低 - 属性抗性穿透) + 全伤害抗性降低 + 全抗性穿透增加 + 快照抗性穿透
- [ ] **减易伤区** = 1 + 对应属性易伤 + 全属性易伤
- [ ] **失衡易伤区**：怪物失衡时 = 1 + 怪物失衡易伤 + 失衡易伤增幅 + 全时段失衡易伤；非失衡时 = 1 + 全时段失衡易伤
- [ ] **特殊倍率区** = 1 + 特殊乘区加成
- [ ] **贯穿伤害区** = 1 + 贯穿伤害增加

##### 2. 异常积蓄值结算 (`AnomalyMul`)
- [ ] 异常积蓄值 = 基础积蓄值 × (异常掌控 / 100) × (1 + 属性异常积蓄效率提升 + 触发类型异常积蓄效率提升) × 积蓄抗性乘区 × 元素伤害比例 / 攻击次数
- [ ] 异常掌控 = 面板异常掌控 × (1 + 局内百分比异常掌控) + 局内固定异常掌控
- [ ] 积蓄抗性乘区 = 1 - 属性异常积蓄抗性降低 - 怪物自身属性异常积蓄抗性

##### 3. 失衡值结算 (`StunMul`)
- [ ] 失衡值累积 = 冲击力 × (技能失衡倍率 / 攻击次数) × 失衡抗性乘区 × 失衡值增幅 × 受到失衡值提升
- [ ] 冲击力 = 面板冲击力 × (1 + 局内百分比冲击力) + 局内固定冲击力
- [ ] 失衡抗性乘区 = 1 - 局内失衡抗性降低 - 额外失衡抗性降低 - 怪物对应属性失衡抗性
- [ ] 失衡值增幅 = 1 + 触发类型失衡增幅 + 全局失衡增幅 + 标签失衡增幅
- [ ] 受到失衡值提升 = 1 + 局内受到失衡值增加 + 额外受到失衡值增加

##### 4. 异常伤害结算 (`CalAnomaly`)
- [ ] 异常伤害期望 = 异常基础伤害区 × 增伤区 × 异常精通区 × 等级区系数 × 异常增伤区 × 激活型异常暴击区 × 防御区 × 抗性区 × 减易伤区 × 失衡易伤区 × 特殊倍率区 × 缩放比例因子
- [ ] **异常基础伤害区** = 攻击力 × 属性异常伤害倍率（物理=7.13, 火=0.5, 冰/烈霜=5, 电=1.25, 以太/玄墨=0.625）
- [ ] **异常精通区** = (面板异常精通 × (1 + 局内百分比异常精通) + 局内固定异常精通) / 100
- [ ] **等级区系数** = 基于虚拟角色等级的查表值
- [ ] **异常增伤区** = 1 + 对应属性异常额外增伤 + 全属性异常额外增伤
- [ ] **激活型异常暴击区** = 1 + 暴击率增加 × 暴击伤害增加（若无相关被动则为1）

##### 5. 紊乱伤害结算 (`CalDisorder`)
- [ ] 紊乱伤害基于异常伤害框架，重置基础伤害区和异常增伤区
- [ ] **紊乱基础伤害区** = (原异常快照基础伤害区 / 属性异常伤害倍率) × 紊乱倍率
  - 紊乱倍率 = 基于异常剩余时间的倍率计算 + 属性紊乱基础常数 + 局内属性紊乱倍率增幅 + 全局紊乱倍率增幅
- [ ] **紊乱异常增伤区** = 1 + 紊乱额外伤害增幅

##### 6. 极性紊乱伤害结算 (`CalPolarityDisorder`)
- [ ] 极性紊乱继承紊乱伤害框架，二次修改基础伤害区
- [ ] **极性紊乱基础伤害区** = (原紊乱基础伤害区 × 极性紊乱比例系数) + (特定角色异常精通值 × 附加伤害精通比例系数)

##### 集成要求
- [ ] 监听 `on_damage_dealt` 事件，计算后发出 `on_damage_applied` 事件
- [ ] 所有结算类为纯函数/静态方法，不持有可变状态
- [ ] 遵循现有代码风格：Pydantic 严格模式、类型标注、中文 docstring
- [ ] Typecheck passes
- [ ] Tests pass（验证已知输入→期望输出，每个结算类独立测试）

#### US-009: 实现装备系统 (EquipmentManager)
**Description:** 作为依赖注入层，管理音擎（武器）与驱动盘数据，将驱动盘套装效果转换为标准 Buff 挂载到角色。

**Acceptance Criteria:**
- [ ] 创建 `calculation/equipment.py`
- [ ] `WEngine` 模型：engine_id, name, base_atk, sub_stat（类型+值）, passive_effect（可选的 BuffData）
- [ ] `DriveDisc` 模型：disc_id, slot（1-6）, main_stat（类型+值）, sub_stats（List[类型+值]）
- [ ] `DriveDiscSet` 模型：set_id, two_piece_bonus（BuffData）, four_piece_bonus（BuffData）
- [ ] `EquipmentManager` 类：在角色初始化时注入装备，自动解析套装效果并生成 Buff
- [ ] 从 JSON 加载装备数据（通过 DataLoader）
- [ ] Typecheck passes
- [ ] Tests pass

#### US-010: 实现异常与紊乱管理 (AnomalyDisorderManager)
**Description:** 作为绝区零核心机制，监听属性异常积蓄槽满溢触发异常状态；监听异属性覆盖触发紊乱计算，并按规则清空异常槽。

**Acceptance Criteria:**
- [ ] 创建 `calculation/anomaly_manager.py`
- [ ] `AnomalyManager` 类：
  - 监听 `on_damage_applied`，将异常积蓄值累加到对应属性的异常槽
  - 当异常槽 >= 阈值时触发对应属性异常（感电/强击/冻结/侵蚀/灼烧/惧风）
  - 触发异常时发出 `on_anomaly_triggered` 事件
  - 检测异属性覆盖：若已有异常状态下叠加异属性异常，触发紊乱
  - 紊乱伤害公式：`紊乱伤害 = 基础紊乱倍率 * 异常精通 * (1 + 紊乱增伤)`
  - 紊乱后按比例清空异常槽（保留部分残余值）
- [ ] 各属性异常效果：感电（持续雷伤）、强击（单次高倍率物理伤）、冻结（控制+碎冰）、侵蚀（持续以太伤）、灼烧（持续火伤）、惧风（持续风伤）
- [ ] Typecheck passes
- [ ] Tests pass

---

### Phase 5: IO、记录与周边设施

#### US-011: 实现数据加载器 (DataLoader)
**Description:** 彻底剥离硬编码，从外部 JSON 文件加载角色面板、敌人数据、技能倍率、装备数据。符合开闭原则——新增数据源不修改加载器代码。

**Acceptance Criteria:**
- [ ] 创建 `io/data_loader.py`
- [ ] `DataLoader` 类：
  - `load_characters(json_path)` → List[Character]
  - `load_enemies(json_path)` → List[EnemyState]
  - `load_skill_data(json_path)` → Dict[action_id, SkillData]
  - `load_equipment(json_path)` → (List[WEngine], List[DriveDisc], List[DriveDiscSet])
  - `load_apl_sequence(json_path)` → APL 排轴数据（支持多轨道 tracks 结构，每个轨道对应一个角色/邦布，格式参考 `Docs/ZZZaxis_Timeline_example.json`）
- [ ] 邦布排轴 JSON 与角色 APL 使用完全相同的 schema 结构（同一 tracks 数组中，邦布作为独立 track）
- [ ] 统一的 JSON Schema 校验（使用 Pydantic 验证输入结构）
- [ ] 数据目录结构：`data/characters/`, `data/enemies/`, `data/skills/`, `data/equipment/`, `data/apl/`
- [ ] Typecheck passes
- [ ] Tests pass

#### US-012: 实现结构化日志记录器 (StructuredLogger)
**Description:** 替代当前简单的日志系统，监听 EventBus 所有关键事件，生成结构化 JSON Lines 格式日志，供下游数据分析使用。

**Acceptance Criteria:**
- [ ] 创建 `io/structured_logger.py`
- [ ] `StructuredLogger` 类：监听 `on_tick`、`on_action_start`、`on_damage_dealt`、`on_damage_applied`、`on_buff_changed`、`on_anomaly_triggered`、`on_disorder_triggered`、`on_chain_attack`、`on_combat_end` 等事件
- [ ] 每条日志为单行 JSON，含：`tick`、`event_type`、`timestamp`、`payload`
- [ ] `payload` 内容按事件类型结构化（如 damage 事件含 source_char、target_enemy、raw_damage、final_damage、crit、element）
- [ ] 输出到 `logs/simulation_YYYYMMDD_HHMMSS.jsonl`
- [ ] Typecheck passes
- [ ] Tests pass

#### US-013: 实现报错管理器 (ErrorReporter)
**Description:** 统一捕获各模块异常，输出友好调试信息并安全中断模拟。承载当前分散在 decorators 中的报错逻辑，提供结构化的错误报告。

**Acceptance Criteria:**
- [ ] 创建 `io/error_reporter.py`
- [ ] `ErrorReporter` 类：监听 `on_error_raised` 事件
- [ ] 错误分类：能量/资源不足、动作词典不匹配、APL 未识别实体、数据加载失败、数值溢出
- [ ] 输出格式：`{error_type, tick, source_module, message, context: {current_action, last_successful_action, missing_resource, ...}}`
- [ ] 集成到现有 `emit_on_error` 装饰器的错误广播流程
- [ ] Typecheck passes
- [ ] Tests pass

#### US-014: 实现数据分析器 (DataAnalyzer)
**Description:** 解析结构化日志，复现旧 ZSim 数据分析全部内容，输出 DPS 曲线、伤害占比饼图、技能占比饼图、异常触发次数、能量轴曲线等可视化数据。

**Acceptance Criteria:**
- [ ] 创建 `analysis/data_analyzer.py`
- [ ] `DataAnalyzer` 类：
  - 加载 JSONL 日志文件
  - `compute_dps_curve(window_size=60)` → List[(tick, dps)]
  - `compute_damage_distribution()` → Dict[char_id, total_damage, percentage]
  - `compute_skill_distribution()` → Dict[action_id, total_damage, count, percentage]
  - `compute_anomaly_summary()` → Dict[element, trigger_count, total_damage]
  - `compute_energy_curve(char_id)` → List[(tick, energy)]
  - `compute_buff_uptime(char_id, buff_id)` → percentage
- [ ] 误差显示格式：所有数值返回 `(value, error_margin)` 元组
- [ ] 控制台输出 ASCII 格式报表（饼图、曲线）
- [ ] 支持导出 PNG 图片：DPS 曲线图、伤害占比饼图、技能占比饼图、异常触发统计图、能量轴曲线图
- [ ] 使用 matplotlib 或 plotly 生成图表，输出到 `reports/` 目录
- [ ] 输出 JSON 格式的报表数据供 PyQt6 GUI 消费
- [ ] Typecheck passes

#### US-015: 多线程并行模拟支持
**Description:** 支持同时运行多个不同队伍配置的模拟实例，每个线程独立的 GameState、RNG Seed、日志输出，充分利用多核 CPU。

**Acceptance Criteria:**
- [ ] 创建 `parallel/runner.py`
- [ ] `ParallelRunner` 类：
  - `run_parallel(configs: List[SimConfig], max_workers: int)` → List[SimResult]
  - 每个 `SimConfig` 包含：队伍配置路径、APL 路径、Seed、模拟模式（循环/全程）、最大 tick 数
  - 每个线程独立 GameState、独立 RNG、独立日志文件
- [ ] 使用 `concurrent.futures.ThreadPoolExecutor`
- [ ] 线程安全：GameState 不跨线程共享，EventBus 与 Blinker 信号隔离
- [ ] 聚合所有线程结果，输出总览报表
- [ ] Typecheck passes
- [ ] Tests pass（验证多配置并行且结果一致）

#### US-016: 运行模式与超时控制完善
**Description:** 完善 EventBus 的三种运行模式（循环测试/全程模拟/超时控制），以及前端可配置的最大运行时间。

**Acceptance Criteria:**
- [ ] 在 `EventBus.run_simulation` 中集成运行模式枚举：`SimMode.LOOP`（可循环重复 X 次）、`SimMode.FULL`（执行全部序列一次）、`SimMode.TIMED`（限时运行）
- [ ] `GameState` 新增 `sim_mode: SimMode`、`loop_count: int`、`max_duration_seconds: float`（默认 300）
- [ ] 循环模式：APL 序列执行完后重置队列并重新开始，重复指定次数
- [ ] 全程模式：执行完成所有序列后等待伤害跳完即停止
- [ ] 超时控制：超过 `max_duration_seconds` 秒自动终止
- [ ] Typecheck passes
- [ ] Tests pass

---

### Phase 6: 集成、PyQt6 GUI 与端到端验证

#### US-017: 端到端集成测试 — 完整模拟流程
**Description:** 加载真实数据，跑通完整模拟流程：数据加载 → 队伍组建 → APL 解析 → 逐帧模拟 → 日志输出 → 数据分析报表。

**Acceptance Criteria:**
- [ ] `data/` 目录下提供示例数据（1 个角色、1 个敌人、1 套技能数据、1 个 APL 序列）
- [ ] 从 `main.py` 或 `run_simulation.py` 一键启动完整模拟
- [ ] 模拟运行至结束或超时，无未处理异常
- [ ] 生成结构化 JSONL 日志文件
- [ ] DataAnalyzer 输出完整报表（DPS 曲线、伤害占比、技能占比、异常统计、能量曲线）
- [ ] 同一 Seed 运行两次，日志逐行完全一致

#### US-018: 更新 CLAUDE.md / AGENTS.md 项目文档
**Description:** 将项目架构、模块依赖、开发约定写入 CLAUDE.md，确保后续 AI 辅助开发能理解代码库模式。

**Acceptance Criteria:**
- [ ] 项目根目录或关键子目录的 CLAUDE.md 文件包含：
  - 模块架构概览（core_control / entities / combat / calculation / io / analysis / parallel）
  - 事件驱动模式说明（Blinker 信号命名规范、订阅模式）
  - Pydantic 严格模式的约定
  - 日志模块命名规范（zsim.Module.ClassName）
  - 测试要求（每个模块对应的测试路径）

#### US-019: 实现 PyQt6 桌面 GUI
**Description:** 提供基于 PyQt6 的桌面客户端，支持配置模拟参数（队伍、APL、Seed、运行模式）、启动模拟、实时查看进度，以及查看可视化报表（DPS 曲线、伤害占比饼图、异常统计、能量曲线）。

**Acceptance Criteria:**
- [ ] 创建 `ui/` 目录
- [ ] 主窗口：多标签页布局（QTabWidget）
- [ ] 页面一：模拟配置页
  - 选择队伍配置 JSON（文件浏览器）
  - 选择 APL 排轴 JSON
  - 输入 Seed（QSpinBox，留空则随机生成）
  - 选择运行模式（QComboBox：循环/全程/限时）及对应参数
  - 设置最大 Tick 数 / 最大运行时间
  - "开始模拟"按钮（QPushButton）
- [ ] 页面二：模拟进度页
  - 实时显示当前 Tick、已完成百分比（QProgressBar + QLabel）
  - 实时日志流展示（QTextEdit 只读，自动滚动）
  - "停止模拟"按钮
- [ ] 页面三：分析报表页
  - 加载 JSONL 日志或选择最近的模拟记录
  - 嵌入 matplotlib 生成的 PNG 图表（DPS 曲线、伤害占比、技能占比、异常统计、能量曲线），通过 FigureCanvas 渲染
  - 显示数值误差范围（模拟值 ± 误差值）（QTableWidget）
- [ ] 页面四：历史记录页
  - QTableView 列出历史模拟记录，支持查看、对比、删除
- [ ] 后端通过 Python 直接调用模拟引擎（与 CLI 共享同一进程）
- [ ] 遵循现有项目代码风格：中文 docstring、类型标注、Pydantic 模型
- [ ] Typecheck passes

## 4. Functional Requirements

- FR-1: 系统以 60 tick/s 纯逐帧推进，不模拟怪物 AI 行为
- FR-2: APL 严格执行用户提供的 JSON 排轴，不设降级/后备指令
- FR-3: 动作的帧级数据支持命中帧、无敌帧、打断窗口、快照标记
- FR-4: 所有模块间通过 EventBus 发布-订阅通信，模块互不感知
- FR-5: BUFF 系统支持叠层、刷新、移除，携带来源 Tag
- FR-6: 异常系统支持 6 大属性异常 + 紊乱机制
- FR-7: 伤害、失衡、异常三大结算通道独立并行
- FR-8: 全局随机数支持 Seed 复现，暴击/概率判定统一走 RNG
- FR-9: 装备系统支持音擎 + 6 槽位驱动盘 + 套装效果
- FR-10: 数据加载完全依赖外部 JSON 文件，无硬编码
- FR-11: 结构化日志为 JSON Lines 格式，每行一条事件记录
- FR-12: 数据分析输出 DPS 曲线、伤害占比、技能占比、异常统计、能量曲线
- FR-13: 所有数值以"模拟值 ± 误差值"格式呈现
- FR-14: 支持循环测试、全程模拟、超时控制三种运行模式
- FR-15: 支持多线程并行模拟不同队伍配置
- FR-16: 报错信息包含当前失败指令与上一成功指令的上下文
- FR-17: 数据分析报表支持导出为 PNG 图片（DPS 曲线、伤害占比、技能占比、异常统计、能量曲线）
- FR-18: 提供 PyQt6 桌面 GUI，支持配置模拟参数、启动/停止模拟、实时进度展示、可视化报表查看与历史记录管理

## 5. Non-Goals (Out of Scope)

- 不做坐标/碰撞体积/空间计算（范围判定仅靠近战/远程 Tag）
- 不模拟怪物 AI 行为（敌人仅为沙袋/木人模型）
- 不做网络同步或多人模拟
- 不实现 CSV/TOML 数据加载（首批仅 JSON）
- 不处理敌人对角色造成伤害（仅角色→敌人单向结算）
- 不提供自定义伤害公式的插件机制
- 不做实时 3D 渲染（PyQt6 GUI 为 2D 图表+表单）
- 不支持同一模拟中多波次/多阶段切换 APL（单次模拟仅执行一个 APL 序列）

## 6. Technical Considerations

- **语言与框架：** Python 3.12+ / Pydantic v2 / Blinker（事件信号）/ pytest / PyQt6（桌面 GUI）/ matplotlib（图表生成）
- **现有代码衔接：** GameState 需从 `List[Character]` 迁移到 `TeamManager`；APLManager 的 validator 需替换为 ResourceValidator 实例
- **Blinker 信号隔离：** 多线程下每个线程需使用独立的 `Namespace` 实例，避免信号跨线程污染
- **Pydantic 严格模式：** 全项目统一 `strict=True`，禁止隐式类型转换
- **日志命名空间：** 遵循 `zsim.Module.ClassName` 格式
- **代码风格一致性：** 所有新代码必须遵循现有项目风格——中文 docstring、完整类型标注、Pydantic 模型用于数据结构、依赖注入模式、`@emit_on_error` 切面装饰器、Blinker 发布-订阅通信
- **测试策略：** 单元测试覆盖核心计算逻辑，集成测试覆盖完整流程，Snapshot 测试验证 Seed 复现性

## 7. Success Metrics

- 完成全部 19 个大纲组件的开发并通过测试
- 同一 Seed + 同一配置下两次模拟的 JSONL 日志逐行完全一致
- 模拟运行效率：单线程 ≥ 1000 tick/s（即 16.7x 实时倍速）
- 核心计算公式与游戏内实测数据误差在公式精度范围内
- 数据分析报表覆盖 DPS、伤害占比、技能占比、异常统计、能量曲线五大维度，可导出为 PNG
- PyQt6 桌面 GUI 支持完整的模拟配置→启动→进度监控→报表查看→历史管理流程

## 8. Open Questions

- PyQt6 图表渲染方案：matplotlib FigureCanvas 嵌入 vs 预生成 PNG 用 QLabel 展示？
- 等级区系数的查表值数据来源和具体数值表是否已有参考？
- 极性紊乱的比例系数和附加伤害精通比例系数是否有已知的角色数据？
