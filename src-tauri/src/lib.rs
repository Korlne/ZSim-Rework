use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;

use tauri::Manager;

// --- Managed state for the Python analysis sidecar ---

struct SidecarState {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
}

// --- Tauri commands ---

/// Run a simulation with the provided JSON configuration.
///
/// This is the primary entry point for the simulation engine. The config
/// string is a JSON object with fields: sim_count, max_tick, base_seed,
/// data_dir, apl_file, output_path.
///
/// Currently a stub — will be wired to zsim-core's ParallelRunner in a
/// future story.
#[tauri::command]
fn run_simulation(config: String) -> Result<String, String> {
    let _cfg: serde_json::Value =
        serde_json::from_str(&config).map_err(|e| format!("Invalid config JSON: {e}"))?;

    // Stub response — real implementation delegates to zsim-core::combat::parallel
    let response = serde_json::json!({
        "status": "ok",
        "message": "Simulation completed (stub)",
        "config_parsed": true
    });

    Ok(response.to_string())
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

    // Read the "ready" message from stdout
    let ready_line = {
        let stdout = child
            .stdout
            .as_mut()
            .ok_or_else(|| "Failed to capture sidecar stdout".to_string())?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read sidecar output: {e}"))?;
        line.trim().to_string()
    };

    // Store handles in managed state
    *state.child.lock().map_err(|e| e.to_string())? = Some(child);
    *state.stdin.lock().map_err(|e| e.to_string())? = Some(child_stdin);

    Ok(ready_line)
}

/// Send a JSON command string to the sidecar's stdin.
///
/// The sidecar reads JSON commands on stdin and responds on stdout.
/// This function returns the command response as a JSON string.
#[tauri::command]
fn send_to_sidecar(app: tauri::AppHandle, command: String) -> Result<String, String> {
    let state: tauri::State<'_, SidecarState> = app.state::<SidecarState>();

    let mut guard = state.stdin.lock().map_err(|e| e.to_string())?;
    let stdin = guard
        .as_mut()
        .ok_or_else(|| "Sidecar not spawned — call spawn_sidecar first".to_string())?;

    // Write the command followed by a newline (the sidecar uses line-delimited JSON)
    writeln!(stdin, "{command}")
        .map_err(|e| format!("Failed to write to sidecar: {e}"))?;
    stdin
        .flush()
        .map_err(|e| format!("Failed to flush sidecar stdin: {e}"))?;

    // Stub: real implementation will read the response from stdout
    let response = serde_json::json!({ "type": "ok", "message": "Command sent (stub)" });
    Ok(response.to_string())
}

// --- Application entry point ---

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(SidecarState {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            run_simulation,
            spawn_sidecar,
            send_to_sidecar,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
