//! Host-only file capabilities and immutable managed byte copies.
use crate::storage::{self, io_error, Result};
use focaus_contracts::{error, ErrorCode};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};

/// Open every path component relative to a pinned directory FD with NOFOLLOW.
/// A path swap by a converter cannot turn a staging capability into an arbitrary
/// host-file read. Absolute paths, parent traversal and symlinks all fail closed.
#[cfg(unix)]
pub fn open_relative(root: &Path, relative: &Path) -> Result<File> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
    };
    unsafe extern "C" {
        fn openat(fd: i32, path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    }
    #[cfg(target_os = "macos")]
    const NOFOLLOW: i32 = 0x100;
    #[cfg(target_os = "macos")]
    const DIRECTORY: i32 = 0x100000;
    #[cfg(not(target_os = "macos"))]
    const NOFOLLOW: i32 = 0x20000;
    #[cfg(not(target_os = "macos"))]
    const DIRECTORY: i32 = 0x10000;
    let parts = relative.components().collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(error(
            ErrorCode::PermissionDenied,
            "Invalid resource relative path",
        ));
    }
    let mut directory = File::open(root).map_err(io_error)?;
    for (i, part) in parts.iter().enumerate() {
        let Component::Normal(part) = part else {
            unreachable!()
        };
        let part = CString::new(part.as_bytes()).map_err(|_| {
            error(
                ErrorCode::PermissionDenied,
                "Invalid resource path component",
            )
        })?;
        // SAFETY: FD remains alive, CString is NUL terminated, no CREATE flag or
        // variadic mode is used. A successful fresh descriptor is owned once.
        let fd = unsafe {
            openat(
                directory.as_raw_fd(),
                part.as_ptr(),
                NOFOLLOW | if i + 1 < parts.len() { DIRECTORY } else { 0 },
            )
        };
        if fd < 0 {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Resource path is inaccessible or contains a link",
            ));
        }
        directory = unsafe { File::from_raw_fd(fd) };
    }
    if !directory.metadata().map_err(io_error)?.is_file() {
        return Err(error(
            ErrorCode::InvalidArgument,
            "Resource is not a regular file",
        ));
    }
    Ok(directory)
}
#[cfg(not(unix))]
pub fn open_relative(_: &Path, _: &Path) -> Result<File> {
    Err(error(
        ErrorCode::UnsupportedCapability,
        "Native resource adapter requires Unix",
    ))
}

/// Copy and hash an already-authorized descriptor. The destination is visible
/// only after fsync + rename + directory fsync; byte budgets are enforced while
/// streaming, including if the input grows after its first metadata check.
pub fn persist_bytes(
    mut source: File,
    root: &Path,
    relative: &str,
    max_bytes: u64,
) -> Result<(String, u64)> {
    if source.metadata().map_err(io_error)?.len() > max_bytes {
        return Err(error(
            ErrorCode::InvalidArgument,
            "Resource byte budget exceeded",
        ));
    }
    let destination = root.join(relative);
    if !relative.starts_with("resources/")
        || Path::new(relative)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(error(
            ErrorCode::PermissionDenied,
            "Invalid managed resource path",
        ));
    }
    let temporary = root
        .join("resources")
        .join(format!("{}.part", storage::id()));
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(io_error)?;
    let mut total = 0u64;
    let mut sha = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = source.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n as u64)
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "Resource byte budget exceeded"))?;
        if total > max_bytes {
            let _ = fs::remove_file(&temporary);
            return Err(error(
                ErrorCode::InvalidArgument,
                "Resource byte budget exceeded",
            ));
        }
        sha.update(&buffer[..n]);
        out.write_all(&buffer[..n]).map_err(io_error)?;
    }
    out.sync_all().map_err(io_error)?;
    if destination.exists() {
        let _ = fs::remove_file(&temporary);
        return Err(error(
            ErrorCode::Conflict,
            "Managed resource identity already exists",
        ));
    }
    // link is an atomic no-replace publication on the same managed filesystem.
    // A concurrent writer cannot replace an existing immutable resource.
    fs::hard_link(&temporary, &destination).map_err(io_error)?;
    fs::remove_file(&temporary).map_err(io_error)?;
    crate::audio::sync_directory(destination.parent().unwrap())?;
    Ok((format!("{:x}", sha.finalize()), total))
}
pub(crate) fn staging_root(root: &Path, task: &str) -> Result<PathBuf> {
    if task.len() != 36 || !task.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(error(ErrorCode::InvalidArgument, "Invalid task identity"));
    }
    let path = root.join("staging").join(task);
    fs::create_dir_all(&path).map_err(io_error)?;
    Ok(path)
}
pub fn read_resource(
    store: &storage::Store,
    input: focaus_contracts::ResourceReadInput,
) -> Result<serde_json::Value> {
    if input.length > 262_144 {
        return Err(error(
            ErrorCode::InvalidArgument,
            "Resource read exceeds chunk budget",
        ));
    }
    let (relative,sha,media,length)=store.read(|c|c.query_row("SELECT r.relative_path,r.sha256,r.media_type,r.byte_length FROM resources r WHERE r.resource_id=?1 AND r.state='ready' AND EXISTS(SELECT 1 FROM resource_owners o JOIN entities e ON e.entity_id=o.owner_entity_id WHERE o.resource_id=r.resource_id AND e.lifecycle<>'deleted')",[&input.resource_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,u64>(3)?))).optional().map_err(storage::sql_error)?.ok_or_else(||error(ErrorCode::SourceDeleted,"Resource has no available owner")))?;
    let mut file = open_relative(store.root(), Path::new(&relative))?;
    if file.metadata().map_err(io_error)?.len() != length {
        return Err(error(ErrorCode::CorruptAsset, "Resource size changed"));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if format!("{:x}", digest.finalize()) != sha {
        return Err(error(ErrorCode::CorruptAsset, "Resource checksum changed"));
    }
    if input.offset > length {
        return Err(error(
            ErrorCode::InvalidArgument,
            "Resource offset exceeds byte length",
        ));
    }
    let count = input.length.min(length - input.offset);
    let mut bytes = vec![0; count as usize];
    file.seek(SeekFrom::Start(input.offset)).map_err(io_error)?;
    file.read_exact(&mut bytes).map_err(io_error)?;
    let available=store.read(|c|c.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN resource_owners o ON o.resource_id=r.resource_id JOIN entities e ON e.entity_id=o.owner_entity_id WHERE r.resource_id=?1 AND r.state='ready' AND e.lifecycle<>'deleted')",params![input.resource_id],|r|r.get::<_,bool>(0)).map_err(storage::sql_error))?;
    if !available {
        return Err(error(
            ErrorCode::SourceDeleted,
            "Resource was deleted during read",
        ));
    }
    Ok(
        serde_json::json!({"resource_id":input.resource_id,"media_type":media,"byte_length":length,"offset":input.offset,"bytes":bytes,"eof":input.offset+count==length}),
    )
}
