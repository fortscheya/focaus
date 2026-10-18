//! macOS application composition. Authority stays in NativeHost, never the WebView.
use crate::ipc::NativeHost;
use crate::material_entry::MaterialJobHandle;
use focaus_contracts::MaterialImportInputPurpose;
use focaus_contracts::{CommandResult, SafeError, Scope, CONTRACT_VERSION};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Emitter, Manager};

struct RuntimeState {
    host: Arc<NativeHost>,
    window_ids: Mutex<HashMap<String, String>>,
    view_contexts: Mutex<HashMap<String, ViewContext>>,
}
#[derive(Clone, Serialize)]
struct ViewContext {
    view_id: String,
    transfer_id: Option<String>,
    base_view_revision: Option<String>,
}
#[derive(Serialize)]
struct Bootstrap {
    window_id: String,
    contract_version: &'static str,
    view_context: Option<ViewContext>,
}

#[tauri::command]
async fn focaus_material_import(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, RuntimeState>,
    lesson_id: String,
    purpose: MaterialImportInputPurpose,
    pages: Vec<u64>,
) -> Result<MaterialJobHandle, SafeError> {
    let host = Arc::clone(&state.host);
    let app = window.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::material_entry::import_from_picker(&host, &app, &lesson_id, purpose, pages)
    })
    .await
    .map_err(|_| internal_error())?
}

#[tauri::command]
async fn focaus_material_convert(
    state: tauri::State<'_, RuntimeState>,
    task_id: String,
) -> Result<serde_json::Value, SafeError> {
    let host = Arc::clone(&state.host);
    tauri::async_runtime::spawn_blocking(move || {
        crate::material_entry::run_local_sidecar(&host, &task_id)
    })
    .await
    .map_err(|_| internal_error())?
}

#[tauri::command]
fn focaus_bootstrap(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, RuntimeState>,
) -> Result<Bootstrap, SafeError> {
    let mut ids = state.window_ids.lock().map_err(|_| internal_error())?;
    let id = if let Some(id) = ids.get(window.label()) {
        id.clone()
    } else {
        let id = if window.label() == "main" {
            state.host.views.persistent_window_id("main", None)?
        } else {
            return Err(internal_error());
        };
        ids.insert(window.label().into(), id.clone());
        id
    };
    // Labels and IDs are issued by the native process. Remote navigation is disallowed.
    let scope: Scope = serde_json::from_value(serde_json::json!({"kind":"global"}))
        .map_err(|_| internal_error())?;
    state.host.bind_window(window.label(), &id, vec![scope])?;
    let view_context = state
        .view_contexts
        .lock()
        .map_err(|_| internal_error())?
        .get(window.label())
        .cloned();
    Ok(Bootstrap {
        window_id: id,
        contract_version: CONTRACT_VERSION,
        view_context,
    })
}

#[tauri::command]
async fn focaus_command(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, RuntimeState>,
    command: serde_json::Value,
) -> Result<CommandResult, SafeError> {
    let command = crate::wire::decode_command(command)?;
    let host = Arc::clone(&state.host);
    let label = window.label().to_owned();
    // Synchronous SQLite transactions and validation never run on the UI thread.
    let should_exit = matches!(&command.name, focaus_contracts::CommandName::AppExit);
    let command_name = command.name.clone();
    let payload = command.payload.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || host.dispatch_webview(&label, command))
            .await
            .map_err(|_| internal_error())??;
    crate::wire::validate_command_result(&command_name, &result)?;
    if result.status == focaus_contracts::CommandResultStatus::Ok {
        apply_window_result(
            window.app_handle(),
            &state,
            &command_name,
            &payload,
            &result,
        )?;
    }
    if should_exit && result.status == focaus_contracts::CommandResultStatus::Ok {
        window.app_handle().exit(0);
    }
    Ok(result)
}

/// OS effects follow the committed native transaction. The renderer cannot
/// choose labels or inject a window identity, and failed preparation rolls back.
fn apply_window_result(
    app: &tauri::AppHandle,
    state: &RuntimeState,
    name: &focaus_contracts::CommandName,
    payload: &serde_json::Value,
    result: &CommandResult,
) -> Result<(), SafeError> {
    use focaus_contracts::{CommandName, ContentView, ViewTransfer, ViewTransferState};
    let body = result.result.clone().ok_or_else(internal_error)?;
    match name {
        CommandName::ViewOpen if payload["placement"] == "standalone" => {
            let view: ContentView = serde_json::from_value(body).map_err(|_| internal_error())?;
            let context = ViewContext {
                view_id: view.view_id.clone(),
                transfer_id: None,
                base_view_revision: None,
            };
            if ensure_view_window(app, state, &view.host_window_id, context, true).is_err() {
                state
                    .host
                    .views
                    .initial_host_failed(&view.view_id, &view.host_window_id)?;
                return Err(internal_error());
            }
            state
                .host
                .views
                .confirm_initial_view(&view.view_id, &view.host_window_id)?;
        }
        CommandName::ViewMove => {
            let transfer: ViewTransfer =
                serde_json::from_value(body).map_err(|_| internal_error())?;
            let current = state
                .host
                .store
                .read(|c| crate::windows::get_transfer(c, &transfer.transfer_id))?;
            if current.state != ViewTransferState::Preparing {
                return Ok(());
            }
            let context = ViewContext {
                view_id: transfer.view_id.clone(),
                transfer_id: Some(transfer.transfer_id.clone()),
                base_view_revision: Some(transfer.base_view_revision.clone()),
            };
            let created =
                match ensure_view_window(app, state, &transfer.target_window_id, context, true) {
                    Ok(created) => created,
                    Err(_) => {
                        state.host.views.rollback_transfer(&transfer.transfer_id)?;
                        return Err(internal_error());
                    }
                };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                let state = app.state::<RuntimeState>();
                let rolled_back = state
                    .host
                    .store
                    .write(|t| {
                        let current = crate::windows::get_transfer(t, &transfer.transfer_id)?;
                        if current.state != ViewTransferState::Preparing {
                            return Ok(false);
                        }
                        crate::windows::rollback(t, &transfer.transfer_id)?;
                        Ok(true)
                    })
                    .unwrap_or(false);
                if rolled_back && created {
                    let label = format!("view-{}", transfer.target_window_id);
                    if let Some(window) = app.get_webview_window(&label) {
                        let _ = window.destroy();
                    }
                }
            });
        }
        CommandName::ViewReady => {
            let transfer: ViewTransfer =
                serde_json::from_value(body).map_err(|_| internal_error())?;
            if transfer.state == ViewTransferState::Committed {
                let mut contexts = state.view_contexts.lock().map_err(|_| internal_error())?;
                for context in contexts
                    .values_mut()
                    .filter(|c| c.transfer_id.as_deref() == Some(&transfer.transfer_id))
                {
                    context.transfer_id = None;
                    context.base_view_revision = None;
                }
                drop(contexts);
                // A detached source is a one-view carrier; the main window stays.
                if let Some(old) =
                    app.get_webview_window(&format!("view-{}", transfer.source_window_id))
                {
                    let _ = old.destroy();
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn ensure_view_window(
    app: &tauri::AppHandle,
    state: &RuntimeState,
    window_id: &str,
    context: ViewContext,
    standalone: bool,
) -> Result<bool, SafeError> {
    let existing = state
        .window_ids
        .lock()
        .map_err(|_| internal_error())?
        .iter()
        .find(|(_, id)| id.as_str() == window_id)
        .map(|(label, _)| label.clone());
    if let Some(label) = existing {
        if let Some(window) = app.get_webview_window(&label) {
            state
                .view_contexts
                .lock()
                .map_err(|_| internal_error())?
                .insert(label.clone(), context.clone());
            window
                .emit("focaus://view-transfer", &context)
                .map_err(|_| internal_error())?;
            window.show().map_err(|_| internal_error())?;
            return Ok(false);
        }
    }
    let label = format!("{}-{window_id}", if standalone { "view" } else { "course" });
    state
        .window_ids
        .lock()
        .map_err(|_| internal_error())?
        .insert(label.clone(), window_id.into());
    state
        .view_contexts
        .lock()
        .map_err(|_| internal_error())?
        .insert(label.clone(), context);
    let scope: Scope = serde_json::from_value(serde_json::json!({"kind":"global"}))
        .map_err(|_| internal_error())?;
    state.host.bind_window(&label, window_id, vec![scope])?;
    let created =
        tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::App("index.html".into()))
            .title("focaus · 内容窗口")
            .inner_size(900., 700.)
            .min_inner_size(600., 420.)
            .on_navigation(|url| {
                matches!(url.scheme(), "tauri" | "about")
                    || url.host_str() == Some("tauri.localhost")
            })
            .build();
    if created.is_err() {
        state
            .window_ids
            .lock()
            .map_err(|_| internal_error())?
            .remove(&label);
        state
            .view_contexts
            .lock()
            .map_err(|_| internal_error())?
            .remove(&label);
        state.host.unbind_window(&label)?;
        return Err(internal_error());
    }
    Ok(true)
}
fn internal_error() -> SafeError {
    focaus_contracts::error(
        focaus_contracts::ErrorCode::InvalidResult,
        "Native application operation failed",
    )
}
fn reveal_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn explicit_exit(app: &tauri::AppHandle) {
    // A failed seal must not silently exit and lose recoverable recording state.
    match app.state::<RuntimeState>().host.exit() {
        Ok(_) => app.exit(0),
        Err(error) => {
            let _ = app.emit("focaus://host-error", error);
            reveal_main(app);
        }
    }
}
pub fn run() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            focaus_bootstrap,
            focaus_command,
            focaus_material_import,
            focaus_material_convert
        ])
        .setup(|app| {
            let root = std::env::var_os("FOCAUS_LIBRARY_ROOT").map(std::path::PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?.join("library"));
            let host = NativeHost::open(root, crate::INITIAL_MIGRATION)
                .map_err(|_| std::io::Error::other("Cannot open Focaus library"))?;
            app.manage(RuntimeState { host: Arc::new(host), window_ids: Mutex::new(HashMap::new()), view_contexts: Mutex::new(HashMap::new()) });
            {
                let state = app.state::<RuntimeState>();
                let main = state.host.views.persistent_window_id("main", None).map_err(|_| std::io::Error::other("Cannot restore window identity"))?;
                state.window_ids.lock().map_err(|_| std::io::Error::other("Window registry unavailable"))?.insert("main".into(), main.clone());
                let plans = state.host.views.restore_plan().map_err(|_| std::io::Error::other("Cannot restore content windows"))?;
                for plan in plans.into_iter().filter(|p| p.window_id != main) {
                    if let Some(view_id) = plan.view_ids.first() {
                        let context = ViewContext { view_id: view_id.clone(), transfer_id: None, base_view_revision: None };
                        ensure_view_window(app.handle(), &state, &plan.window_id, context, plan.placement == "standalone")
                            .map_err(|_| std::io::Error::other("Cannot restore content window"))?;
                    }
                }
            }
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
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.state::<RuntimeState>();
                if let Ok(mut contexts) = state.view_contexts.lock() {
                    if let Some(context) = contexts.remove(window.label()) {
                        if let Some(transfer) = context.transfer_id { let _ = state.host.store.write(|t| crate::windows::rollback(t, &transfer)); }
                    }
                }
                let _ = state.host.unbind_window(window.label());
                if let Ok(mut ids) = state.window_ids.lock() { ids.remove(window.label()); };
            }
        })
        .build(tauri::generate_context!())
        .expect("Cannot initialize focaus desktop runtime");
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        tauri::RunEvent::Reopen { .. } => reveal_main(app),
        _ => {}
    });
}
