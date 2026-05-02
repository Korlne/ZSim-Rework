use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};

// --- Managed state ---

struct SidecarState {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    stdout: Mutex<Option<BufReader<ChildStdout>>>,
}

/// Shared state for simulation cancellation.
/// When set to true, the running simulation should abort.
struct SimulationState {
    cancel_flag: Arc<AtomicBool>,
}

// --- Simulation progress event payload (matches frontend expectations) ---

#[derive(Clone, serde::Serialize)]
struct SimulationProgress {
    percent: f64,
    current: u64,
    total: u64,
}

/// Payload for the simulation-complete event.
#[derive(Clone, serde::Serialize)]
struct SimulationComplete {
    status: String,
    message: String,
    config: serde_json::Value,
}

// --- Tauri commands ---

/// Run a simulation with the provided JSON configuration.
///
/// This is the primary entry point for the simulation engine. The config
/// string is a JSON object with fields: sim_count, max_tick, base_seed,
/// data_dir, apl_file, output_path.
///
/// Spawns a background thread that emits progress events to the frontend
/// via Tauri's event system (`simulation-progress`) and sends a
/// `simulation-complete` event when finished. Supports cancellation via
/// the `stop_simulation` command.
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

    // Get the cancellation flag from managed state
    let state: tauri::State<'_, SimulationState> = app.state::<SimulationState>();
    let cancel_flag = state.cancel_flag.clone();
    // Reset cancellation flag for new run
    cancel_flag.store(false, Ordering::SeqCst);

    // Total steps for progress reporting: we simulate progress in 1% chunks
    let total_steps = 100u64;

    // Spawn a background thread so the Tauri command returns immediately
    // while the simulation runs and emits progress events.
    let app_clone = app.clone();
    let cfg_clone = cfg.clone();

    std::thread::spawn(move || {
        for step in 1..=total_steps {
            // Check for cancellation
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

            // Emit progress event
            let _ = app_clone.emit(
                "simulation-progress",
                SimulationProgress {
                    percent: step as f64,
                    current: step,
                    total: total_steps,
                },
            );

            // Simulate work: sleep proportional to workload
            let sleep_ms = (sim_count.saturating_mul(max_tick) / 10_000_000).clamp(10, 200);
            std::thread::sleep(Duration::from_millis(sleep_ms));
        }

        // Emit completion event
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

/// Stop the currently running simulation by setting the cancellation flag.
#[tauri::command]
fn stop_simulation(app: tauri::AppHandle) -> Result<String, String> {
    let state: tauri::State<'_, SimulationState> = app.state::<SimulationState>();
    state.cancel_flag.store(true, Ordering::SeqCst);
    Ok(serde_json::json!({"status": "cancelling"}).to_string())
}

/// Spawn the Python analysis sidecar process.
///
/// Launches `python analysis_sidecar.py` and waits for the initial
/// `{"type": "ready"}` message on stdout. Returns that message as a
/// JSON string. Stores the child process handle and stdin pipe for
/// later use by `send_to_sidecar`.
#[tauri::command]
fn spawn_sidecar(app: tauri::AppHandle) -> Result<String, String> {
    let state: tauri::State<'_, SidecarState> = app.state::<SidecarState>();

    // Kill any previously-running sidecar
    if let Ok(mut guard) = state.child.lock() {
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    // Launch the Python sidecar
    let mut child = Command::new("python")
        .arg("analysis_sidecar.py")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn sidecar: {e}"))?;

    // Take ownership of stdin for later writes
    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to capture sidecar stdin".to_string())?;

    // Take ownership of stdout and create a persistent BufReader
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture sidecar stdout".to_string())?;
    let mut stdout_reader = BufReader::new(child_stdout);

    // Read the "ready" message from stdout
    let mut ready_buf = String::new();
    stdout_reader
        .read_line(&mut ready_buf)
        .map_err(|e| format!("Failed to read sidecar output: {e}"))?;
    let ready_line = ready_buf.trim().to_string();

    // Store handles in managed state
    *state.child.lock().map_err(|e| e.to_string())? = Some(child);
    *state.stdin.lock().map_err(|e| e.to_string())? = Some(child_stdin);
    *state.stdout.lock().map_err(|e| e.to_string())? = Some(stdout_reader);

    Ok(ready_line)
}

/// Send a JSON command string to the sidecar's stdin.
///
/// The sidecar reads JSON commands on stdin and responds on stdout.
/// This function returns the command response as a JSON string.
#[tauri::command]
fn send_to_sidecar(app: tauri::AppHandle, command: String) -> Result<String, String> {
    let state: tauri::State<'_, SidecarState> = app.state::<SidecarState>();

    // Write the command followed by a newline (the sidecar uses line-delimited JSON)
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

    // Read the response line from stdout
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

// --- Application entry point ---

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(SidecarState {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
        })
        .manage(SimulationState {
            cancel_flag: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            run_simulation,
            stop_simulation,
            spawn_sidecar,
            send_to_sidecar,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
