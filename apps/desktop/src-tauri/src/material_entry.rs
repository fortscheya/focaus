//! Native-only material import and local sidecar boundary.
//!
//! The WebView receives task and material identities only. The selected file,
//! managed source path, staging directory, and sidecar request stay inside the
//! native process.
use crate::ipc::NativeHost;
use focaus_contracts::{
    error, Capabilities, EngineRequest, EngineRequestName, EngineResponse, EngineResponseKind,
    MaterialImportInputPurpose, MaterialManifest, MaterialPageResultInput, SafeError, TaskState,
    CONTRACT_VERSION,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

/// A user may need to navigate a file provider or mounted volume before
/// choosing a material. Keep the native command bounded, but leave enough
/// time for ordinary manual selection instead of treating it as a 15-second
/// background operation.
const PICKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const PICKER_TIMEOUT_MESSAGE: &str = "Native material picker timed out after 5 minutes";

#[derive(Debug, Clone, Serialize)]
pub struct MaterialJobHandle {
    pub task_id: String,
    pub material_id: String,
    pub source_version_id: String,
    pub parse_version_id: String,
    pub parent_parse_version_id: Option<String>,
}

pub fn job_handle(host: &NativeHost, task_id: &str) -> Result<MaterialJobHandle, SafeError> {
    let job = host.materials.job(task_id)?;
    Ok(MaterialJobHandle {
        task_id: job.task.task_id,
        material_id: job.material_id,
        source_version_id: job.source_version_id,
        parse_version_id: job.parse_version_id,
        parent_parse_version_id: job.parent_parse_version_id,
    })
}

pub fn import_from_picker(
    host: &NativeHost,
    app: &tauri::AppHandle,
    lesson_id: &str,
    purpose: MaterialImportInputPurpose,
    pages: Vec<u64>,
) -> Result<MaterialJobHandle, SafeError> {
    let Some(picked) = pick_file(app)? else {
        return Err(error(
            focaus_contracts::ErrorCode::Cancelled,
            "Material import was cancelled",
        ));
    };
    let job = host.import_material_original_file(
        picked.file,
        &picked.display_name,
        lesson_id,
        purpose,
        pages,
    )?;
    Ok(MaterialJobHandle {
        task_id: job.task.task_id,
        material_id: job.material_id,
        source_version_id: job.source_version_id,
        parse_version_id: job.parse_version_id,
        parent_parse_version_id: job.parent_parse_version_id,
    })
}

/// Run the configured local material engine. Page results and checkpoints are
/// validated into the host-owned task staging state; only the completed
/// manifest can advance the immutable material head. No network client or
/// credential resolver is reachable from this process boundary.
pub fn run_local_sidecar(host: &NativeHost, task_id: &str) -> Result<Value, SafeError> {
    let job = host.materials.job(task_id)?;
    let _ = host
        .tasks
        .transition(task_id, job.task.attempt, TaskState::Running)?;
    let result = run_local_sidecar_inner(host, task_id, &job);
    if result.is_err() {
        let _ = host
            .tasks
            .transition(task_id, job.task.attempt, TaskState::Failed);
    }
    result
}

fn run_local_sidecar_inner(
    host: &NativeHost,
    task_id: &str,
    job: &crate::library::materials::MaterialJob,
) -> Result<Value, SafeError> {
    let request = job.engine_request()?;
    let python =
        std::env::var_os("FOCAUS_MATERIAL_ENGINE_PYTHON").unwrap_or_else(|| "python3".into());
    let module_root = std::env::var_os("FOCAUS_MATERIAL_ENGINE").ok_or_else(|| {
        error(
            focaus_contracts::ErrorCode::UnsupportedCapability,
            "Local material engine is not configured",
        )
    })?;
    let handshake = EngineRequest {
        contract_version: CONTRACT_VERSION.into(),
        request_id: crate::storage::id(),
        name: EngineRequestName::Handshake,
        task_id: None,
        payload: json!({}),
    };
    let mut child = Command::new(python)
        .args(["-m", "material_engine"])
        .env("PYTHONPATH", module_root)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            error(
                focaus_contracts::ErrorCode::UnsupportedCapability,
                "Local material engine could not start",
            )
        })?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Local material engine input unavailable",
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Local material engine output unavailable",
        )
    })?;
    writeln!(
        stdin,
        "{}",
        serde_json::to_string(&handshake).map_err(|_| {
            error(
                focaus_contracts::ErrorCode::InvalidResult,
                "Cannot encode material engine handshake",
            )
        })?
    )
    .map_err(|_| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Cannot send material engine handshake",
        )
    })?;
    stdin.flush().map_err(|_| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Cannot flush material engine handshake",
        )
    })?;
    let mut reader = BufReader::new(stdout);
    let ready = read_engine_response(&mut reader)?;
    if ready.kind != EngineResponseKind::Ready
        || ready.request_id != handshake.request_id
        || ready.task_id.is_some()
    {
        return Err(error(
            focaus_contracts::ErrorCode::InvalidResult,
            "Material engine did not complete its handshake",
        ));
    }
    verify_capabilities(&ready.payload)?;
    writeln!(
        stdin,
        "{}",
        serde_json::to_string(&request).map_err(|_| {
            error(
                focaus_contracts::ErrorCode::InvalidResult,
                "Cannot encode material engine request",
            )
        })?
    )
    .map_err(|_| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Cannot send material engine request",
        )
    })?;
    stdin.flush().map_err(|_| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Cannot flush material engine request",
        )
    })?;
    drop(stdin);
    let mut published = None;
    while let Some(response) = read_engine_response_optional(&mut reader)? {
        let identity_matches = response.request_id == request.request_id
            && response.task_id.as_deref() == Some(task_id);
        if !identity_matches {
            return Err(error(
                focaus_contracts::ErrorCode::InvalidResult,
                "Material engine response identity does not match the request",
            ));
        }
        match response.kind {
            EngineResponseKind::Ready => {
                return Err(error(
                    focaus_contracts::ErrorCode::InvalidResult,
                    "Material engine sent a second handshake response",
                ));
            }
            EngineResponseKind::Completed => {
                let manifest: MaterialManifest = serde_json::from_value(
                    response.payload.get("manifest").cloned().ok_or_else(|| {
                        error(
                            focaus_contracts::ErrorCode::InvalidResult,
                            "Material engine completed without a manifest",
                        )
                    })?,
                )
                .map_err(|_| {
                    error(
                        focaus_contracts::ErrorCode::InvalidResult,
                        "Material engine manifest is invalid",
                    )
                })?;
                let result = host.accept_material_completed(MaterialPageResultInput {
                    task_id: task_id.into(),
                    snapshot_id: job.task.snapshot_id.clone(),
                    manifest,
                }, &response.payload)?;
                let _ = host
                    .tasks
                    .transition(task_id, job.task.attempt, TaskState::Succeeded)?;
                published = Some(result);
            }
            EngineResponseKind::Error => {
                return Err(error(
                    focaus_contracts::ErrorCode::ProviderError,
                    "Material engine rejected the local request",
                ));
            }
            EngineResponseKind::PageResult => {
                host.accept_material_page_result(task_id, &job.task.snapshot_id, &response.payload)?;
            }
            EngineResponseKind::Checkpoint => {
                host.record_material_checkpoint(task_id, job.task.attempt, &response.payload)?;
            }
        }
    }
    let status = child.wait().map_err(|_| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Cannot wait for material engine",
        )
    })?;
    published.ok_or_else(|| {
        if status.success() {
            error(
                focaus_contracts::ErrorCode::InvalidResult,
                "Material engine ended without a completed manifest",
            )
        } else {
            error(
                focaus_contracts::ErrorCode::ProviderError,
                "Material engine exited before publication",
            )
        }
    })
}

fn read_engine_response(reader: &mut impl BufRead) -> Result<EngineResponse, SafeError> {
    read_engine_response_optional(reader)?.ok_or_else(|| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Material engine ended before responding",
        )
    })
}

fn read_engine_response_optional(
    reader: &mut impl BufRead,
) -> Result<Option<EngineResponse>, SafeError> {
    let mut line = String::new();
    let read = reader.read_line(&mut line).map_err(|_| {
        error(
            focaus_contracts::ErrorCode::ProviderError,
            "Cannot read material engine response",
        )
    })?;
    if read == 0 {
        return Ok(None);
    }
    serde_json::from_str(&line).map(Some).map_err(|_| {
        error(
            focaus_contracts::ErrorCode::InvalidResult,
            "Material engine returned an invalid response",
        )
    })
}

fn verify_capabilities(payload: &Value) -> Result<(), SafeError> {
    let capabilities: Capabilities =
        serde_json::from_value(payload.get("capabilities").cloned().ok_or_else(|| {
            error(
                focaus_contracts::ErrorCode::UnsupportedVersion,
                "Material engine did not declare capabilities",
            )
        })?)
        .map_err(|_| {
            error(
                focaus_contracts::ErrorCode::UnsupportedVersion,
                "Material engine capabilities are unknown",
            )
        })?;
    if !capabilities.supported_contract_majors.contains(&2)
        || !capabilities.supported_material_majors.contains(&2)
        || !capabilities.search_projection_versions.contains(&1)
    {
        return Err(error(
            focaus_contracts::ErrorCode::UnsupportedVersion,
            "Material engine cannot publish this package version",
        ));
    }
    Ok(())
}

struct PickedFile {
    file: std::fs::File,
    display_name: String,
}

#[cfg(target_os = "macos")]
fn pick_file(app: &tauri::AppHandle) -> Result<Option<PickedFile>, SafeError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result = pick_file_on_main_thread();
        let _ = sender.send(result);
    })
    .map_err(|_| {
        error(
            focaus_contracts::ErrorCode::InvalidResult,
            "Cannot open native file picker",
        )
    })?;
    receive_picker_result(receiver, PICKER_RESPONSE_TIMEOUT)
}

fn receive_picker_result<T>(
    receiver: mpsc::Receiver<Result<T, SafeError>>,
    timeout: Duration,
) -> Result<T, SafeError> {
    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(error(
            focaus_contracts::ErrorCode::DeadlineExceeded,
            PICKER_TIMEOUT_MESSAGE,
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(error(
            focaus_contracts::ErrorCode::InvalidResult,
            "Native file picker callback disconnected",
        )),
    }
}

#[cfg(not(target_os = "macos"))]
fn pick_file(_: &tauri::AppHandle) -> Result<Option<PickedFile>, SafeError> {
    Err(error(
        focaus_contracts::ErrorCode::UnsupportedCapability,
        "Native material picker requires macOS",
    ))
}

#[cfg(target_os = "macos")]
fn pick_file_on_main_thread() -> Result<Option<PickedFile>, SafeError> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::{NSArray, NSString};
    use std::{ffi::CStr, fs::File, os::unix::ffi::OsStrExt, path::PathBuf};
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        error(
            focaus_contracts::ErrorCode::InvalidResult,
            "Native file picker must run on the macOS main thread",
        )
    })?;
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setResolvesAliases(false);
    let mut allowed = NSArray::<NSString>::array();
    for extension in [
        "pdf", "pptx", "txt", "md", "markdown", "png", "jpg", "jpeg", "gif", "webp",
    ] {
        let item = NSString::from_str(extension);
        allowed = allowed.arrayByAddingObject(&item);
    }
    #[allow(deprecated)]
    panel.setAllowedFileTypes(Some(&allowed));
    if panel.runModal() != NSModalResponseOK {
        return Ok(None);
    }
    let url = panel.URLs().firstObject().ok_or_else(|| {
        error(
            focaus_contracts::ErrorCode::InvalidArgument,
            "No material file selected",
        )
    })?;
    if !url.isFileURL() {
        return Err(error(
            focaus_contracts::ErrorCode::PermissionDenied,
            "Selected material is not a local file",
        ));
    }
    let raw = url.fileSystemRepresentation();
    let path = PathBuf::from(std::ffi::OsStr::from_bytes(unsafe {
        CStr::from_ptr(raw.as_ptr()).to_bytes()
    }));
    let display_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            error(
                focaus_contracts::ErrorCode::InvalidArgument,
                "Selected material name is invalid",
            )
        })?
        .to_owned();
    // Open the descriptor while the security-scoped extension is active. The
    // descriptor, rather than this path, crosses into MaterialService, so the
    // scope can be released before returning from the main-thread callback.
    let scoped = unsafe { url.startAccessingSecurityScopedResource() };
    let file = File::open(&path).map_err(crate::storage::io_error);
    if scoped {
        unsafe { url.stopAccessingSecurityScopedResource() };
    }
    Ok(Some(PickedFile {
        file: file?,
        display_name,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities() -> Value {
        json!({
            "capabilities": {
                "supported_contract_majors": [2],
                "supported_material_majors": [2],
                "visualization_protocol_majors": [],
                "templates": {},
                "search_projection_versions": [1],
                "focaus_result_semantics": []
            }
        })
    }

    #[test]
    fn unknown_engine_capability_is_rejected_before_publication() {
        assert!(verify_capabilities(&capabilities()).is_ok());
        let mut unknown = capabilities();
        unknown["capabilities"]["supported_material_majors"] = json!([1]);
        assert_eq!(
            verify_capabilities(&unknown).unwrap_err().code,
            focaus_contracts::ErrorCode::UnsupportedVersion
        );
        let mut no_search = capabilities();
        no_search["capabilities"]["search_projection_versions"] = json!([]);
        assert_eq!(
            verify_capabilities(&no_search).unwrap_err().code,
            focaus_contracts::ErrorCode::UnsupportedVersion
        );
    }

    #[test]
    fn picker_wait_is_bounded_and_distinguishes_timeout_from_callback_disconnect() {
        assert!(PICKER_RESPONSE_TIMEOUT > Duration::from_secs(15));
        assert!(PICKER_RESPONSE_TIMEOUT <= Duration::from_secs(10 * 60));

        let (_sender, receiver) = mpsc::sync_channel::<Result<(), SafeError>>(1);
        let timed_out = receive_picker_result(receiver, Duration::from_millis(1)).unwrap_err();
        assert_eq!(
            timed_out.code,
            focaus_contracts::ErrorCode::DeadlineExceeded
        );
        assert_eq!(timed_out.message, PICKER_TIMEOUT_MESSAGE);

        let (sender, receiver) = mpsc::sync_channel::<Result<(), SafeError>>(1);
        drop(sender);
        let disconnected = receive_picker_result(receiver, Duration::from_millis(1)).unwrap_err();
        assert_eq!(
            disconnected.code,
            focaus_contracts::ErrorCode::InvalidResult
        );
    }
}
