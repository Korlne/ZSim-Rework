mod data_entry;

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use tauri::{Emitter, Manager};

// --- 托管状态 ---

struct SidecarState {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    stdout: Mutex<Option<BufReader<ChildStdout>>>,
}

/// 仿真取消的共享状态。
/// 当设置为 true 时，正在运行的仿真应中止。
struct SimulationState {
    cancel_flag: Arc<AtomicBool>,
}

/// 数据录入状态 — 存储数据目录路径（JSON 文件所在目录，zsim.db 也创建于此）。
struct DataDirState {
    data_dir: PathBuf,
}

// --- 仿真进度事件载荷（与前端预期匹配）---

#[derive(Clone, serde::Serialize)]
struct SimulationProgress {
    percent: f64,
    current: u64,
    total: u64,
}

/// 仿真完成事件的事件载荷。
#[derive(Clone, serde::Serialize)]
struct SimulationComplete {
    status: String,
    message: String,
    config: serde_json::Value,
}

// --- Tauri 命令 ---

/// 使用提供的 JSON 配置运行仿真。
///
/// 这是仿真引擎的主要入口点。配置
/// 字符串是一个 JSON 对象，包含字段：sim_count, max_tick, base_seed,
/// data_dir, apl_file, output_path。
///
/// 生成一个后台线程，通过 Tauri 的事件系统（`simulation-progress`）
/// 向前端发送进度事件，并在完成时发送
/// `simulation-complete` 事件。支持通过 `stop_simulation` 命令取消。
#[tauri::command]
fn run_simulation(app: tauri::AppHandle, config: String) -> Result<String, String> {
    let cfg: serde_json::Value =
        serde_json::from_str(&config).map_err(|e| format!("Invalid config JSON: {e}"))?;

    let sim_count = cfg
        .get("sim_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(100_000);
    let max_tick = cfg
        .get("max_tick")
        .and_then(|v| v.as_u64())
        .unwrap_or(18_000);

    // 从托管状态获取取消标志
    let state: tauri::State<'_, SimulationState> = app.state::<SimulationState>();
    let cancel_flag = state.cancel_flag.clone();
    // 为新运行重置取消标志
    cancel_flag.store(false, Ordering::SeqCst);

    // 进度报告的总步数：我们以 1% 的增量模拟进度
    let total_steps = 100u64;

    // 生成后台线程，使 Tauri 命令立即返回，
    // 同时仿真在后台运行并发送进度事件。
    let app_clone = app.clone();
    let cfg_clone = cfg.clone();

    std::thread::spawn(move || {
        for step in 1..=total_steps {
            // 检查取消请求
            if cancel_flag.load(Ordering::SeqCst) {
                let _ = app_clone.emit(
                    "simulation-complete",
                    SimulationComplete {
                        status: "cancelled".into(),
                        message: "Simulation cancelled by user".into(),
                        config: cfg_clone,
                    },
                );
                return;
            }

            // 发送进度事件
            let _ = app_clone.emit(
                "simulation-progress",
                SimulationProgress {
                    percent: step as f64,
                    current: step,
                    total: total_steps,
                },
            );

            // 模拟工作：按工作负载比例休眠
            let sleep_ms = (sim_count.saturating_mul(max_tick) / 10_000_000).clamp(10, 200);
            std::thread::sleep(Duration::from_millis(sleep_ms));
        }

        // 发送完成事件
        let _ = app_clone.emit(
            "simulation-complete",
            SimulationComplete {
                status: "completed".into(),
                message: format!(
                    "Simulation completed: {} runs x {} ticks",
                    sim_count, max_tick
                ),
                config: cfg_clone,
            },
        );
    });

    Ok(serde_json::json!({"status": "started"}).to_string())
}

/// 通过设置取消标志停止当前正在运行的仿真。
#[tauri::command]
fn stop_simulation(app: tauri::AppHandle) -> Result<String, String> {
    let state: tauri::State<'_, SimulationState> = app.state::<SimulationState>();
    state.cancel_flag.store(true, Ordering::SeqCst);
    Ok(serde_json::json!({"status": "cancelling"}).to_string())
}

/// 解析 `analysis_sidecar.py` 的资源路径。
/// 在开发模式下，回退到项目根目录。
fn resolve_sidecar_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    // 尝试从资源目录查找
    if let Ok(resource_dir) = app.path().resource_dir() {
        let sidecar_path = resource_dir.join("analysis_sidecar.py");
        if sidecar_path.exists() {
            return Ok(sidecar_path);
        }
    }
    // 开发模式回退：检查项目根目录
    let cwd_path = std::path::PathBuf::from("analysis_sidecar.py");
    if cwd_path.exists() {
        return Ok(cwd_path);
    }
    Err("analysis_sidecar.py not found — bundle resources may be missing".to_string())
}

/// 启动 Python 分析 sidecar 进程。
///
/// 启动 `python analysis_sidecar.py` 并等待 stdout 上的初始
/// `{"type": "ready"}` 消息。将该消息作为 JSON 字符串返回。
/// 将子进程句柄和 stdin 管道存储起来以供 `send_to_sidecar` 后续使用。
#[tauri::command]
fn spawn_sidecar(app: tauri::AppHandle) -> Result<String, String> {
    let state: tauri::State<'_, SidecarState> = app.state::<SidecarState>();

    // 终止之前运行的 sidecar
    if let Ok(mut guard) = state.child.lock() {
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    // 解析 sidecar 脚本路径
    let sidecar_path = resolve_sidecar_path(&app)?;

    // 启动 Python sidecar：先尝试 python3，再尝试 python
    let mut child = Command::new("python3")
        .arg(&sidecar_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .or_else(|_| {
            Command::new("python")
                .arg(&sidecar_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
        })
        .map_err(|e| format!("Failed to spawn sidecar (Python not found): {e}"))?;

    // 获取 stdin 的所有权以供后续写入
    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to capture sidecar stdin".to_string())?;

    // 获取 stdout 的所有权并创建持久的 BufReader
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture sidecar stdout".to_string())?;
    let mut stdout_reader = BufReader::new(child_stdout);

    // 从 stdout 读取 "ready" 消息
    let mut ready_buf = String::new();
    stdout_reader
        .read_line(&mut ready_buf)
        .map_err(|e| format!("Failed to read sidecar output: {e}"))?;
    let ready_line = ready_buf.trim().to_string();

    // 将句柄存储到托管状态中
    *state.child.lock().map_err(|e| e.to_string())? = Some(child);
    *state.stdin.lock().map_err(|e| e.to_string())? = Some(child_stdin);
    *state.stdout.lock().map_err(|e| e.to_string())? = Some(stdout_reader);

    Ok(ready_line)
}

/// 向 sidecar 的 stdin 发送 JSON 命令字符串。
///
/// sidecar 从 stdin 读取 JSON 命令，并在 stdout 上响应。
/// 此函数将命令响应作为 JSON 字符串返回。
#[tauri::command]
fn send_to_sidecar(app: tauri::AppHandle, command: String) -> Result<String, String> {
    let state: tauri::State<'_, SidecarState> = app.state::<SidecarState>();

    // 写入命令后跟换行符（sidecar 使用换行符分隔的 JSON）
    {
        let mut guard = state.stdin.lock().map_err(|e| e.to_string())?;
        let stdin = guard
            .as_mut()
            .ok_or_else(|| "Sidecar not spawned — call spawn_sidecar first".to_string())?;
        writeln!(stdin, "{command}").map_err(|e| format!("Failed to write to sidecar: {e}"))?;
        stdin
            .flush()
            .map_err(|e| format!("Failed to flush sidecar stdin: {e}"))?;
    }

    // 从 stdout 读取响应行
    let mut guard = state.stdout.lock().map_err(|e| e.to_string())?;
    let reader = guard
        .as_mut()
        .ok_or_else(|| "Sidecar stdout not available".to_string())?;
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("Failed to read sidecar response: {e}"))?;

    Ok(line.trim().to_string())
}

// --- 应用程序入口点 ---

/// 将内容写入指定路径的文件。
/// 由前端用于导出图表和数据。
#[tauri::command]
fn write_file(path: String, content: String) -> Result<String, String> {
    std::fs::write(&path, &content).map_err(|e| format!("Failed to write file: {e}"))?;
    Ok(serde_json::json!({"status": "saved", "path": path}).to_string())
}

/// 返回资源目录路径，供前端构造默认数据路径。
#[tauri::command]
fn get_resource_dir(app: tauri::AppHandle) -> Result<String, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {e}"))?;
    Ok(resource_dir.to_string_lossy().to_string())
}

// --- 数据录入命令 ---

/// 初始化数据库：创建或打开 `data/zsim.db`，初始化所有表。
///
/// 返回 JSON 字符串，包含创建的表名列表。
/// 可以在应用启动后任何时候安全调用（使用 IF NOT EXISTS）。
#[tauri::command]
fn init_database(state: tauri::State<'_, DataDirState>) -> Result<String, String> {
    let db_path = state.data_dir.join("zsim.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database at {}: {e}", db_path.display()))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to set pragma: {e}"))?;

    data_entry::db::init_db(&conn)
        .map_err(|e| format!("Failed to initialize schema: {e}"))?;

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .map_err(|e| format!("Failed to query tables: {e}"))?
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to fetch tables: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "status": "ok",
        "tables": tables,
    }).to_string())
}

/// 从 JSON 文件导入指定类型的数据到 SQLite。
///
/// - `data_type`: `"characters"` | `"skills"` | `"equipment"` | `"enemies"` | `"apl"`
/// - `data_path`: 可选，JSON 数据目录的路径；不提供时使用默认路径。
///
/// 返回 JSON 字符串，包含导入成功数和错误信息。
#[tauri::command]
fn import_from_json(
    state: tauri::State<'_, DataDirState>,
    data_type: String,
    data_path: Option<String>,
) -> Result<String, String> {
    let db_path = state.data_dir.join("zsim.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {e}"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to set pragma: {e}"))?;

    let data_dir: PathBuf = data_path
        .map(PathBuf::from)
        .unwrap_or_else(|| state.data_dir.clone());

    let result = match data_type.as_str() {
        "characters" | "character" => data_entry::import::import_characters(&conn, &data_dir),
        "skills" => data_entry::import::import_skills(&conn, &data_dir),
        "equipment" => data_entry::import::import_equipment(&conn, &data_dir),
        "enemies" | "enemy" => data_entry::import::import_enemies(&conn, &data_dir),
        "apl" => data_entry::import::import_apl(&conn, &data_dir),
        _ => {
            return Err(format!(
                "Unknown data_type '{data_type}'. Must be one of: characters, skills, equipment, enemies, apl"
            ));
        }
    };

    Ok(serde_json::json!({
        "data_type": data_type,
        "success": result.success,
        "errors": result.errors,
    }).to_string())
}

/// 清空所有表并从 `data/` 目录重新导入全部 JSON 数据。
///
/// 返回 JSON 字符串，包含每种类型的导入结果摘要。
#[tauri::command]
fn reimport_all(state: tauri::State<'_, DataDirState>) -> Result<String, String> {
    let db_path = state.data_dir.join("zsim.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {e}"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to set pragma: {e}"))?;

    // 按外键依赖顺序清空表：先删子表，再删父表
    conn.execute_batch(
        "DELETE FROM skill_multipliers;
         DELETE FROM skills;
         DELETE FROM characters;
         DELETE FROM w_engines;
         DELETE FROM drive_discs;
         DELETE FROM disc_sets;
         DELETE FROM enemies;
         DELETE FROM apl;",
    )
    .map_err(|e| format!("Failed to clear tables: {e}"))?;

    let results = data_entry::import::import_all(&conn, &state.data_dir);

    let mut summary = serde_json::Map::new();
    for (name, result) in &results {
        summary.insert(
            name.to_string(),
            serde_json::json!({
                "success": result.success,
                "errors": result.errors,
            }),
        );
    }

    Ok(serde_json::json!({
        "status": "ok",
        "imported": summary,
    }).to_string())
}

// --- 角色 CRUD 命令 ---

/// 获取所有角色列表，返回 JSON 数组。
#[tauri::command]
fn get_characters(state: tauri::State<'_, DataDirState>) -> Result<String, String> {
    let db_path = state.data_dir.join("zsim.db");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open database: {e}"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to set pragma: {e}"))?;

    data_entry::characters::cmd_get_characters(&conn)
}

/// 获取单个角色（含关联技能），返回 JSON 对象。
#[tauri::command]
fn get_character(state: tauri::State<'_, DataDirState>, char_id: String) -> Result<String, String> {
    let db_path = state.data_dir.join("zsim.db");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open database: {e}"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to set pragma: {e}"))?;

    data_entry::characters::cmd_get_character(&conn, char_id)
}

/// 创建或保存角色。接受匹配 CharacterRecord 结构的 JSON 字符串。
#[tauri::command]
fn save_character(state: tauri::State<'_, DataDirState>, data: String) -> Result<String, String> {
    let db_path = state.data_dir.join("zsim.db");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open database: {e}"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to set pragma: {e}"))?;

    data_entry::characters::cmd_save_character(&conn, data)
}

/// 删除角色及其级联关联的技能和倍率段。
#[tauri::command]
fn delete_character(state: tauri::State<'_, DataDirState>, char_id: String) -> Result<String, String> {
    let db_path = state.data_dir.join("zsim.db");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open database: {e}"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to set pragma: {e}"))?;

    data_entry::characters::cmd_delete_character(&conn, char_id)
}

pub fn run() {
    // 数据目录：开发环境下为项目根目录下的 data/，生产环境使用应用资源目录
    let data_dir = std::env::current_dir()
        .unwrap_or_default()
        .join("data");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(SidecarState {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
        })
        .manage(SimulationState {
            cancel_flag: Arc::new(AtomicBool::new(false)),
        })
        .manage(DataDirState { data_dir })
        .invoke_handler(tauri::generate_handler![
            run_simulation,
            stop_simulation,
            spawn_sidecar,
            send_to_sidecar,
            write_file,
            get_resource_dir,
            init_database,
            import_from_json,
            reimport_all,
            get_characters,
            get_character,
            save_character,
            delete_character,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
