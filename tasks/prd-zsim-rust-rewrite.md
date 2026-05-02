# PRD: ZSim 2.0 — Rust 重写与全栈架构升级

## 1. Introduction/Overview

将 ZSim 2.0 战斗模拟系统从纯 Python 单体架构完全重写为 **Rust 核心引擎 + Python 分析 Sidecar + Tauri 桌面前端** 的三层架构。核心目标是支撑 **10 万次独立模拟 × 18,000 tick 的高并发场景**，将亿级行数数据以 Parquet 列式格式高效落盘，并通过 PyO3 扩展模块在 Rust 层完成聚合后交付 Plotly 交互式图表。

**迁移策略**：完全重写（废弃现有 Python 核心代码库）。Rust 负责全部战斗逻辑（模拟引擎、6 大结算模块、BUFF 管理、异常管理、装备系统），Python 退化为轻量 Sidecar 负责数据分析编排，Tauri 作为桌面壳渲染前端。现有 `data/` 目录下 JSON 文件保持向后兼容，Rust 引擎直接读取。

## 2. Goals

- **并行性能**：基于 rayon 实现 100,000 次独立模拟并行执行，线性利用多核 CPU
- **状态隔离**：每个模拟实例拥有完全独立的内存状态机，通过种子差异化保证统计有效性
- **高频处理**：单次模拟支持最高 18,000 tick，极端情况下每 tick 均有事件产生不丢帧
- **存储效率**：全量明细数据写入 Parquet（Zstd 压缩 + 按模拟场次划分 Row Group），禁止 JSON 输出
- **内存安全**：亿级行数数据仅在 Rust 层聚合（均值/方差/分位数），Python 内存中不存在明细数据
- **交互可视化**：Plotly.js 动态图表（缩放/平移/选取），替代静态 PNG
- **向后兼容**：Rust 引擎直接读取现有 `data/` 目录 JSON 文件（characters/enemies/skills/APL）
- **国际化**：Tauri 前端界面支持中/英/日三语切换（`locales/` 目录 JSON 翻译文件，可扩展语种）
- **自包含分发**：最终发布包含所有运行时环境（Rust 二进制 + Python embedded + Tauri bundle）

## 3. User Stories

---

### Phase 1: Rust 核心引擎 — 项目脚手架与数据模型

#### US-001: Rust 项目初始化与模块骨架
**Description:** 作为开发者，需要搭建 Rust workspace，定义 crate 边界和模块依赖关系，确保项目可以编译并运行单元测试。

**Acceptance Criteria:**
- [ ] 创建 Rust workspace，包含以下 crates：
  - `zsim-core`：战斗模拟引擎（entities / combat / calculation / engine）
  - `zsim-parquet`：Parquet 读写与聚合
  - `zsim-pyo3`：PyO3 扩展模块
  - `zsim-cli`：CLI 入口（`cargo run --release -- --config sim.toml`）
- [ ] `Cargo.toml` 声明核心依赖：`rayon`, `serde`, `serde_json`, `parquet` (arrow-rs), `rand`, `rand_chacha`, `pyo3`, `anyhow`, `thiserror`
- [ ] 项目可通过 `cargo build` 和 `cargo test` 编译与测试
- [ ] `.github/workflows/ci.yml` 包含 `cargo test`, `cargo clippy`, `cargo fmt --check`

#### US-002: 实体数据模型层 (entities)
**Description:** 作为模拟引擎的基础，需要将现有 Python `entities/` 模块全部映射为 Rust struct/enum，保持与现有 JSON 数据格式完全兼容。

**Acceptance Criteria:**
- [ ] `BaseStats` struct：12 项战斗属性（ATK/DEF/HP/CRIT_RATE/CRIT_DMG/PEN/PEN_RATIO/ANOMALY_MASTERY/ANOMALY_PROFICIENCY/IMPACT/ENERGY_REGEN/DMG_BONUS），全部 `f64`
- [ ] `Character` struct：identity（id/name/level/ascension）、base_stats、current_stats（含实时修饰器）、resources（HP/Energy/Decibel）、state（on_field/is_alive/stun_timer）
- [ ] `EnemyState` struct：HP/def_val/stun_gauge/stun_threshold/anomaly_bars（Vec<AnomalyGauge>）/resistances/weaknesses, 使用 `#[serde(alias = "def")]` 处理 Python 关键字
- [ ] 枚举类型：`FactionTag`, `SpecialtyTag`, `ElementTag`, `SkillType`, `TriggerType`, `SimMode`
- [ ] 所有 struct 派生 `Serialize + Deserialize + Clone + Debug`
- [ ] 现有 `data/characters/` 和 `data/enemies/` JSON 文件可被正确反序列化
- [ ] `cargo test` 通过

#### US-003: JSON 数据加载器
**Description:** 作为数据入口，Rust 引擎必须直接读取现有 `data/` 目录 JSON 文件（characters/enemies/skills/equipment/APL），保持完全向后兼容。

**Acceptance Criteria:**
- [ ] `DataLoader` struct：`load_characters(path)`, `load_enemies(path)`, `load_skills(path)`, `load_equipment(path)`, `load_apl(path)` 方法
- [ ] `SkillData` struct：action_id, action_type, trigger_type, damage_multipliers（Vec<HitFrame>）, daze_multiplier, anomaly_multiplier, hit_frames, invincible_frames, interruptible_frame, allows_background_completion, is_snapshot, charge_branches, prerequisite_action_id, hp_cost
- [ ] `APLData` struct：tracks（Vec<Track>），每个 Track 含 track_id + actions（Vec<ActionEntry>）
- [ ] 加载失败时返回 `anyhow::Result` 并包含文件路径上下文
- [ ] 现有 `data/` 目录全部 JSON 文件可被成功加载
- [ ] `cargo test` 通过

---

### Phase 2: 战斗结算层

#### US-004: 全局 RNG 管理器
**Description:** 作为模拟复现的保证，实现基于 `rand_chacha` 的固定 Seed 随机数管理器，所有随机操作必须通过该管理器，确保同一 Seed 多次运行结果完全一致。

**Acceptance Criteria:**
- [ ] `RNGManager` struct：封装 `ChaCha8Rng`，存储 `seed: u64` 和 `call_count: u64`
- [ ] 每次调用 `gen_range(min, max)`, `gen_bool(prob)`, `gen_crit(crit_rate)` 递增 `call_count` 并记录调用栈（debug 模式下）
- [ ] 支持 `snapshot()` / `restore()` 快照机制（用于快照类技能）
- [ ] 每个模拟实例独立创建 RNGManager（seed = base_seed + sim_index）
- [ ] 同一种子 10 次独立运行产出完全一致的战斗日志
- [ ] `cargo test` 通过

#### US-005: 六大结算模块
**Description:** 作为战斗数值核心，实现全部 6 大结算模块（纯函数，无状态），与现有 Python `calculation/calculator.py` 逻辑等价。

**Acceptance Criteria:**
- [ ] `RegularMul`：常规伤害倍率结算，公式 `ATK × multiplier × (1 + DMG_BONUS) × crit_factor × def_factor × res_factor × (1 - dmg_reduction)`，`crit_factor` 由 RNG 判定暴击后取值
- [ ] `AnomalyMul`：异常伤害结算，`ATK × anomaly_multiplier × anomaly_proficiency_factor × (1 + anomaly_dmg_bonus)`
- [ ] `StunMul`：失衡值结算，`IMPACT × daze_multiplier × (1 + daze_bonus) × (1 - daze_resistance)`
- [ ] `CalAnomaly`：异常积蓄结算，`anomaly_mastery × anomaly_multiplier × element_factor × (1 - anomaly_resistance)`，达到阈值触发异常状态
- [ ] `CalDisorder`：紊乱结算，当新异常覆盖已有异常时触发，`(old_gauge + new_gauge) × disorder_multiplier`
- [ ] `CalPolarityDisorder`：极性紊乱结算（特殊紊乱变体）
- [ ] 所有函数签名：`fn calc(&self, input: &CalcInput, rng: &mut RNGManager) -> CalcOutput`
- [ ] 数值误差与 Python 版本 ≤ 1e-9（浮点精度差异可接受）
- [ ] `cargo test` 通过

#### US-006: BUFF 管理器
**Description:** 作为战斗状态核心，实现 BUFF 的添加、叠层、刷新、到期移除，支持条件触发和快照机制。

**Acceptance Criteria:**
- [ ] `BuffManager` struct：持有 `HashMap<String, Vec<ActiveBuff>>`（按角色ID索引）
- [ ] `BuffData` struct：buff_id, category（ATK/DEF/CRIT/DMG_BONUS/RES_PEN/ANOMALY/STUN/SPECIAL）, stack_type（Replace/Additive/Independent）, max_stacks, duration_ticks, modifiers（HashMap<StatKey, f64>）
- [ ] `apply_buff(target_id, buff_data, source_id)` — 按 stack_type 处理叠层逻辑
- [ ] `remove_expired(current_tick)` — 移除到期 BUFF
- [ ] `get_effective_modifiers(character_id) -> ModifierSnapshot` — 返回当前所有 BUFF 的合计修饰值
- [ ] 支持 `on_tick` 连接，每 Tick 递减剩余持续帧
- [ ] `cargo test` 通过

#### US-007: 异常与紊乱状态管理器
**Description:** 作为元素异常系统，管理敌人异常积蓄槽、异常触发、异常覆盖和紊乱结算。

**Acceptance Criteria:**
- [ ] `AnomalyDisorderManager` struct
- [ ] 每个敌人维护 `Vec<AnomalyState>`（每个元素一个槽），含 element/gauge/max_gauge/remaining_duration/trigger_count
- [ ] `accumulate(enemy_id, element, amount)` — 积蓄 + 触发判定
- [ ] `trigger_anomaly(enemy_id, element)` — 触发异常（广播事件，设置剩余持续 Tick）
- [ ] `check_disorder(enemy_id, new_element)` — 检测紊乱条件，触发紊乱结算
- [ ] `on_tick(current_tick)` — 递减异常状态剩余持续帧，到期清除
- [ ] `cargo test` 通过

#### US-008: 装备系统
**Description:** 作为角色战力组成，实现音擎（W-Engine）和驱动盘（Drive Disc）套装效果管理。

**Acceptance Criteria:**
- [ ] `EquipmentManager` struct
- [ ] `WEngine` struct：id/name/level/ascension/base_stats（ATK/ATK_RATIO/stat_buff）/passive_effects（条件触发的 BuffData 列表）
- [ ] `DriveDisc` struct：id/slot/level/main_stat/sub_stats（Vec<StatEntry>）
- [ ] `DiscSet` struct：set_id/two_piece_bonus/four_piece_bonus（BuffData）
- [ ] `apply_equipment_buffs(character_id)` — 将音擎 + 驱动盘 + 套装效果注册到 BuffManager
- [ ] 现有 `data/equipment/` JSON 可被正确加载
- [ ] `cargo test` 通过

---

### Phase 3: 战斗引擎

#### US-009: 核心事件总线与信号系统
**Description:** 作为模拟引擎的神经中枢，实现基于 Tokio `broadcast` channel 的发布-订阅事件系统，替代现有 Blinker 信号。

**Acceptance Criteria:**
- [ ] `EventBus` struct：持有 `HashMap<EventType, broadcast::Sender<GameEvent>>`
- [ ] `GameEvent` enum：TickStart / ActionStart / DamageDealt / DamageApplied / BuffChanged / AnomalyTriggered / DisorderTriggered / ChainAttack / CoordinatedAction / CombatEnd / ErrorRaised（共 12 种事件类型）
- [ ] `publish(event_type, payload)` — 广播事件到所有订阅者
- [ ] `subscribe(event_type) -> broadcast::Receiver<GameEvent>` — 返回独立 Receiver
- [ ] 每个 Tick 内同步完成所有事件处理（不跨 Tick 延迟）
- [ ] 模拟结束后所有 Receiver 自动 Drop（无需手动 disconnect）
- [ ] `cargo test` 通过（验证事件广播、多订阅者、Drop 清理）

#### US-010: 游戏状态机
**Description:** 作为全局状态持有者，管理 Tick 计数、队伍引用、运行模式和模拟终止条件。

**Acceptance Criteria:**
- [ ] `GameState` struct：current_tick（u64）, team（TeamManager）, mode（SimMode）, is_terminated（bool）, termination_reason（Option<String>）
- [ ] `advance_tick()` — 递增 current_tick
- [ ] `check_termination()` — 判定条件：①所有敌人死亡 ②所有角色死亡 ③达到 max_tick（18,000）④APL 所有轨道动作已耗尽
- [ ] `terminate(reason)` — 设置 is_terminated = true 并记录原因
- [ ] `cargo test` 通过

#### US-011: 队伍管理器
**Description:** 作为编队管理，协调 1-3 名角色的前后台切换、换人冷却、喧响值（大招能量）和连携技点数。

**Acceptance Criteria:**
- [ ] `TeamManager` struct：characters（Vec<Character>）, bangboo（Option<Character>）, current_on_field_index（usize）
- [ ] `switch_to(index)` — 切换前台角色，触发换人冷却（CD 与 APL 排轴秒数相关）
- [ ] `add_decibel(amount)` / `consume_decibel(amount)` — 喧响值管理（上限 3000/角色，邦布无能量）
- [ ] `add_chain_point()` / `consume_chain_point()` — 连携技点数管理
- [ ] `get_on_field()` / `get_off_field(index)` — 返回前台/后台角色可变引用
- [ ] 邦布使用与角色相同的 SkillAction 模型，但无能量机制
- [ ] `cargo test` 通过（22+ tests 覆盖等价于现有 Python 版本）

#### US-012: 资源校验器
**Description:** 作为动作执行的前置守卫，进行 8 维资源校验（能量/CD/HP/喧响/连携点/换人冷却/异常状态/役职）。

**Acceptance Criteria:**
- [ ] `ResourceValidator` struct：实现 8 个校验方法
- [ ] `validate_energy(character, skill)`, `validate_cooldown(character, skill)`, `validate_hp(character, skill)`, `validate_decibel(character, skill)`, `validate_chain_point(team, skill)`, `validate_switch_cooldown(team)`, `validate_anomaly_state(enemy, skill)`, `validate_role(character, skill)`
- [ ] 校验失败返回 `Result::Err(ValidationError { action_id, missing_resource, current_value, required_value })`（而非返回 false）
- [ ] `validate_all(character, team, enemies, skill) -> Result<(), Vec<ValidationError>>` 聚合全部校验
- [ ] `cargo test` 通过（25+ tests 覆盖等价于现有 Python 版本）

#### US-013: APL 排轴管理器
**Description:** 作为模拟的主要驱动力，逐帧解析 APL 排轴并执行动作，驱动整个战斗流程。

**Acceptance Criteria:**
- [ ] `APLManager` struct：持有 tracks（多角色排轴）、每个 track 的 action 队列指针、ResourceValidator 引用
- [ ] `process_next_action(current_tick, game_state)` — 检查所有 track 的队首动作是否满足执行条件，满足则启动执行
- [ ] 动作执行流程：`validate → deduct_resources → start_skill → advance_frames → trigger_hit_frames → complete_skill`
- [ ] 支持动作前提条件（`prerequisite_action_id` 必须已完成）
- [ ] 支持蓄力分支（`charge_branches`：蓄力时长 → 变体技能）
- [ ] 动作队列消费模式：`get_pending_actions()` 返回时清空内部队列
- [ ] `cargo test` 通过

#### US-014: 协同动作系统
**Description:** 作为后台输出机制，实现后台角色协同攻击和派生动作的触发与管理。

**Acceptance Criteria:**
- [ ] `CoordinatedActionSystem` struct：管理已注册的协同监听器
- [ ] `CoordinatedListener` trait：`on_event(event: &GameEvent, game_state: &GameState) -> Vec<SkillAction>`
- [ ] 支持事件类型：`on_damage_dealt`, `on_anomaly_triggered`, `on_chain_attack`, `on_dodge`, `on_parry`
- [ ] 每个离线角色限每 N 秒触发一次（由内部 CD 控制，按角色独立计时，单位帧 @ 60 tick/s）
- [ ] 派生动作在父动作完成后在同一 Tick 内立即执行
- [ ] `cargo test` 通过（30+ tests 覆盖等价于现有 Python 版本）

#### US-015: 主循环事件循环
**Description:** 作为模拟引擎的核心调度器，驱动逐 Tick 推进的全流程事件循环。

**Acceptance Criteria:**
- [ ] `SimulationRunner` struct：`run(config: SimConfig) -> SimulationResult`
- [ ] 每 Tick 执行顺序：
  1. `on_tick_start` → BuffManager.remove_expired() / AnomalyManager.on_tick() / cooldown 递减
  2. APLManager.process_next_action() → 可能触发 on_action_start
  3. 协同动作检查 → CoordinatedActionSystem.process()
  4. 敌人 AI（如有）→ 敌方动作执行
  5. `check_termination()` → 若终止则退出循环
- [ ] 同一 Tick 内所有事件处理同步完成
- [ ] `cargo test` 通过（验证完整 Tick 循环、终止条件、事件顺序）

---

### Phase 4: 并行执行与数据落盘

#### US-016: Rayon 并行模拟执行器
**Description:** 作为性能核心，基于 rayon 实现 100,000 次独立模拟的并行执行，每个线程拥有完全独立的状态机。

**Acceptance Criteria:**
- [ ] `ParallelRunner` struct：接收 `SimConfig { sim_count, base_seed, max_tick, data_dir }`
- [ ] `rayon::par_iter(0..sim_count).map(|i| { ... })` 并行执行
- [ ] 每次模拟独立创建：RNGManager(seed = base_seed + i), GameState, EventBus, APLManager, BuffManager, AnomalyManager, EquipmentManager
- [ ] 共享只读数据（SkillData/CharacterData/EnemyData 等 JSON 加载数据）使用 `Arc` 跨线程共享
- [ ] 每次模拟返回 `SimResult { sim_index, seed, total_ticks, termination_reason, events: Vec<LoggedEvent> }`
- [ ] 进度回调：每完成 1% 的模拟，通过 `std::sync::mpsc::channel` 发送进度更新
- [ ] `cargo test` 通过（验证 100 次并行结果与单线程一致）

#### US-017: Parquet 列式落盘与压缩
**Description:** 作为数据存储层，将模拟结果写入 Parquet 文件，替代 JSON Lines 输出。

**Acceptance Criteria:**
- [ ] `ParquetWriter` struct：`write_results(results: Vec<SimResult>, output_path: &Path) -> Result<()>`
- [ ] Parquet Schema 定义：

| 列名 | 类型 | 说明 |
|---|---|---|
| `sim_index` | UInt32 | 模拟编号 (0 ~ 99999) |
| `tick` | UInt32 | Tick 序号 |
| `event_type` | Utf8 | 事件类型枚举 |
| `source_id` | Utf8 | 来源角色/敌人 ID |
| `target_id` | Utf8 | 目标角色/敌人 ID |
| `action_id` | Utf8 | 动作标识 |
| `damage` | Float64 | 伤害数值 |
| `crit` | Boolean | 是否暴击 |
| `element` | Utf8 | 元素类型 |
| `anomaly_gauge` | Float64 | 异常积蓄值 |
| `stun_dmg` | Float64 | 失衡值 |
| `buff_id` | Utf8 | BUFF 标识（nullable） |
| `buff_value` | Float64 | BUFF 数值（nullable） |
| `coordinated_flag` | Boolean | 是否协同动作 |
| `timestamp` | UInt64 | 毫秒时间戳（nullable, 用于调试） |

- [ ] Parquet Writer Options：`Compression::Zstd(3)`, `max_row_group_size = 模拟场次行数`（即每个 sim_index 的数据一个 Row Group）
- [ ] 写入 `results.parquet` 文件，支持追加模式（分批写入不覆盖）
- [ ] 极限测试：100,000 次模拟 × 18,000 tick × 每条 tick 平均 10 行 = 18 亿行写入成功且不 OOM
- [ ] `cargo test` 通过

#### US-018: 数据聚合模块 (zsim-parquet)
**Description:** 作为数据分析的 Rust 层，直接读取 Parquet 文件进行聚合计算，返回降维后的统计数据。

**Acceptance Criteria:**
- [ ] `ParquetAggregator` struct：`aggregate(parquet_path: &Path, query: AggQuery) -> AggResult`
- [ ] `AggQuery` enum 支持以下聚合类型：
  - `TotalDamage { group_by: Option<SimIndex | SourceId | Element | Tick> }`
  - `DPS { window_ticks: u64 }` — 滑动窗口 DPS 曲线
  - `DamageBreakdown { group_by: BreakdownBy::SourceId }` — 伤害占比
  - `AnomalyStats { group_by: Element }` — 异常触发统计
  - `StunStats` — 失衡统计
  - `CritRate { source_id: String }` — 暴击率统计
  - `StatsSummary { field: Damage | StunDmg | AnomalyGauge }` — 返回 `{ mean, variance, std_dev, p50, p90, p95, p99, min, max, count }`
- [ ] 聚合在 Rust 层完成，Python 侧仅接收聚合后的 `AggResult`（JSON-serializable）
- [ ] 使用 Arrow/Parquet 的列式读取 + `filter` pushdown 优化（只读需要的列/行组）
- [ ] 1,000 万行数据聚合时间 ≤ 2 秒（SSD 冷读条件）
- [ ] `cargo test` 通过

---

### Phase 5: PyO3 扩展与 Python Sidecar

#### US-019: PyO3 扩展模块
**Description:** 作为 Python 与 Rust 的桥梁，将 `zsim-parquet` 的聚合能力暴露为 Python 可调用的扩展模块。

**Acceptance Criteria:**
- [ ] `zsim_pyo3` crate：使用 `pyo3` 导出 Python 模块 `zsim_rs`
- [ ] 导出函数签名：
  ```python
  import zsim_rs

  # 聚合查询
  result_json: str = zsim_rs.aggregate(parquet_path: str, query_json: str)

  # 直接获取统计摘要
  summary_json: str = zsim_rs.summary(parquet_path: str)

  # 获取 DPS 曲线数据
  dps_json: str = zsim_rs.dps_curve(parquet_path: str, window_ticks: int = 60)

  # 获取伤害占比数据
  breakdown_json: str = zsim_rs.damage_breakdown(parquet_path: str)
  ```
- [ ] 所有函数返回 JSON 字符串（不返回 Python 对象，避免内存拷贝）
- [ ] 编译产物为 `.pyd`（Windows）/ `.so`（Linux）/ `.dylib`（macOS）
- [ ] Wheel 包可通过 `pip install zsim_rs` 安装
- [ ] `maturin develop` 可在开发模式下构建

#### US-020: Python 分析 Sidecar
**Description:** 作为数据分析编排层，Python Sidecar 负责接收分析请求，通过 PyO3 扩展读取 Parquet 并聚合，生成 Plotly 图表配置 JSON 通过 stdout 输出。

**Acceptance Criteria:**
- [ ] `analysis_sidecar.py`：常驻进程，通过 stdin 接收 JSON 命令，stdout 输出 JSON 结果
- [ ] 命令协议：
  ```json
  // Request (stdin)
  {"cmd": "dps_curve", "parquet_path": "...", "params": {"window_ticks": 60}}
  {"cmd": "damage_breakdown", "parquet_path": "..."}
  {"cmd": "summary", "parquet_path": "..."}
  {"cmd": "anomaly_timeline", "parquet_path": "..."}

  // Response (stdout)
  {"type": "chart", "id": "dps_curve", "data": [...], "layout": {...}}
  {"type": "summary", "data": {...}}
  {"type": "error", "message": "..."}
  {"type": "ready"}
  ```
- [ ] 图表配置符合 Plotly.js `Plotly.newPlot()` 参数格式
- [ ] Sidecar 启动时输出 `{"type": "ready"}` 表示就绪
- [ ] Sidecar 出错时输出 `{"type": "error", "message": "..."}` 不崩溃
- [ ] Sidecar 进程生命周期由 Tauri 管理（spawn on start, kill on exit）
- [ ] `python analysis_sidecar.py` 可独立测试

---

### Phase 6: Tauri 桌面前端

#### US-021: Tauri 项目脚手架与 IPC
**Description:** 作为桌面壳，Tauri 管理前端窗口、Python Sidecar 进程和文件系统交互。

**Acceptance Criteria:**
- [ ] Tauri v2 项目初始化（Rust 后端 + Vanilla JS/CSS 前端）
- [ ] `tauri.conf.json` 配置窗口尺寸（1280×800，可调整）、标题（ZSim Analyzer）
- [ ] `src-tauri/src/main.rs` 注册以下命令：
  - `run_simulation(config_json: String)` — 调用 Rust 模拟引擎（嵌入）
  - `spawn_sidecar()` — 启动 Python Sidecar 子进程
  - `send_to_sidecar(cmd_json: String)` — 向 Sidecar stdin 写入命令
- [ ] Sidecar 进程使用 `tauri::api::process::Command::new_sidecar()` 管理
- [ ] Sidecar 通过 `Command::sidecar("python")` 或 `python analysis_sidecar.py` 启动

#### US-022: 前端 UI — 模拟配置面板
**Description:** 作为用户交互入口，提供模拟参数配置界面和启动/停止控制。

**Acceptance Criteria:**
- [ ] 配置面板包含：
  - 模拟次数（数值输入，默认 100000）
  - 最大 Tick（数值输入，默认 18000）
  - 基种子（数值输入，默认随机 0-2^32）
  - 数据目录（文件选择器，默认 `./data/`）
  - APL 文件（文件选择器）
  - 输出路径（文件选择器，默认 `./results.parquet`）
- [ ] "开始模拟" 按钮：调用 `run_simulation` 命令，显示进度条 + 完成百分比
- [ ] "停止" 按钮：终止模拟进程
- [ ] 进度条实时更新（通过 Tauri event `simulation-progress`）
- [ ] 模拟完成时弹出通知并自动加载分析视图
- [ ] `cargo tauri dev` 后手动验证 UI 交互完整

#### US-023: 前端 UI — Plotly.js 交互式图表
**Description:** 作为数据可视化核心，渲染 DPS 曲线、伤害占比饼图、异常时间线和统计摘要。

**Acceptance Criteria:**
- [ ] 引入 Plotly.js（CDN 或 npm bundle）
- [ ] 四个图表区域（支持切换显示/隐藏）：
  - **DPS 曲线**（折线图，X=Tick, Y=DPS）：支持缩放和平移，可叠加多条曲线（不同角色）
  - **伤害占比**（饼图/环形图）：按角色/元素/动作类型分组（切换按钮）
  - **异常时间线**（堆叠面积图，X=Tick, Y=异常积蓄值）：按元素着色
  - **统计摘要**（卡片面板）：均值/方差/中位数/P50/P90/P95/P99/最小值/最大值
- [ ] 图表数据通过 Sidecar JSON 响应填充
- [ ] 所有图表支持：缩放（滚轮）、平移（拖拽）、选取（框选）、悬停详情、图例切换
- [ ] `cargo tauri dev` 后手动验证所有图表可交互

#### US-024: 前端 UI — 数据导出与共享
**Description:** 作为分析结果出口，支持导出图表为 HTML 文件和导出统计数据为 CSV。

**Acceptance Criteria:**
- [ ] "导出 HTML" 按钮：将当前图表导出为独立 HTML 文件（内嵌 Plotly.js CDN 引用 + JSON 数据）
- [ ] "导出 CSV" 按钮：将当前统计摘要导出为 CSV 文件
- [ ] 导出使用 Tauri `save_file` 对话框选择保存路径
- [ ] `cargo tauri dev` 后手动验证导出功能

#### US-024a: 前端 UI — 国际化 (i18n)
**Description:** 作为面向多语言用户的应用，Tauri 前端必须支持中/英/日三语界面切换，并提供可扩展的翻译文件结构。

**Acceptance Criteria:**
- [ ] 创建 `src/locales/zh-CN.json`, `en-US.json`, `ja-JP.json` 翻译文件
- [ ] 每个 JSON 文件为扁平 key-value 结构：`{"menu.file": "文件", "menu.simulate": "开始模拟", ...}`
- [ ] 实现 `i18n.t(key)` 翻译函数，根据当前语言设置返回对应文本
- [ ] 语言切换下拉菜单置于窗口右上角（国旗图标 + 语言名）
- [ ] 语言偏好持久化到 Tauri store（`localStorage`），下次启动自动恢复
- [ ] 翻译覆盖所有 UI 文本：菜单栏、配置面板标签、按钮文本、图表标题、统计表头、错误提示
- [ ] 未翻译 key 回退显示 `en-US`，`en-US` 也不存在时显示 key 名本身
- [ ] 新增语种仅需添加对应 JSON 文件，无需修改 JS 代码
- [ ] `cargo tauri dev` 后手动验证三语切换完整

---

### Phase 7: 集成与分发

#### US-025: 端到端集成测试
**Description:** 作为质量保证，实现从模拟配置到图表渲染的完整流程自动化测试。

**Acceptance Criteria:**
- [ ] Rust 端到端测试：`cargo test --test e2e` 包含
  - 加载 `data/apl/sample_apl.json` 执行 10 次模拟
  - 验证 `results.parquet` 生成且可被正确读取
  - 验证聚合查询返回合理的统计值
- [ ] Python 集成测试：验证 PyO3 模块可被导入且 `aggregate()` 返回正确 JSON
- [ ] Python 集成测试：验证 Sidecar stdin/stdout 协议正常通信
- [ ] Seed 复现测试：同一种子 10 次独立模拟产生完全相同的 Parquet 文件（hash 校验）
- [ ] 所有测试在 CI 中通过

#### US-026: 发布打包
**Description:** 作为面向终端用户的交付物，生成自包含的桌面安装包，包含所有运行时环境。

**Acceptance Criteria:**
- [ ] `cargo tauri build` 生成：
  - Windows：`.msi` 安装包
  - macOS：`.dmg` 镜像
  - Linux：`.AppImage` 和 `.deb`
- [ ] 安装包包含：
  - Rust 模拟引擎二进制
  - Python embedded runtime + `zsim_rs` .pyd/.so
  - `analysis_sidecar.py` 及依赖
  - `data/` 示例数据目录
- [ ] 安装后无需额外配置即可运行（自包含）
- [ ] 安装包大小 ≤ 200MB
- [ ] 流式处理验证：100,000 次模拟 × 18,000 tick 内存峰值 ≤ 2GB

---

## 4. Functional Requirements

### FR-CORE: 核心模拟引擎
- **FR-1**: 系统必须基于 rayon 库实现并行模拟，默认执行 100,000 次独立模拟
- **FR-2**: 每个模拟实例必须拥有完全独立的内存状态机，通过 `seed = base_seed + sim_index` 保证统计差异
- **FR-3**: 系统必须支持单次模拟最高 18,000 tick 推演，每 tick 最多 100 个事件的极端负载
- **FR-4**: 模拟引擎必须是纯 Rust 实现，无 Python 依赖（除 Sidecar 分析）

### FR-DATA: 数据存储
- **FR-5**: 模拟结果必须写入 Parquet 文件，严禁输出 JSON Lines 日志
- **FR-6**: Parquet 必须启用 Zstd 压缩（compression level 3）
- **FR-7**: Parquet 必须按 `sim_index` 划分 Row Group
- **FR-8**: Parquet Schema 必须包含 sim_index/tick/event_type/source_id/target_id/action_id/damage/crit/element/anomaly_gauge/stun_dmg/buff_id/buff_value/coordinated_flag/timestamp 全部 15 列

### FR-ANALYZE: 数据分析
- **FR-9**: PyO3 扩展模块必须提供 `aggregate()`, `summary()`, `dps_curve()`, `damage_breakdown()` 四个导出函数
- **FR-10**: 数据聚合必须在 Rust 层完成，明细数据严禁加载到 Python 内存
- **FR-11**: Python Sidecar 必须通过 stdin/stdout JSON 协议与 Tauri 通信
- **FR-12**: PyO3 模块必须返回 JSON 字符串，不返回 Python 原生对象

### FR-UI: 前端交互
- **FR-13**: 前端必须使用 Plotly.js 渲染动态图表（支持缩放、平移、选取、悬停详情）
- **FR-14**: 必须包含 DPS 曲线、伤害占比、异常时间线、统计摘要四个图表区域
- **FR-15**: 必须提供模拟配置面板（模拟次数、最大 Tick、Seed、数据路径、APL 选择、输出路径）
- **FR-16**: 必须提供实时进度条和模拟终止按钮
- **FR-17**: 必须支持导出图表为 HTML 文件和导出统计为 CSV 文件
- **FR-18**: 前端必须支持中/英/日三语切换，翻译文件为 `locales/` 目录下 JSON，新增语种仅需添加对应文件

### FR-COMPAT: 向后兼容
- **FR-19**: Rust 引擎必须直接读取现有 `data/` 目录 JSON 文件（characters/enemies/skills/equipment/APL）
- **FR-20**: 6 大结算模块数值误差与 Python 版本 ≤ 1e-9

### FR-DEPLOY: 部署分发
- **FR-21**: 最终发布包必须自包含（Rust 二进制 + Python embedded + Tauri bundle），无需用户额外安装环境
- **FR-22**: 流式处理确保 100,000 次模拟 × 18,000 tick 内存峰值 ≤ 2GB

---

## 5. Non-Goals (Out of Scope)

- **不保留**现有 Python 核心代码库（`core_control/`, `entities/`, `combat/`, `calculation/`, `parallel/`），这些由 Rust 完全替代
- **不保留**现有 `data_io/structured_logger.py`（JSON Lines 日志），由 Parquet 完全替代
- **不保留**现有 PyQt6 GUI，由 Tauri + Plotly.js 完全替代
- **不保留**Blinker 信号库依赖，由 Tokio broadcast channel 替代
- **不支持**非 Windows/macOS/Linux 之外的平台（不包含 Web 版）
- **不实现**敌人 AI 系统（仅支持预设排轴或随机行动模板）
- **不实现**多人联机或网络对战
- **不实现**实时游戏引擎（60 tick/s 为逻辑帧，不做渲染帧）
- **不实现**数据库持久化（仅 Parquet 文件存储，不接入 SQL 数据库）

---

## 6. Architecture Design

### 6.1 系统架构图

```
┌─────────────────────────────────────────────────────────────┐
│                      Tauri Desktop App                       │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                  Frontend (HTML/JS/CSS)                 │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌───────┐ │  │
│  │  │ Config   │  │ DPS      │  │ Damage   │  │ Stats │ │  │
│  │  │ Panel    │  │ Curve    │  │ Pie      │  │ Cards  │ │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └───────┘ │  │
│  │              Plotly.js  ←── JSON ──→  stdin/stdout     │  │
│  └───────────────────────────────────────────────────────┘  │
│                          │ Tauri Commands                     │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Tauri Rust Backend                         │  │
│  │  • run_simulation()    (embedded Rust engine)           │  │
│  │  • spawn_sidecar()     (Python child process)           │  │
│  │  • send_to_sidecar()   (stdin JSON)                     │  │
│  │  • read_sidecar()      (stdout JSON)                    │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
        │                                               │
        ▼                                               ▼
┌──────────────────────┐          ┌──────────────────────────┐
│   zsim-core (Rust)    │          │  Python Analysis Sidecar  │
│                       │          │                           │
│  • SimulationRunner   │          │  analysis_sidecar.py      │
│  • ParallelRunner     │          │       │                   │
│  • GameState          │          │       ▼                   │
│  • EventBus           │    ──▶   │  zsim_rs (PyO3)           │
│  • APLManager         │  写文件  │       │                   │
│  • Calculation (x6)   │          │       ▼                   │
│  • BuffManager        │          │  zsim-parquet (Rust)      │
│  • AnomalyManager     │          │  • ParquetAggregator      │
│  • EquipmentManager   │          │  • StatsSummary           │
│  • RNGManager         │          │  • DpsCurve               │
└──────────┬───────────┘          └──────────────────────────┘
           │
           ▼
┌──────────────────────┐
│   data/ (JSON files)  │
│  • characters/        │
│  • enemies/           │
│  • skills/            │
│  • equipment/         │
│  • apl/               │
└──────────────────────┘     ┌──────────────────────┐
                             │  results.parquet      │
                             │  (Zstd, Row Groups)   │
                             └──────────────────────┘
```

### 6.2 Rust Crate 依赖关系

```
zsim-core  ──→  zsim-parquet  ──→  zsim-pyo3
   │                                    │
   └──→  zsim-cli                       │
                                        │
                          Python Sidecar imports zsim_rs
```

- `zsim-core`：战斗模拟引擎，依赖 `serde`, `serde_json`, `rand`, `rand_chacha`, `rayon`, `tokio`, `anyhow`, `thiserror`
- `zsim-parquet`：Parquet 读写与聚合，依赖 `arrow`, `parquet`, `zsim-core`（仅为类型引用）
- `zsim-pyo3`：PyO3 绑定，依赖 `zsim-parquet`, `pyo3`, `serde_json`
- `zsim-cli`：CLI 入口，依赖 `zsim-core`, `zsim-parquet`, `clap`, `indicatif`（进度条）

### 6.3 Python Sidecar IPC 协议

**通信模型**：一行一个 JSON 对象，通过 stdin/stdout 传输。

```
Tauri Frontend ──(invoke)──→ Tauri Backend ──(stdin)──→ Python Sidecar
                                     ↑                       │
                                     └──(stdout JSON)────────┘
```

**Request 格式**（Tauri → Sidecar，stdin）：
```json
{"id": "req-001", "cmd": "summary", "parquet_path": "/path/to/results.parquet"}
{"id": "req-002", "cmd": "dps_curve", "parquet_path": "/path/to/results.parquet", "params": {"window_ticks": 60, "group_by": "source_id"}}
{"id": "req-003", "cmd": "damage_breakdown", "parquet_path": "/path/to/results.parquet", "params": {"group_by": "source_id"}}
{"id": "req-004", "cmd": "anomaly_timeline", "parquet_path": "/path/to/results.parquet"}
{"id": "req-005", "cmd": "shutdown"}
```

**Response 格式**（Sidecar → Tauri，stdout）：
```json
{"id": "req-001", "type": "summary", "data": {"mean": 1234.5, "variance": 567.8, "p50": 1200, "p95": 1800, ...}}
{"id": "req-002", "type": "chart", "chart_type": "dps_curve", "data": [{"name": "Anson", "x": [0,1,...], "y": [0,500,...]}], "layout": {"title": "DPS Curve", "xaxis": {"title": "Tick"}, "yaxis": {"title": "DPS"}}}
{"id": "req-003", "type": "chart", "chart_type": "pie", "data": [{"labels": ["Anson", "Billy"], "values": [65000, 35000]}], "layout": {"title": "Damage Breakdown"}}
{"id": "req-004", "type": "error", "message": "Parquet file not found: /path/to/results.parquet"}
```

### 6.4 Parquet Row Group 策略

```text
results.parquet
├── RowGroup[0]  ← sim_index = 0   (全部 tick 事件)
├── RowGroup[1]  ← sim_index = 1
├── ...
└── RowGroup[N]  ← sim_index = 99999
```

- 每个 `sim_index` 一个 Row Group = 每个模拟场次的数据物理相邻
- 聚合查询时可按 Row Group 统计 `mean/std_dev`（部分查询可只读元数据不读明细）
- Row Group 级别的 stats（min/max/null_count）由 Parquet 自动维护
- Zstd level 3 压缩：在压缩率与速度间取平衡点（典型压缩比 5-10x）

---

## 7. Technical Considerations

### 7.1 内存安全策略

- **模拟内存隔离**：每个 rayon 任务在独立栈上执行，模拟结束后状态机自动 Drop
- **共享只读数据**：CharacterData/SkillData/EnemyData 通过 `Arc` 共享，无锁访问
- **Parquet 流式写入**：使用 Arrow RecordBatch 分批写入，避免全量数据在内存中组装
- **聚合内存上限**：PyO3 聚合层仅保留聚合中间结果（不超过 1MB），原始行数据在 Parquet reader 层被过滤/投影后立即丢弃

### 7.2 线程模型

```rust
// 主线程：UI + Sidecar 通信
// 工作线程池（rayon）：模拟执行
rayon::ThreadPoolBuilder::new()
    .num_threads(num_cpus::get())
    .build()
    .install(|| {
        (0..sim_count).into_par_iter().map(|i| run_single_sim(i, &shared_data))
    })
```

- rayon 默认使用 work-stealing 调度，负载均衡自动处理
- 单个模拟执行是 CPU-bound（无 I/O），适合 rayon 的并行模型
- 进度报告通过 `mpsc::channel` 发送到主线程（非阻塞）

### 7.3 错误处理策略

- Rust 层：`anyhow::Result` 用于应用层，`thiserror` 定义自定义错误类型
- PyO3 层：Rust `Err` 转换为 Python `RuntimeError`，携带错误消息
- Sidecar 层：捕获所有异常，输出 `{"type": "error", "message": "..."}` 不崩溃
- 模拟层：单次模拟失败不影响其他并行任务（`rayon::map` 返回 `Result`）

### 7.4 性能关键路径

| 路径 | 目标 | 策略 |
|---|---|---|
| 单 Tick 结算 | ≤ 0.1ms | 纯函数 + 栈分配 + 无锁 |
| 100,000 次模拟 | ≤ 60s (16核) | rayon 并行 + Arc 共享数据 |
| Parquet 写入 1M 行 | ≤ 1s | 列式编码 + Zstd 流式压缩 |
| 聚合 1M 行 | ≤ 0.2s | 列投影 + filter pushdown |
| PyO3 往返 | ≤ 1ms | JSON 序列化/反序列化 |

### 7.5 浮点确定性

- `ChaCha8Rng` 保证跨平台确定性（不受 CPU 差异影响）
- 所有结算使用 `f64`（IEEE 754），在同一二进制编译下结果一致
- 禁止使用 `HashMap` 遍历（顺序不确定）— 需要顺序一致时使用 `BTreeMap`
- rayon `par_iter` 结果按 `sim_index` 排序后再写入 Parquet

---

## 8. Success Metrics

| 指标 | 当前 (Python) | 目标 (Rust) | 测量方法 |
|---|---|---|---|
| 100,000 次模拟耗时 | N/A (不可行) | ≤ 60s (16核) | `cargo bench` |
| 内存峰值 (100k sims) | N/A | ≤ 2GB | `heaptrack` / 任务管理器 |
| Parquet 文件大小 (100k sims) | N/A (JSON ~50GB) | ≤ 5GB | `du -h results.parquet` |
| 聚合 10M 行延迟 | N/A | ≤ 2s | Sidecar 计时日志 |
| 图表渲染首屏 | N/A | ≤ 3s | 浏览器 Performance API |
| Seed 复现一致性 | ✓ (已验证) | ✓ (维持) | hash(results.parquet) 比对 |
| 打包体积 | N/A | ≤ 200MB | `tauri build` 产物大小 |
| 数值精度差异 | — | ≤ 1e-9 | 对比测试 Python vs Rust |

---

## 9. Open Questions

1. **敌人 AI 行为**：当前 Non-Goals 不包括敌人 AI，但若后续需要，应定义为"被动响应式"还是"主动排轴式"？
2. **中间结果缓存**：100,000 次模拟的中间结果是否需要在 Parquet 写完前保留在内存？（当前设计：分批写入 RecordBatch，写完即释放）
3. **Sidecar 进程崩溃恢复**：若 Python Sidecar 意外退出，Tauri 是否自动重启 Sidecar 并通知前端重试？
4. **翻译管理流程**：`locales/` JSON 翻译文件如何与翻译团队协作？是否需要接入 Lokalise/Crowdin 等翻译管理平台？（当前：开发者手动编辑 JSON，后续可接入）
5. **Parquet Schema 演进**：后续新增事件类型是否要求向后兼容旧 Parquet 文件？（建议：使用 `event_type: Utf8` 枚举字符串，天然兼容新增类型）
6. **GPU 加速**：结算模块是否考虑 GPU 加速（CUDA/Vulkan compute shader）？（当前 10 万次在 CPU 上 60s 内完成暂不需要，但架构上结算纯函数设计便于未来迁移）
