# zsim-tauri

Tauri v2 desktop shell for ZSim Analyzer.

## Key Patterns

- **Commands**: Define with `#[tauri::command]` in `lib.rs`, register via `generate_handler![]`. Async commands use `async fn`.
- **State**: Managed state via `app.manage()` and accessed via `app.state::<T>()`. Use `Mutex<T>` for interior mutability.
- **Sidecar**: Python analysis sidecar is managed via `std::process::Command` wrapped in `tauri::State<SidecarState>` (not Tauri v1 `api::process::Command`). SidecarState stores child process, stdin pipe, and a persistent `BufReader<ChildStdout>` for reading line-delimited JSON responses. `spawn_sidecar` takes ownership of `child.stdout` and reads the initial `{"type":"ready"}` line. `send_to_sidecar` writes a command to stdin then reads exactly one response line from the stdout reader. Both stdin and stdout are wrapped in `Mutex` for thread-safe access.
- **Long-running commands**: For operations that block (simulation, etc.), use `std::thread::spawn` with `app.clone()` to emit progress events. Use `SimulationState` with `Arc<AtomicBool>` for cancellation. Import `tauri::Emitter` trait to call `app.emit()`.
- **Windows ICO**: `icons/icon.ico` required by `tauri-build` even for `cargo check`. Generate with Python `struct.pack` for raw BGRA bytes.
- **Permissions**: Tauri v2 uses `src-tauri/capabilities/default.json` for capability-based permissions.

## Dev Setup

- Frontend: Vanilla JS + Vite 6 in `src/` and `package.json` at project root
- `npm run dev` starts Vite on `:1420`
- `npm run tauri dev` starts full Tauri dev mode
- `dist/` must exist for production builds (Vite output)
