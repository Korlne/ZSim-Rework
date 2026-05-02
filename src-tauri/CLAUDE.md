# zsim-tauri

Tauri v2 desktop shell for ZSim Analyzer.

## Key Patterns

- **Commands**: Define with `#[tauri::command]` in `lib.rs`, register via `generate_handler![]`. Async commands use `async fn`.
- **State**: Managed state via `app.manage()` and accessed via `app.state::<T>()`. Use `Mutex<T>` for interior mutability.
- **Sidecar**: Python analysis sidecar is managed via `std::process::Command` wrapped in `tauri::State<SidecarState>` (not Tauri v1 `api::process::Command`).
- **Windows ICO**: `icons/icon.ico` required by `tauri-build` even for `cargo check`. Generate with Python `struct.pack` for raw BGRA bytes.
- **Permissions**: Tauri v2 uses `src-tauri/capabilities/default.json` for capability-based permissions.

## Dev Setup

- Frontend: Vanilla JS + Vite 6 in `src/` and `package.json` at project root
- `npm run dev` starts Vite on `:1420`
- `npm run tauri dev` starts full Tauri dev mode
- `dist/` must exist for production builds (Vite output)
