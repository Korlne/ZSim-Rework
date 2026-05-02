# PRD: 数据录入系统重构 — equipment 拆分 + CSV 导入 + 驱动盘验证

## Introduction

当前数据录入系统的 equipment 模块同时管理三种装备类型（W-Engine、Drive Disc、Disc Set），且仅支持 JSON 导入。为了更好的可维护性和数据录入体验，需要将 equipment 拆分为独立的模块，并添加 CSV 导入支持。同时，Drive Disc 编辑表单需要按槽位添加主/副属性的可选词条验证。

## Goals

- 将 equipment 模块重命名为 drive_disc，功能拆分到三个独立模块
- W-Engine 从 equipment 中分离，作为一级导航项
- Disc Sets 作为 Drive Discs 的子 Tab 保留
- 添加 CSV 导入功能，支持从 CSV 文件批量录入数据
- Drive Disc 编辑表单添加每槽位主属性/副属性的可选词条验证规则
- 不修改现有 JSON 导入/导出逻辑

## User Stories

### US-001: 后端 equipment → drive_disc + wengine 拆分
**Description:** 作为开发者，我需要将 equipment.rs 拆分为 drive_disc.rs 和 wengine.rs，使代码结构清晰、职责单一。

**Acceptance Criteria:**
- [ ] equipment.rs 文件重命名为 drive_disc.rs，保留 DriveDiscRecord / DiscSetRecord 及对应 CRUD
- [ ] 创建 wengine.rs 文件，包含 WEngineRecord 及对应 CRUD（从原 equipment.rs 提取）
- [ ] mod.rs 更新：`pub mod drive_disc;` + `pub mod wengine;`
- [ ] lib.rs 更新所有 Tauri 命令注册，确保 drive_disc 和 wengine 命令可访问
- [ ] cargo build 编译通过

### US-002: 前端 equipment.js → drive_disc.js + wengine.js 拆分
**Description:** 作为用户，我希望在界面中看到 W-Engines 作为独立的导航页面，Drive Discs 和 Disc Sets 作为 Drive Discs 页面下的子 Tab。

**Acceptance Criteria:**
- [ ] equipment.js 重命名为 drive_disc.js，保留 drive-discs + disc-sets 子 Tab 渲染
- [ ] 创建 wengine.js 页面（从原 equipment.js 提取 W-Engine 列表/编辑/删除逻辑）
- [ ] app.js 路由更新：`w-engines` → `./pages/wengine.js`，`drive-discs` / `disc-sets` → `./pages/drive_disc.js`
- [ ] vite build 构建通过

### US-003: 更新侧边栏导航和 i18n
**Description:** 作为用户，我希望侧边栏导航按新结构排列：Dashboard / W-Engines / Characters / Skills / Drive Discs（含 Disc Sets 子项）/ Enemies / Import。

**Acceptance Criteria:**
- [ ] sidebar.js 更新：Equipment 改为 Drive Discs（含 w-engines 改为顶级项，保留 drive-discs / disc-sets 子项）
- [ ] W-Engines 作为顶级项出现在 Drive Discs 之前
- [ ] Dashboard 搜索结果映射更新（w_engine → #/w-engines）
- [ ] 三语 i18n 文件添加 editor.nav.wEngines（顶级）、更新 editor.nav.driveDiscs、editor.nav.discSets

### US-004: 添加 CSV 依赖和导入基础设施
**Description:** 作为开发者，我需要在 Rust 端添加 CSV 解析依赖，并创建通用的 CSV 导入框架。

**Acceptance Criteria:**
- [ ] src-tauri/Cargo.toml 添加 `csv = "1.3"`
- [ ] import.rs 添加 `import_from_csv(conn, csv_path, data_type)` 函数签名
- [ ] lib.rs 注册 `import_from_csv` Tauri 命令
- [ ] cargo build 编译通过

### US-005: 实现 CSV 导入 — Disc Sets
**Description:** 作为用户，我希望通过 CSV 文件批量导入 Disc Set 数据（套装 ID、名称、二件套/四件套效果描述）。

**Acceptance Criteria:**
- [ ] 解析 CSV：列名映射到 set_id / name / two_piece_description / four_piece_description
- [ ] 使用 UPSERT（ON CONFLICT(set_id) DO UPDATE SET）实现幂等导入
- [ ] 返回导入结果统计（成功数、错误数）
- [ ] 错误行跳过并记录错误信息，不影响其他行导入

### US-006: 实现 CSV 导入 — W-Engines
**Description:** 作为用户，我希望能通过 CSV 文件批量导入音擎数据（ID、名称、等级、攻击力、暴击率等面板字段）。

**Acceptance Criteria:**
- [ ] 解析 CSV：列名映射到 id / name / level / ascension / atk / crit_rate / crit_dmg / pen_ratio / energy_regen / impact / anomaly_mastery
- [ ] 列名大小写不敏感
- [ ] 使用 UPSERT 实现幂等导入
- [ ] 返回导入结果统计

### US-007: 实现 CSV 导入 — Drive Discs
**Description:** 作为用户，我希望能通过 CSV 文件批量导入驱动盘数据（ID、槽位、等级、套装 ID、主属性、副属性）。

**Acceptance Criteria:**
- [ ] 解析 CSV：列名映射到 id / slot / level / set_id / main_stat_name / main_stat_value / sub_stat_1_name / sub_stat_1_value / ... / sub_stat_4_value
- [ ] slot 验证 1-6 范围
- [ ] 使用 UPSERT 实现幂等导入
- [ ] 返回导入结果统计

### US-008: 前端 CSV 导入界面
**Description:** 作为用户，我希望在 Import 页面能看到 CSV 导入入口，选择文件类型后执行导入。

**Acceptance Criteria:**
- [ ] import.js 添加 CSV 导入区域：显示支持的类型（W-Engines / Drive Discs / Disc Sets）
- [ ] 每个类型有对应的导入按钮
- [ ] 导入时显示进度/结果
- [ ] 在 Tauri 环境中通过文件对话框选择 CSV 文件路径

### US-009: Drive Disc 每槽位主属性验证
**Description:** 作为用户，我在编辑 Drive Disc 时，系统应该根据所选的槽位（1-6）限制可选的主属性词条。

**Acceptance Criteria:**
- [ ] 槽 1：主属性固定为"HP"，下拉只显示 HP
- [ ] 槽 2：主属性固定为"ATK"，下拉只显示 ATK
- [ ] 槽 3：主属性固定为"DEF"，下拉只显示 DEF
- [ ] 槽 4：主属性可选 ATK% / HP% / DEF% / 暴击率 / 暴击伤害 / 异常精通
- [ ] 槽 5：主属性可选 ATK% / HP% / DEF% / 穿透率 / 物理伤害加成 / 火/冰/电/以太/风属性伤害加成
- [ ] 槽 6：主属性可选 ATK% / HP% / DEF% / 异常掌控 / 冲击力 / 能量自动回复
- [ ] 前端表单根据 slot 切换动态更新主属性下拉选项

### US-010: Drive Disc 每槽位副属性验证
**Description:** 作为用户，我在编辑 Drive Disc 时，系统应该验证副属性不能与主属性重复词条，且副属性来自正确的词条池。

**Acceptance Criteria:**
- [ ] 所有槽位的副属性从统一词条池选择：HP / HP% / ATK / ATK% / DEF / DEF% / 穿透值 / 暴击率 / 暴击伤害 / 异常精通
- [ ] 副属性的下拉选项自动排除已选的主属性词条
- [ ] 如果主属性选了"暴击率"，副属性下拉中不显示"暴击率"
- [ ] 最多 4 条副属性，每条可选 name + value
- [ ] vite build 通过

## Functional Requirements

- FR-1: equipment.rs 拆分为 drive_disc.rs + wengine.rs，原有 CRUD 功能完整迁移
- FR-2: W-Engines 作为侧边栏顶级导航项，位于 Dashboard 之后、Characters 之前
- FR-3: Drive Discs 作为侧边栏顶级项，含 Drive Discs / Disc Sets 两个子 Tab
- FR-4: CSV 导入使用 UPSERT 策略：同 ID 覆盖，新 ID 插入
- FR-5: CSV 导入支持三种类型：w_engine / drive_disc / disc_set
- FR-6: CSV 列名大小写不敏感
- FR-7: Drive Disc 主属性下拉选项根据 slot 值动态过滤
- FR-8: Drive Disc 副属性选项自动排除已选主属性词条
- FR-9: 排除了主属性的后端验证（Rust 端 return error）

## Non-Goals

- 不添加 Characters / Skills / Enemies 的 CSV 导入（如需后续添加）
- 不修改现有的 JSON 导入/导出代码（`import.rs` 中 `import_*` 函数不动）
- 不添加 APL 的 CSV 导入
- 不修改数据库 schema
- 不重构其他模块（characters/skills/enemies）

## Technical Considerations

- 现有 `import.rs` 中的 `import_from_json` 保持不动，CSV 作为独立入口
- Cargo.toml 添加 `csv = "1.3"` crate
- frontend api.js 添加 `importFromCsv(filePath, dataType)` 函数
- import.js 页面添加 CSV 区域，和已有的 JSON 导入区域并列
- Drive Disc 词条验证在 validation.js 中实现，equipment.js（drive_disc.js）表单使用

## Success Metrics

- cargo build + vite build 无错误
- 导航按 W-Engines / Characters / Skills / Drive Discs / Disc Sets / Enemies / Import 顺序正确显示
- CSV 导入后，SQLite 中数据正确（get_w_engines / get_drive_discs / get_disc_sets 可查）
- Drive Disc 编辑时切换 slot 1-6，主属性下拉选项自动更新

## Open Questions

- CSV 文件路径如何传入？通过 Tauri 文件对话框选择，还是固定在 `data/` 目录下读取？
