# PRD: ZSim 数据录入前端

## 1. 概述

### 1.1 目标
为 ZSim 战斗模拟器构建一个**独立的 Tauri 桌面数据录入界面**，实现对角色、技能、装备、敌人等游戏数据的**可视化增删改查（CRUD）**。录入数据通过 SQLite 持久化存储，与现有模拟引擎解耦但数据结构兼容。

### 1.2 范围
- 在现有 Tauri v2 项目中新增 SQLite 存储层
- 新增 4 个前端录入视图（角色、技能、装备、敌人）
- 新增 Tauri Rust 命令提供 CRUD API
- 支持对数据库中已有数据的实时编辑与删除
- **改造 `zsim-core/src/data/loader.rs`**：将 `DataLoader` 从读取 JSON 文件改为读取 `zsim.db`（SQLite），使模拟引擎直接消费数据库中的数据
- 保留 JSON 文件作为可选的数据交换格式（导入/导出），但运行时数据源从 JSON 迁移至 SQLite
- 不修改现有模拟引擎核心计算逻辑（zsim-core 的 combat/calculation/events 模块）和 Parquet 分析管线

### 1.3 现有架构约束
- 项目为 Rust workspace，包含 `zsim-core`、`zsim-parquet`、`zsim-pyo3`、`zsim-cli`、`src-tauri`
- 现有前端为 Vanilla JS + Vite 6，通过 `@tauri-apps/api` 调用 Rust 命令
- 现有游戏数据存储为 `data/` 目录下的 JSON 文件（导入后迁移至 SQLite）
- 新录入界面应复用现有前端技术栈，保持架构一致性
- 代码风格与现有其他代码一致。

---

## 2. 技术方案

### 2.1 架构总览

```
┌──────────────────────────────────────────────────┐
│                  Tauri 桌面窗口                     │
│  ┌────────────────────────────────────────────┐   │
│  │  前端 Web 界面 (Vanilla JS + Vite)           │   │
│  │  ┌─────────┐ ┌──────────┐ ┌───────────┐   │   │
│  │  │角色录入   │ │技能编辑器 │ │装备管理    │   │   │
│  │  ├─────────┤ ├──────────┤ ├───────────┤   │   │
│  │  │敌人配置   │ │APL 管理  │ │数据总览    │   │   │
│  │  └─────────┘ └──────────┘ └───────────┘   │   │
│  └──────────────────┬─────────────────────────┘   │
│                     │ invoke()                      │
│  ┌──────────────────▼─────────────────────────┐   │
│  │    Tauri Rust Backend (src-tauri)           │   │
│  │  ┌─────────────────────────────────────┐    │   │
│  │  │  数据录入模块 (data_entry mod)        │    │   │
│  │  │  - CRUD 命令 (10+ 个 Tauri 命令)      │    │   │
│  │  │  - SQLite 连接池管理                  │    │   │
│  │  │  - JSON ↔ SQL 映射层                 │    │   │
│  │  └──────────────┬──────────────────────┘    │   │
│  │                 │ rusqlite                   │   │
│  │  ┌──────────────▼──────────────────────┐    │   │
│  │  │        zsim.db (SQLite)              │    │   │
│  │  └─────────────────────────────────────┘    │   │
│  └─────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────┘
```

### 2.2 存储层：SQLite

在 `src-tauri` 中引入 `rusqlite` 依赖，使用 `zsim.db` 作为本地数据库文件。

**数据库位置策略**：
- 开发环境：项目根目录下的 `data/zsim.db`
- 生产环境：`tauri::api::path::app_data_dir()` 返回的应用数据目录
- 提供"从 JSON 导入"功能，将现有 `data/` 下的 JSON 文件导入 SQLite

### 2.3 数据库 Schema

```sql
-- =============================================
-- 角色表
-- =============================================
CREATE TABLE characters (
    char_id         TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    faction         TEXT NOT NULL,       -- Gentle_House / etc.
    specialty       TEXT NOT NULL,       -- Attack / Stun / Support / Rupture / Anomaly / Defense
    element         TEXT NOT NULL,       -- Ice / Fire / Ether / Physical / Electric / ...
    level           INTEGER NOT NULL DEFAULT 60,
    ascension       INTEGER NOT NULL DEFAULT 6,
    hp              REAL NOT NULL DEFAULT 0,
    atk             REAL NOT NULL DEFAULT 0,
    def             REAL NOT NULL DEFAULT 0,
    impact          REAL NOT NULL DEFAULT 0,
    crit_rate       REAL NOT NULL DEFAULT 0,
    crit_dmg        REAL NOT NULL DEFAULT 0,
    pen_ratio       REAL NOT NULL DEFAULT 0,
    pen_fixed       REAL NOT NULL DEFAULT 0,
    anomaly_mastery       REAL NOT NULL DEFAULT 0,
    anomaly_proficiency    REAL NOT NULL DEFAULT 0,
    energy_regen    REAL NOT NULL DEFAULT 0,
    energy_gen_rate REAL NOT NULL DEFAULT 0,
    constellations  TEXT NOT NULL DEFAULT '[false,false,false,false,false,false]',  -- JSON array
    action_dict     TEXT NOT NULL DEFAULT '[]',  -- JSON array of action IDs
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- =============================================
-- 技能表 (一对多: 角色 → 技能)
-- =============================================
CREATE TABLE skills (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    char_id     TEXT NOT NULL REFERENCES characters(char_id) ON DELETE CASCADE,
    action_id   TEXT NOT NULL,
    action_type TEXT NOT NULL,   -- Normal / Special / Ultimate / Dodge / Chain / Assist / ...
    daze_multiplier     REAL NOT NULL DEFAULT 0,
    energy_cost         REAL NOT NULL DEFAULT 0,
    decibel_cost        REAL NOT NULL DEFAULT 0,
    hp_cost             REAL NOT NULL DEFAULT 0,
    cooldown_ticks      INTEGER NOT NULL DEFAULT 0,
    animation_frames    INTEGER NOT NULL DEFAULT 0,
    interruptible_frame INTEGER,
    is_snapshot         INTEGER NOT NULL DEFAULT 0,  -- boolean
    prerequisite_action_id TEXT,                     -- nullable
    effect_id           TEXT,                        -- 逻辑占位符 (如 buff_001)
    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(char_id, action_id)
);

-- =============================================
-- 伤害倍率表 (一对多: 技能 → 倍率段)
-- =============================================
CREATE TABLE skill_multipliers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    skill_id    INTEGER NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    segment_index INTEGER NOT NULL,  -- 段数序号
    frame       INTEGER NOT NULL,    -- 命中帧
    multiplier  REAL NOT NULL,       -- 倍率值
    decay_coeff REAL DEFAULT 1.0     -- 衰减系数
);

-- =============================================
-- 音擎 (W-Engine) 表
-- =============================================
CREATE TABLE w_engines (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    level           INTEGER NOT NULL DEFAULT 60,
    ascension       INTEGER NOT NULL DEFAULT 6,
    atk             REAL NOT NULL DEFAULT 0,
    crit_rate       REAL DEFAULT 0,
    crit_dmg        REAL DEFAULT 0,
    pen_ratio       REAL DEFAULT 0,
    energy_regen    REAL DEFAULT 0,
    impact          REAL DEFAULT 0,
    anomaly_mastery REAL DEFAULT 0,
    passive_effects TEXT NOT NULL DEFAULT '[]',  -- JSON array of effect IDs
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- =============================================
-- 驱动盘表
-- =============================================
CREATE TABLE drive_discs (
    id          TEXT PRIMARY KEY,
    slot        INTEGER NOT NULL CHECK(slot BETWEEN 1 AND 6),
    level       INTEGER NOT NULL DEFAULT 15,
    set_id      TEXT NOT NULL,
    main_stat_name  TEXT NOT NULL,
    main_stat_value REAL NOT NULL,
    sub_stat_1_name  TEXT,
    sub_stat_1_value REAL,
    sub_stat_2_name  TEXT,
    sub_stat_2_value REAL,
    sub_stat_3_name  TEXT,
    sub_stat_3_value REAL,
    sub_stat_4_name  TEXT,
    sub_stat_4_value REAL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- =============================================
-- 驱动盘套装表
-- =============================================
CREATE TABLE disc_sets (
    set_id      TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    two_piece_description TEXT,
    two_piece_buff_id     TEXT,  -- 效果 ID
    four_piece_description TEXT,
    four_piece_buff_id     TEXT,  -- 效果 ID
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- =============================================
-- 敌人表
-- =============================================
CREATE TABLE enemies (
    enemy_id    TEXT PRIMARY KEY,
    enemy_type  TEXT NOT NULL DEFAULT 'Normal',  -- Normal / Elite / Boss
    name        TEXT NOT NULL DEFAULT '',
    level       INTEGER NOT NULL DEFAULT 60,
    hp          REAL NOT NULL DEFAULT 0,
    def         REAL NOT NULL DEFAULT 0,
    base_res    REAL NOT NULL DEFAULT 0,       -- 最终减伤系数
    daze_max    REAL NOT NULL DEFAULT 0,
    resistances TEXT NOT NULL DEFAULT '{}',    -- JSON map: { "Ice": 0.4, "Ether": 0.6 }
    weaknesses  TEXT NOT NULL DEFAULT '[]',    -- JSON array: ["Fire", "Physical"]
    anomaly_buildup TEXT NOT NULL DEFAULT '{}',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### 2.4 依赖变更

**`src-tauri/Cargo.toml` 新增依赖**:
```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }   # SQLite (bundled = 无需系统安装)
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**`package.json` 不变** — 前端继续使用 Vanilla JS + Vite。

---

## 3. Tauri Rust 命令设计

### 3.1 数据库初始化与 JSON 导入

```rust
// 初始化数据库 (建表 + 从 data/ 目录导入 JSON)
#[tauri::command]
fn init_database(app: AppHandle) -> Result<String, String>;
// 返回: { "status": "ok", "tables": ["characters",...], "imported": { "characters": 1, ... } }

// 从 JSON 文件导入指定类型
#[tauri::command]
fn import_from_json(app: AppHandle, data_type: String, file_path: String) -> Result<String, String>;
// data_type: "character" | "skills" | "equipment" | "enemy" | "apl"

// 清空并重新导入所有数据
#[tauri::command]
fn reimport_all(app: AppHandle) -> Result<String, String>;
```

### 3.2 角色 CRUD

```rust
// 获取所有角色 (列表)
#[tauri::command]
fn get_characters(app: AppHandle) -> Result<String, String>;
// 返回 JSON 数组

// 获取单个角色 (含关联技能)
#[tauri::command]
fn get_character(app: AppHandle, char_id: String) -> Result<String, String>;
// 返回: { character: {...}, skills: [...] }

// 创建或更新角色
#[tauri::command]
fn save_character(app: AppHandle, data: String) -> Result<String, String>;
// data: JSON string 匹配 characters 表结构 (不含 created_at/updated_at)

// 删除角色 (级联删除关联技能、倍率)
#[tauri::command]
fn delete_character(app: AppHandle, char_id: String) -> Result<String, String>;
```

### 3.3 技能 CRUD

```rust
// 获取指定角色的所有技能
#[tauri::command]
fn get_skills(app: AppHandle, char_id: String) -> Result<String, String>;
// 返回: [{ skill 字段..., multipliers: [...] }, ...]

// 保存技能 (含倍率段)
#[tauri::command]
fn save_skill(app: AppHandle, data: String) -> Result<String, String>;
// data 包含 skill 字段 + multipliers 数组

// 删除技能
#[tauri::command]
fn delete_skill(app: AppHandle, skill_id: i64) -> Result<String, String>;
```

### 3.4 装备 CRUD

```rust
// 获取所有音擎/驱动盘/套装
#[tauri::command]
fn get_w_engines(app: AppHandle) -> Result<String, String>;
#[tauri::command]
fn get_drive_discs(app: AppHandle) -> Result<String, String>;
#[tauri::command]
fn get_disc_sets(app: AppHandle) -> Result<String, String>;
#[tauri::command]
fn get_all_equipment(app: AppHandle) -> Result<String, String>;
// 返回: { w_engines: [...], drive_discs: [...], disc_sets: [...] }

// 保存/删除
#[tauri::command]
fn save_w_engine(app: AppHandle, data: String) -> Result<String, String>;
#[tauri::command]
fn delete_w_engine(app: AppHandle, id: String) -> Result<String, String>;
#[tauri::command]
fn save_drive_disc(app: AppHandle, data: String) -> Result<String, String>;
#[tauri::command]
fn delete_drive_disc(app: AppHandle, id: String) -> Result<String, String>;
#[tauri::command]
fn save_disc_set(app: AppHandle, data: String) -> Result<String, String>;
#[tauri::command]
fn delete_disc_set(app: AppHandle, set_id: String) -> Result<String, String>;
```

### 3.5 敌人 CRUD

```rust
#[tauri::command]
fn get_enemies(app: AppHandle) -> Result<String, String>;

#[tauri::command]
fn get_enemy(app: AppHandle, enemy_id: String) -> Result<String, String>;

#[tauri::command]
fn save_enemy(app: AppHandle, data: String) -> Result<String, String>;

#[tauri::command]
fn delete_enemy(app: AppHandle, enemy_id: String) -> Result<String, String>;
```

### 3.6 数据查询

```rust
// 通用数据概览 (各表记录数)
#[tauri::command]
fn get_data_summary(app: AppHandle) -> Result<String, String>;
// 返回: { "characters": 5, "skills": 24, "w_engines": 3, "drive_discs": 18, "disc_sets": 4, "enemies": 2 }

// 搜索角色/装备/敌人
#[tauri::command]
fn search_data(app: AppHandle, query: String, data_type: String) -> Result<String, String>;
```

---

---

## 4. DataLoader 改造方案 (zsim-core)

### 4.1 改造目标

将 `zsim-core/src/data/loader.rs` 从**文件系统 JSON 读取**改造为**SQLite 数据库读取**，使模拟引擎在运行时直接从 `zsim.db` 获取游戏数据，无需中间 JSON 文件。

### 4.2 依赖变更

**`zsim-core/Cargo.toml` 新增依赖**:
```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
```

### 4.3 改造后 `DataLoader` 接口设计

```rust
/// 改造后的 DataLoader — 从 SQLite 读取游戏数据
pub struct DataLoader {
    db: Connection,  // rusqlite Connection
}

impl DataLoader {
    /// 连接到 zsim.db
    pub fn new(db_path: &Path) -> Result<Self>;

    /// 加载所有角色
    pub fn load_characters(&self) -> Result<Vec<Character>>;

    /// 加载所有敌人
    pub fn load_enemies(&self) -> Result<Vec<EnemyState>>;

    /// 加载指定角色的所有技能
    pub fn load_skills(&self, char_id: &str) -> Result<Vec<SkillData>>;

    /// 加载所有技能（跨角色）
    pub fn load_all_skills(&self) -> Result<Vec<SkillData>>;

    /// 加载装备数据
    pub fn load_equipment(&self) -> Result<EquipmentData>;

    /// 加载 APL 数据 (从 SQLite 的 apl 表)
    pub fn load_apl(&self, apl_id: &str) -> Result<APLData>;
}
```

### 4.4 核心改造点

| 当前 (JSON) | 改造后 (SQLite) |
|---|---|
| `DataLoader` 为无状态结构体，方法接受 `&Path` | `DataLoader` 持有 `rusqlite::Connection`，构造时接受 `&Path` 指向 `.db` 文件 |
| `fs::read_to_string` + `serde_json::from_str` | `conn.query_row` / `conn.prepare` + 手动字段映射到 Rust 结构体 |
| `load_characters(dir: &Path)` 遍历目录下所有 `.json` | `load_characters()` 执行 `SELECT * FROM characters` |
| `load_skills(dir: &Path)` 多文件组合，先尝试数组再尝试对象 | `load_skills(char_id)` 执行 `SELECT ... FROM skills WHERE char_id = ?` + 联表查询 `skill_multipliers` |
| `load_equipment(path: &Path)` 单个 JSON 文件 | `load_equipment()` 三表联查：`w_engines` + `drive_discs` + `disc_sets`，组合为 `EquipmentData` |
| `load_apl(path: &Path)` 单个 JSON 文件 | `load_apl(apl_id)` 从 `apl` 表查询指定 APL 数据 |

### 4.5 SQL ↔ Rust 字段映射

核心原则：**Rust 结构体字段定义不变**，仅在加载层做 SQL → struct 的映射转换。

**示例：角色加载映射逻辑**

```rust
pub fn load_characters(&self) -> Result<Vec<Character>> {
    let mut stmt = self.db.prepare(
        "SELECT char_id, name, faction, specialty, element, level, ascension,
                hp, atk, def, impact, crit_rate, crit_dmg,
                pen_ratio, pen_fixed, anomaly_mastery, anomaly_proficiency,
                energy_regen, energy_gen_rate, constellations, action_dict
         FROM characters"
    )?;

    let characters = stmt.query_map([], |row| {
        Ok(Character {
            char_id: row.get(0)?,
            name: row.get(1)?,
            faction: row.get(2)?,
            specialty: row.get(3)?,
            element: row.get(4)?,
            level: row.get(5)?,
            ascension: row.get(6)?,
            base_stats: BaseStats {
                hp: row.get(7)?,
                atk: row.get(8)?,
                def: row.get(9)?,
                impact: row.get(10)?,
                crit_rate: row.get(11)?,
                crit_dmg: row.get(12)?,
                pen_ratio: row.get(13)?,
                pen_fixed: row.get(14)?,
                anomaly_mastery: row.get(15)?,
                anomaly_proficiency: row.get(16)?,
                energy_regen: row.get(17)?,
                energy_gen_rate: row.get(18)?,
            },
            constellations: serde_json::from_str(&row.get::<_, String>(19)?)?,
            action_dict: serde_json::from_str(&row.get::<_, String>(20)?)?,
            // 其余字段使用默认值
            ..Default::default()
        })
    })?;

    let mut result = Vec::new();
    for character in characters {
        result.push(character?);
    }
    Ok(result)
}
```

### 4.6 技能加载特殊处理

当前 JSON 的 `SkillData` 结构体包含内联的 `damage_multipliers: Vec<HitFrame>`，而 SQLite 将其拆分为 `skills` + `skill_multipliers` 两张表。加载时需要联表查询并组合：

```rust
pub fn load_skills(&self, char_id: &str) -> Result<Vec<SkillData>> {
    // 1. 查询 skills 表获取基础字段
    let mut stmt = self.db.prepare(
        "SELECT id, action_id, action_type, daze_multiplier, energy_cost,
                decibel_cost, hp_cost, cooldown_ticks, animation_frames,
                interruptible_frame, is_snapshot, prerequisite_action_id,
                effect_id
         FROM skills WHERE char_id = ?"
    )?;

    let mut skills = Vec::new();
    let skill_rows = stmt.query_map([char_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    for skill_row in skill_rows {
        let (skill_id, action_id) = skill_row?;

        // 2. 对每个技能查询其倍率段
        let mut mult_stmt = self.db.prepare(
            "SELECT frame, multiplier, decay_coeff
             FROM skill_multipliers
             WHERE skill_id = ? ORDER BY segment_index"
        )?;

        let hit_frames: Vec<HitFrame> = mult_stmt.query_map([skill_id], |row| {
            Ok(HitFrame {
                frame: row.get(0)?,
                multiplier: row.get(1)?,
                decay_coeff: row.get::<_, Option<f64>>(2)?.unwrap_or(1.0),
            })
        })?.filter_map(|r| r.ok()).collect();

        // 3. 组合为 SkillData
        skills.push(SkillData {
            action_id,
            damage_multipliers: hit_frames,
            // ... 其余字段
        });
    }

    Ok(skills)
}
```

### 4.7 装备加载方案

`EquipmentData` 是三个 Vec 的聚合结构体（`w_engines`、`drive_discs`、`disc_sets`）。改造后分别从三张表查询后组合：

```rust
pub fn load_equipment(&self) -> Result<EquipmentData> {
    let w_engines = self.load_all_w_engines()?;
    let drive_discs = self.load_all_drive_discs()?;
    let disc_sets = self.load_all_disc_sets()?;
    Ok(EquipmentData { w_engines, drive_discs, disc_sets })
}
```

### 4.8 APL 存储方案

新增 `apl` 表（与现有 `sample_apl.json` 结构对应）：

```sql
CREATE TABLE apl (
    apl_id      TEXT PRIMARY KEY,
    name        TEXT NOT NULL DEFAULT '',
    tracks      TEXT NOT NULL DEFAULT '[]',  -- JSON array, 格式同现有 APL JSON
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**`track_id` 和 `char_id` 在 `tracks` JSON 内部存储**（因为 APL 结构较为灵活，直接存 JSON 字段避免过度规范化）。

### 4.9 测试策略

原有单元测试（`#[cfg(test)] mod tests`）全部更新：

| 测试 | 原做法 | 新做法 |
|---|---|---|
| `test_load_characters` | 读 `data/characters/*.json` | 读测试用 `test_zsim.db`（构建时初始化） |
| `test_load_enemies` | 同上 | 同上 |
| `test_load_skills_*` | 同上 | 同上 |
| `test_load_equipment` | 读 `data/equipment/equipment.json` | 同上 |
| `test_load_apl` | 读 `data/apl/sample_apl.json` | 同上 |

**测试数据库方案**：在 `tests/` 目录下维护 `test_data.json`，测试启动时通过 `rusqlite` 建表并插入测试数据，运行完毕后清理临时 `.db` 文件。保持测试不依赖外部 `data/` 目录。

### 4.10 数据流变更总览

```
改造前:
  data/*.json ──→ DataLoader (JSON 解析) ──→ 模拟引擎

改造后:
  data/*.json ──→ 录入界面导入 ──→ zsim.db ──→ DataLoader (SQLite 查询) ──→ 模拟引擎
                                                           ↑
                                             (可选) 录入界面直接 CRUD

  向后兼容: DataLoader::from_json_dir() 保留为辅助方法，用于"一键导入"
```

### 4.11 向后兼容与迁移

1. 保留 `data/` 目录结构不变，JSON 文件作为"源文件"保留在仓库中
2. 新增 `DataLoader::from_json_dir(db_path, data_dir)` 方法：读取 JSON 数据 → 写入 SQLite → 再从 SQLite 读取（原子化导入）
3. 用户流程：克隆仓库 → `init_database`（自动将 `data/` JSON 导入 `zsim.db`）→ 录入界面编辑 → 模拟引擎读取 SQLite
4. 可选导出到 JSON（供版本控制/分享）

---

## 5. 前端 UI 设计

### 5.1 导航布局

采用**侧边栏 + 内容区**的两栏布局，与现有模拟器前端共存：

```
┌─────────────────────────────────────────────────────┐
│  ZSim Data Editor                    [🌐 语言] [⚙️]   │
├──────────┬──────────────────────────────────────────┤
│ 📊 总览  │  [内容区域]                               │
│ 👤 角色  │                                          │
│ ⚡ 技能  │  - 数据表格                               │
│ 🔧 装备  │  - 编辑表单 (行内/弹窗)                    │
│   ├ 音擎 │  - 验证提示                               │
│   ├ 驱动盘│                                          │
│   └ 套装 │                                          │
│ 👾 敌人  │                                          │
│ 📥 导入  │                                          │
└──────────┴──────────────────────────────────────────┘
```

### 5.2 页面设计

#### 5.2.1 数据总览页
- 卡片式显示各表记录数
- 最近编辑记录列表
- 快速搜索框 (跨表搜索)

#### 5.2.2 角色管理页

**列表视图**：
| 角色ID | 名称 | 阵营 | 专精 | 属性 | 等级 | 攻击力 | 生命值 | 防御力 | 操作 |
|--------|------|------|------|------|------|--------|--------|--------|------|

- 支持按阵营、专精、属性筛选
- 点击行展开详情或点击"编辑"按钮

**编辑/新增表单** (弹窗或独立面板)：
```
┌──────────────────────────────────────────────┐
  👤 角色编辑                                  │
  ┌──────────────────────────────────────────┐  │
  │ char_id: [anby_demara] name: [Anby]     │  │
  │ faction: [▼ Gentle_House]               │  │
  │ specialty: [▼ Stun] element: [▼ Electric]│  │
  │ level: [60] ascension: [6]              │  │
  ├────────── 基础面板 ──────────────────────┤  │
  │ HP:   [9500]   ATK:   [1100]            │  │
  │ DEF:  [550]    Impact: [120]            │  │
  │ CR:   [0.15]   CD:     [0.80]           │  │
  │ PEN:  [0.10]   PEN_F: [40.0]            │  │
  │ Anom_Mast: [80] Anom_Prof: [95]         │  │
  │ Energy_Regen: [1.2] Energy_Gen_Rate: [0.3]│  │
  ├────────── 影画 ──────────────────────────┤  │
  │ [■][□][□][□][□][□]  (点击切换)           │  │
  ├────────── 动作列表 ──────────────────────┤  │
  │ Attack_Normal_1  Attack_Normal_2  ...   │  │
  │ [添加动作]                               │  │
  ├──────────────────────────────────────────┤  │
  │          [保存]  [取消]  [删除]           │  │
  └──────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

**字段说明**：
- char_id 为只读（创建后不可修改，或修改时给出警告）
- 基础面板 12 个数值字段分两列排列
- 影画用 6 个按钮/开关表示
- 动作列表为多选下拉或标签输入

#### 5.2.3 技能编辑器

**顶部选择角色** → 加载该角色的所有技能列表：

```
┌──────────────────────────────────────────────┐
│ 角色: [▼ anby_demara]                        │
├──────────────────────────────────────────────┤
│ [新增技能]                                   │
├──────────────────────────────────────────────┤
│ 技能列表:                                     │
│ ┌─────────┬────────┬──────┬──────┬────────┐ │
│ │ action_id│ 类型   │能量消 │冷却帧 │ 操作   │ │
│ ├─────────┼────────┼──────┼──────┼────────┤ │
│ │ Normal_1│ Normal │ 0    │ 0    │ [✏][🗑]│ │
│ │ EX_1    │ Special│ 40   │ 8    │ [✏][🗑]│ │
│ │ ...     │        │      │      │        │ │
│ └─────────┴────────┴──────┴──────┴────────┘ │
└──────────────────────────────────────────────┘
```

**技能编辑面板** (展开或弹窗)：
```
┌──────────────────────────────────────────────┐
│ ⚡ 技能编辑                                   │
│ action_id: [Attack_Normal_1]                 │
│ action_type: [▼ Normal]                      │
│ ┌──────────── 基础参数 ─────────────────────┐ │
│ │ 能量消耗: [0]    分贝消耗: [0]            │ │
│ │ HP消耗: [0]      冷却帧: [0]              │ │
│ │ 动画帧: [30]     可中断帧: [25]           │ │
│ │ 击晕倍率: [0.4]  Is Snapshot: [□]        │ │
│ │ 前置动作: [▼ Attack_Normal_0 / 无]       │ │
│ │ 效果 ID: [buff_001]   ← 逻辑占位符        │ │
│ └────────────────────────────────────────────┘ │
│ ┌──────────── 伤害倍率段 ────────────────────┐ │
│ │ 段数 │ 命中帧 │ 倍率值 │ 衰减系数 │ 操作    │ │
│ │  1   │   8    │  0.50  │  1.0    │ [🗑]   │ │
│ │  2   │  16    │  0.70  │  1.0    │ [🗑]   │ │
│ │ [添加段]                                   │ │
│ └────────────────────────────────────────────┘ │
│              [保存] [取消] [删除]              │
└──────────────────────────────────────────────┘
```

**核心功能**：
- 倍率段支持动态增删行
- 每段可编辑命中帧、倍率值、衰减系数
- "效果 ID" 输入框用于标记未实现的复杂逻辑

#### 5.2.4 装备管理页

**三级 Tab 导航**：

```
┌──────────────────────────────────────────────┐
│ [音擎]  [驱动盘]  [套装]                       │
├──────────────────────────────────────────────┤
│ (音擎列表)                                    │
│ ┌──────┬──────┬──────┬──────┬─────────────┐  │
│ │ ID   │ 名称 │ 等级 │ 攻击 │ 被动效果      │  │
│ ├──────┼──────┼──────┼──────┼─────────────┤  │
│ │ ...  │      │      │      │              │  │
│ └──────┴──────┴──────┴──────┴─────────────┘  │
│              [新增音擎]                       │
└──────────────────────────────────────────────┘
```

**音擎编辑**：
- 基础面板字段（ATK、crit_rate、crit_dmg、pen_ratio 等）
- 被动效果 ID 列表（标签输入框）

**驱动盘编辑**：
- 槽位选择 (1-6)
- 主词条：名称 + 值
- 副词条：最多 4 条，动态增删
- 套装 ID 下拉选择

**套装编辑**：
- 二件套/四件套效果描述
- 效果 ID 输入框

#### 5.2.5 敌人配置页

```
┌──────────────────────────────────────────────┐
│ 👾 敌人编辑                                   │
│ enemy_id: [boss_dullahan]  名称: [杜拉罕]    │
│ 类型: [▼ Boss]             等级: [60]       │
│ HP: [150000]               DEF: [600]       │
│ 基础减伤: [0.15]           失衡上限: [200]  │
│ ┌────────── 抗性 ──────────────────────────┐ │
│ │ 属性           抗性值    操作             │ │
│ │ Ice            [0.40]    [🗑]            │ │
│ │ Ether          [0.60]    [🗑]            │ │
│ │ Fire           [0.10]    [🗑]            │ │
│ │ [添加抗性]                                │ │
│ └──────────────────────────────────────────┘ │
│ ┌────────── 弱点 ──────────────────────────┐ │
│ │ [Fire] [Physical] [添加弱点]              │ │
│ └──────────────────────────────────────────┘ │
│              [保存] [取消] [删除]             │
└──────────────────────────────────────────────┘
```

#### 5.2.6 JSON 导入页

```
┌──────────────────────────────────────────────┐
│ 📥 从 JSON 导入                               │
│                                              │
│ 数据目录: [data/]                    [扫描]   │
│                                              │
│ ┌──────────────────────────────────────────┐  │
│ │ 发现以下 JSON 文件:                        │  │
│ │ ☑ characters/anby_demara.json    → 角色   │  │
│ │ ☑ skills/anby_demara.json        → 技能   │  │
│ │ ☑ equipment/equipment.json       → 装备   │  │
│ │ ☑ enemies/boss_dullahan.json     → 敌人   │  │
│ │ ☑ apl/sample_apl.json            → APL   │  │
│ └──────────────────────────────────────────┘  │
│                                              │
│ 导入模式: [● 覆盖现有数据 / ○ 追加]           │
│                                              │
│              [开始导入]                       │
│                                              │
│ ┌─────────── 导入结果 ─────────────────────┐  │
│ │ ✅ 角色: 1 条导入成功                     │  │
│ │ ✅ 技能: 4 条导入成功                     │  │
│ │ ❌ 装备: 文件解析失败 (字段不匹配)          │  │
│ └──────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

### 5.3 交互规范

| 操作 | 交互方式 |
|------|----------|
| 新增 | 列表页顶部"新增"按钮 → 打开空白编辑表单 |
| 编辑 | 点击列表行或"编辑"按钮 → 打开预填表单 |
| 保存 | 表单底部"保存"按钮 → validate → invoke → 刷新列表 |
| 删除 | 二次确认弹窗 → 删除并刷新 |
| 取消编辑 | 关闭弹窗，不保存更改 |
| 搜索/筛选 | 列表顶部搜索框 + 筛选下拉 |
| 批量操作 | 暂不支持，MVP 以单条操作为主 |

### 5.4 文件结构

新增前端文件位于 `src/editor/` 目录下，与现有 `src/main.js` 并存：

```
src/
├── main.js              # 现有模拟器前端入口 (不变)
├── editor/              # 新增录入界面
│   ├── app.js           # 录入界面入口 (路由/导航)
│   ├── styles.css       # 录入界面样式
│   ├── components/
│   │   ├── sidebar.js   # 侧边栏导航
│   │   ├── datatable.js # 通用数据表格组件
│   │   ├── form.js      # 通用表单组件
│   │   └── confirm.js   # 确认弹窗
│   ├── pages/
│   │   ├── dashboard.js # 数据总览
│   │   ├── characters.js # 角色管理
│   │   ├── skills.js    # 技能编辑器
│   │   ├── equipment.js # 装备管理
│   │   ├── enemies.js   # 敌人配置
│   │   └── import.js    # JSON 导入
│   └── utils/
│       ├── api.js       # invoke 封装 (所有 Tauri 命令调用)
│       ├── validation.js # 表单验证
│       └── format.js    # 数值格式化
├── i18n.js              # 现有 i18n (扩展录入界面翻译)
├── styles.css           # 现有全局样式 (扩展)
├── locales/             # 新增录入界面翻译
│   ├── zh-CN.json
│   ├── en-US.json
│   └── ja-JP.json
└── index.html           # 现有入口 (新增导航入口点)
```

新增 Rust 文件在 `src-tauri/src/`：

```
src-tauri/src/
├── lib.rs               # 现有 Tauri 命令 (扩展)
├── data_entry/
│   ├── mod.rs           # 模块导出
│   ├── db.rs            # SQLite 连接/初始化
│   ├── characters.rs    # 角色 CRUD
│   ├── skills.rs        # 技能 CRUD
│   ├── equipment.rs     # 装备 CRUD
│   ├── enemies.rs       # 敌人 CRUD
│   └── import.rs        # JSON 导入逻辑
```

### 5.5 与现有前端的集成方案

**方案：独立页面 + 导航切换**

在 `index.html` 顶部添加导航 Tab：
```
┌──────────────────────────────────────────────┐
│ [🔬 模拟分析]  [📝 数据录入]                  │
├──────────────────────────────────────────────┤
│                ...                            │
```

- "模拟分析" → 加载现有 `main.js` 功能
- "数据录入" → 加载新的 `editor/app.js` 功能
- 两个视图共享全局样式、i18n、Tauri API 层
- 数据录入界面可通过 Tauri 命令直接读写 SQLite

---

## 6. 数据流与验证

### 6.1 前端 → Rust → SQLite 数据流

```
用户填写表单 → JS 校验 → invoke("save_character", { data: JSON.stringify(formData) })
  → Rust 命令接收 JSON String → serde_json::from_str → 验证字段
  → rusqlite INSERT OR REPLACE → 返回 { "status": "ok", "char_id": "anby_demara" }
  → 前端接收结果 → 显示成功提示 → 刷新列表
```

### 6.2 表单验证规则

| 字段 | 验证规则 |
|------|----------|
| char_id / enemy_id / id | 必填，仅允许小写字母、数字、下划线 |
| name | 必填，字符串 |
| level | 整数，1-60 |
| ascension | 整数，0-6 |
| 百分比数值 (crit_rate, pen_ratio, base_res 等) | 0.0 - 1.0 浮点数 |
| 非负数值 (HP, ATK, DEF 等) | ≥ 0 浮点数 |
| constellations | 6 元素 boolean 数组 |
| 倍率段 | ≥ 1 段，每段 frame ≥ 0, multiplier ≥ 0 |
| slot (驱动盘) | 整数 1-6 |
| 效果 ID | 可选，格式 `[a-z0-9_]+` |

---

## 7. 阶段规划

### 第一阶段：基础设施与 loader 改造
1. `src-tauri/Cargo.toml` 添加 `rusqlite` 依赖
2. 创建 `data_entry/` Rust 模块：`db.rs` (数据库初始化 + 建表)
3. 创建 `import.rs` (JSON 文件解析 + 批量导入)
4. **`zsim-core/Cargo.toml` 添加 `rusqlite` 依赖**
5. **改造 `zsim-core/src/data/loader.rs`**：新增 `DataLoader::new(db_path)`，实现从 SQLite 读取角色/技能/装备/敌人/APL 的方法
6. **更新 `loader.rs` 单元测试**：改为使用测试数据库而非 JSON 文件
7. 前端创建 `editor/` 目录结构 + 导航切换
8. Tauri 命令：`init_database`、`import_from_json`

### 第二阶段：角色 + 技能 CRUD + loader 联调
1. Rust：characters.rs (CRUD 命令)
2. Rust：skills.rs (CRUD 命令 + 倍率段管理)
3. 前端：角色管理页 (列表 + 编辑表单)
4. 前端：技能编辑器 (列表 + 编辑表单 + 倍率段动态行)
5. **联调验证：录入角色/技能 → zsim.db → DataLoader 正确读取 → 模拟引擎可消费**
6. 全链路联调测试

### 第三阶段：装备 + 敌人 CRUD + loader 联调
1. Rust：equipment.rs (音擎 + 驱动盘 + 套装的 CRUD)
2. Rust：enemies.rs (敌人 CRUD)
3. 前端：装备管理页 (三级 Tab + 编辑表单)
4. 前端：敌人配置页 (列表 + 编辑表单 + 抗性动态行)
5. **联调验证：录入装备/敌人 → zsim.db → DataLoader 正确读取 → 模拟引擎可消费**
6. i18n 中/英/日三语覆盖

### 第四阶段：完善与优化
1. 前端：数据总览页
2. 前端：JSON 导入页 + 进度反馈
3. 搜索功能
4. UX 优化 (表单自动保存提示、键盘快捷键、批量编辑)
5. 数据导出回 JSON（供模拟引擎使用）

---

## 8. 非功能性需求

### 8.1 性能
- 表单输入无感知延迟（本地 SQLite，无网络开销）
- 列表加载 ≤ 200ms（1000 条以内）
- 数据库初始化 ≤ 1s

### 8.2 安全
- 所有 Tauri 命令校验输入参数（拒绝 SQL 注入 — 使用参数化查询）
- 删除操作二次确认
- 数据库文件路径隔离（不覆盖用户系统文件）

### 8.3 兼容性
- 与现有 `data/` 目录 JSON 文件双向兼容（导入 ↔ 导出）
- 不破坏现有模拟引擎对 JSON 文件的读取逻辑
- 支持 Windows 10+（NSIS/MSI 打包）

### 8.4 可维护性
- Rust `data_entry/` 模块与现有 `lib.rs` 解耦
- 前端 `editor/` 目录独立，不与现有 `main.js` 产生冲突
- 数据库 Schema 变更通过版本号管理（预留 `schema_version` 元数据表）

---

## 9. 验收标准

| # | 验收条件 |
|---|---------|
| 1 | 应用启动后自动初始化 `zsim.db`，所有表创建成功 |
| 2 | 可以从 `data/` JSON 文件导入数据到 SQLite |
| 3 | 可以创建新角色并填写全部 12 个面板字段 |
| 4 | 可以为角色添加技能，每段技能倍率可动态增删 |
| 5 | 技能编辑器的"效果 ID"字段可自由输入并持久化 |
| 6 | 可以录入音擎和驱动盘，驱动盘主/副词条完整 |
| 7 | 可以为装备被动效果填写效果 ID |
| 8 | 可以录入敌人等级、防御、抗性（动态增删）、减伤系数 |
| 9 | 所有已录入数据可在界面中修改并保存 |
| 10 | 所有已录入数据可在界面中删除（含级联删除） |
| 11 | 角色列表支持按阵营/专精/属性筛选 |
| 12 | 中/英/日三语界面完整 |
| 13 | 删除操作有二次确认弹窗 |
| 14 | `DataLoader` 可从 `zsim.db` 正确读取所有角色及其完整面板字段 |
| 15 | `DataLoader` 可从 `zsim.db` 正确读取技能及关联的倍率段数据 |
| 16 | `DataLoader` 可从 `zsim.db` 正确读取音擎、驱动盘、套装全套装备数据 |
| 17 | `DataLoader` 可从 `zsim.db` 正确读取敌人数据 |
| 18 | `DataLoader` 从 SQLite 读取的数据内容与原始 JSON 文件完全一致 |
| 19 | `zsim-core` 的 loader 单元测试全部通过，不依赖 `data/` 目录下的 JSON 文件 |
| 20 | 数据可从 SQLite 导出回 JSON 供版本控制/分享 |
