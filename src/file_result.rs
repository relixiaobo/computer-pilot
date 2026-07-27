use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const OUTPUT_DIR_ENV: &str = "COMPUTER_PILOT_OUTPUT_DIR";
static NEXT_FILE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Serialize)]
pub struct FileResult {
    pub path: String,
    pub mime: &'static str,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

pub fn write_png<T, F>(
    requested: Option<String>,
    prefix: &str,
    capture: F,
) -> Result<(T, FileResult), String>
where
    F: FnOnce(&str) -> Result<(T, f64), String>,
{
    let destination = destination_path(requested, prefix)?;
    let temporary = temporary_path(&destination)?;
    let mut reservation = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| format!("failed to reserve output file: {error}"))?;
    reservation
        .write_all(&[])
        .map_err(|error| format!("failed to initialize output file: {error}"))?;
    drop(reservation);

    let temporary_string = temporary.to_string_lossy().into_owned();
    let captured = capture(&temporary_string);
    let (value, scale) = match captured {
        Ok(captured) => captured,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    };

    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|error| {
        cleanup_error(&temporary, format!("failed to secure output file: {error}"))
    })?;
    let (width, height) =
        png_dimensions(&temporary).map_err(|error| cleanup_error(&temporary, error))?;
    let file = fs::File::open(&temporary).map_err(|error| {
        cleanup_error(&temporary, format!("failed to open output file: {error}"))
    })?;
    file.sync_all().map_err(|error| {
        cleanup_error(&temporary, format!("failed to sync output file: {error}"))
    })?;
    let bytes = file
        .metadata()
        .map_err(|error| {
            cleanup_error(
                &temporary,
                format!("failed to inspect output file: {error}"),
            )
        })?
        .len();

    // hard_link publishes the completed inode atomically and fails if the
    // destination appeared after validation, giving us no-overwrite semantics.
    fs::hard_link(&temporary, &destination).map_err(|error| {
        cleanup_error(
            &temporary,
            format!("failed to publish output without overwriting: {error}"),
        )
    })?;
    fs::remove_file(&temporary)
        .map_err(|error| format!("output published but temporary link cleanup failed: {error}"))?;

    Ok((
        value,
        FileResult {
            path: destination.to_string_lossy().into_owned(),
            mime: "image/png",
            bytes,
            width,
            height,
            scale,
        },
    ))
}

fn destination_path(requested: Option<String>, prefix: &str) -> Result<PathBuf, String> {
    let path = match requested {
        Some(path) => PathBuf::from(path),
        None => {
            let directory = match std::env::var_os(OUTPUT_DIR_ENV) {
                Some(value) => PathBuf::from(value),
                None => crate::broker::runtime_home().join("outputs"),
            };
            if !directory.is_absolute() {
                return Err(format!("{OUTPUT_DIR_ENV} must be an absolute path"));
            }
            reject_symlink_components(&directory)?;
            let existed = directory.exists();
            fs::create_dir_all(&directory)
                .map_err(|error| format!("failed to create output directory: {error}"))?;
            reject_symlink_components(&directory)?;
            if !existed {
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                    .map_err(|error| format!("failed to secure output directory: {error}"))?;
            }
            directory.join(default_filename(prefix))
        }
    };
    if !path.is_absolute() {
        return Err("output path must be absolute".into());
    }
    reject_parent_components(&path)?;
    if fs::symlink_metadata(&path).is_ok() {
        return Err(format!(
            "output already exists; refusing to overwrite: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn default_filename(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{millis}-{}-{sequence}.png", std::process::id())
}

fn temporary_path(destination: &Path) -> Result<PathBuf, String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "output path has no parent directory".to_string())?;
    let filename = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "output filename must be valid UTF-8".to_string())?;
    let sequence = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".{filename}.cu-{}-{sequence}.tmp",
        std::process::id()
    )))
}

fn reject_parent_components(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "output path has no parent directory".to_string())?;
    reject_symlink_components(parent)?;
    let metadata = fs::metadata(parent)
        .map_err(|error| format!("output directory is unavailable: {error}"))?;
    if !metadata.is_dir() {
        return Err("output parent must be a directory".into());
    }
    Ok(())
}

fn reject_symlink_components(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                current.push(component.as_os_str());
            }
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err("output path must not contain '..' components".into());
            }
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "output path must not traverse symlinks: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("failed to validate output path: {error}")),
        }
    }
    Ok(())
}

fn png_dimensions(path: &Path) -> Result<(u32, u32), String> {
    let mut header = [0_u8; 24];
    let mut file = fs::File::open(path).map_err(|error| format!("failed to read PNG: {error}"))?;
    file.read_exact(&mut header)
        .map_err(|error| format!("incomplete PNG output: {error}"))?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return Err("output is not a valid PNG".into());
    }
    Ok((
        u32::from_be_bytes(header[16..20].try_into().unwrap()),
        u32::from_be_bytes(header[20..24].try_into().unwrap()),
    ))
}

fn cleanup_error(path: &Path, error: String) -> String {
    let _ = fs::remove_file(path);
    error
}
