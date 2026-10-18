//! macOS application composition. Authority stays in NativeHost, never the WebView.
use crate::ipc::NativeHost;
use focaus_contracts::{CommandResult, SafeError, Scope, CONTRACT_VERSION};
use serde::Serialize;
use std::{collections::HashMap, sync::{Arc, Mutex}, time::Duration};
use tauri::{Emitter, Manager};

struct RuntimeState {
    host: Arc<NativeHost>,
    window_ids: Mutex<HashMap<String, String>>,
}
#[derive(Serialize)]
struct Bootstrap { window_id: String, contract_version: &'static str }

#[tauri::command]
fn focaus_bootstrap(window: tauri::WebviewWindow, state: tauri::State<'_, RuntimeState>) -> Result<Bootstrap, SafeError> {
    let mut ids = state.window_ids.lock().map_err(|_| internal_error())?;
    let id = ids.entry(window.label().to_owned()).or_insert_with(|| uuid::Uuid::new_v4().to_string()).clone();
    // Labels and IDs are issued by the native process. Remote navigation is disallowed.
    let scope: Scope = serde_json::from_value(serde_json::json!({"kind":"global"})).map_err(|_| internal_error())?;
    state.host.bind_window(window.label(), &id, vec![scope])?;
    Ok(Bootstrap { window_id: id, contract_version: CONTRACT_VERSION })
}

#[tauri::command]
async fn focaus_command(window: tauri::WebviewWindow, state: tauri::State<'_, RuntimeState>, command: serde_json::Value) -> Result<CommandResult, SafeError> {
    let command = crate::wire::decode_command(command)?;
    let host = Arc::clone(&state.host);
    let label = window.label().to_owned();
    // Synchronous SQLite transactions and validation never run on the UI thread.
    let should_exit = matches!(&command.name, focaus_contracts::CommandName::AppExit);
    let result = tauri::async_runtime::spawn_blocking(move || host.dispatch_webview(&label, command))
        .await.map_err(|_| internal_error())??;
    crate::wire::validate_result(&result)?;
    if should_exit && result.status == focaus_contracts::CommandResultStatus::Ok { window.app_handle().exit(0); }
    Ok(result)
}
fn internal_error() -> SafeError {
    focaus_contracts::error(focaus_contracts::ErrorCode::InvalidResult, "Native application operation failed")
}
fn reveal_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show(); let _ = window.unminimize(); let _ = window.set_focus();
    }
}
fn explicit_exit(app: &tauri::AppHandle) {
    // A failed seal must not silently exit and lose recoverable recording state.
    match app.state::<RuntimeState>().host.exit() {
        Ok(_) => app.exit(0),
        Err(error) => { let _ = app.emit("focaus://host-error", error); reveal_main(app); }
    }
}
pub fn run() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![focaus_bootstrap, focaus_command])
        .setup(|app| {
            let root = std::env::var_os("FOCAUS_LIBRARY_ROOT").map(std::path::PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?.join("library"));
            let host = NativeHost::open(root, crate::INITIAL_MIGRATION)
                .map_err(|_| std::io::Error::other("Cannot open Focaus library"))?;
            app.manage(RuntimeState { host: Arc::new(host), window_ids: Mutex::new(HashMap::new()) });
            use tauri::menu::{Menu, MenuItem, Submenu, PredefinedMenuItem};
            let show = MenuItem::with_id(app, "show", "打开 focaus", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出 focaus 并保存录音", true, Some("CmdOrCtrl+Q"))?;
            let app_menu = Submenu::with_items(app, "focaus", true, &[&show, &quit])?;
            let edit_menu = Submenu::with_items(app, "编辑", true, &[
                &PredefinedMenuItem::undo(app, None)?, &PredefinedMenuItem::redo(app, None)?,
                &PredefinedMenuItem::separator(app)?, &PredefinedMenuItem::cut(app, None)?,
                &PredefinedMenuItem::copy(app, None)?, &PredefinedMenuItem::paste(app, None)?,
                &PredefinedMenuItem::select_all(app, None)?,
            ])?;
            app.set_menu(Menu::with_items(app, &[&app_menu, &edit_menu])?)?;
            app.on_menu_event(|app, event| match event.id.as_ref() {
                "show" => reveal_main(app), "quit" => explicit_exit(app), _ => {}
            });
            let menu = Menu::with_items(app, &[&show, &quit])?;
            // Template status icon stays visible even after every window is closed.
            let mut pixels = vec![0_u8; 16 * 16 * 4];
            for y in 2..14 { for x in 4..12 { if x < 6 || y < 4 || (7..9).contains(&y) {
                pixels[(y * 16 + x) * 4 + 3] = 255;
            } } }
            tauri::tray::TrayIconBuilder::with_id("focaus")
                .icon(tauri::image::Image::new_owned(pixels, 16, 16)).icon_as_template(true)
                .tooltip("focaus").menu(&menu)
                .build(app)?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    let state = handle.state::<RuntimeState>();
                    for label in handle.webview_windows().keys() {
                        match state.host.take_event_batches(label) {
                            Ok(batches) => for batch in batches { let _ = handle.emit_to(label, "focaus://events", batch); },
                            Err(_) => { /* unbound window; no subscription or sensitive error logging */ }
                        }
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close(); let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("Cannot initialize focaus desktop runtime");
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
        tauri::RunEvent::Reopen { .. } => reveal_main(app),
        _ => {}
    });
}
