# zsim-tauri

Tauri v2 desktop shell for ZSim Analyzer.

## Key Patterns

- **Commands**: Define with `#[tauri::command]` in `lib.rs`, register via `generate_handler![]`. Async commands use `async fn`.
- **State**: Managed state via `app.manage()` and accessed via `app.state::<T>()`. Use `Mutex<T>` for interior mutability.
- **Sidecar**: Python analysis sidecar is managed via `std::process::Command` wrapped in `tauri::State<SidecarState>` (not Tauri v1 `api::process::Command`). SidecarState stores child process, stdin pipe, and a persistent `BufReader<ChildStdout>` for reading line-delimited JSON responses. `spawn_sidecar` takes ownership of `child.stdout` and reads the initial `{"type":"ready"}` line. `send_to_sidecar` writes a command to stdin then reads exactly one response line from the stdout reader. Both stdin and stdout are wrapped in `Mutex` for thread-safe access.
- **Long-running commands**: For operations that block (simulation, etc.), use `std::thread::spawn` with `app.clone()` to emit progress events. Use `SimulationState` with `Arc<AtomicBool>` for cancellation. Import `tauri::Emitter` trait to call `app.emit()`.
- **Windows ICO**: `icons/icon.ico` required by `tauri-build` even for `cargo check`. Generate with Python `struct.pack` for raw BGRA bytes.
- **Permissions**: Tauri v2 uses `src-tauri/capabilities/default.json` for capability-based permissions.
- **File Export with Dialog**: Use `tauri-plugin-dialog` (Rust) + `@tauri-apps/plugin-dialog` (JS) for save dialogs. Frontend calls `save()` from `@tauri-apps/plugin-dialog` to get a file path, then invokes a `write_file` Tauri command (defined in `lib.rs`) via `invoke("write_file", { path, content })` to write content. Add `dialog:allow-save` to capabilities. Register plugin with `.plugin(tauri_plugin_dialog::init())`.

## Bundle / Packaging

- **Icon set**: Tauri v2 requires at minimum: `icons/32x32.png`, `icons/128x128.png`, `icons/128x128@2x.png`, `icons/icon.ico`, `icons/icon.icns`. Generate with Python Pillow or `npx @tauri-apps/cli icon`.
- **Resources**: Use `bundle.resources` in `tauri.conf.json` with glob patterns relative to `src-tauri/` dir (e.g., `"../data/**/*.json"`, `"../analysis_sidecar.py"`).
- **Resource API**: At runtime, resolve bundled files via `app.path().resource_dir().join("filename")` (from `tauri::Manager` trait). For scripts bundled as resources, always provide a CWD-relative fallback for dev mode.
- **Bundle targets**: Configure `bundle.targets` as `["nsis", "msi", "appimage", "deb", "dmg"]`. Each platform only builds its applicable targets. Cross-platform builds require CI matrix (GitHub Actions).
- **Sidecar Python resolution**: Use `resolve_sidecar_path()` pattern: try `resource_dir().join("analysis_sidecar.py")`, fall back to CWD. Spawn with `python3` first, then `python`.
- **NSIS**: Tauri can auto-generate NSIS installer. `bundle.windows.nsis.installMode = "currentUser"` for per-user install.
- **Bundle sizes**: Release binary ~11 MB, NSIS installer ~2.5 MB, MSI ~3.8 MB (well under 200 MB limit).

## Dev Setup

- Frontend: Vanilla JS + Vite 6 in `src/` and `package.json` at project root
- `npm run dev` starts Vite on `:1420`
- `npm run tauri dev` starts full Tauri dev mode
- `dist/` must exist for production builds (Vite output)
- `npx tauri build` generates installers (NSIS + MSI on Windows, DMG on macOS, AppImage+deb on Linux)
