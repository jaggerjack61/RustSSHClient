use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ssh2::{RenameFlags, Sftp};
use walkdir::WalkDir;

use crate::error::{AppError, AppResult};
use crate::models::{FileEntry, TransferDirection, TransferProgress, TransferStatus};

use super::file_tree::{format_permissions, infer_kind};

const MAX_EDITOR_BYTES: usize = 512 * 1024;

pub fn list_directory(sftp: &Sftp, path: &str) -> AppResult<Vec<FileEntry>> {
    let mut entries = sftp
        .readdir(Path::new(path))?
        .into_iter()
        .filter_map(|(entry_path, stat)| file_entry_from_dirent(entry_path, stat))
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    entries.sort_by_key(|entry| !entry.is_directory());
    Ok(entries)
}

fn file_entry_from_dirent(entry_path: PathBuf, stat: ssh2::FileStat) -> Option<FileEntry> {
    let name = entry_path.file_name()?.to_string_lossy().to_string();
    if name == "." || name == ".." {
        return None;
    }

    Some(FileEntry {
        name,
        path: normalize_remote_string(&entry_path),
        kind: infer_kind(stat.perm),
        size: stat.size.unwrap_or_default(),
        permissions: format_permissions(stat.perm),
        owner: stat.uid.map(|uid| uid.to_string()),
        modified: stat
            .mtime
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds as i64, 0)),
    })
}

pub fn ensure_remote_directory(sftp: &Sftp, path: &Path) -> AppResult<()> {
    if path.as_os_str().is_empty() || path == Path::new("/") {
        return Ok(());
    }

    let mut current = PathBuf::new();

    for component in path.components() {
        current.push(component);
        if current.as_os_str().is_empty() {
            continue;
        }

        if sftp.stat(&current).is_err() {
            sftp.mkdir(&current, 0o755)?;
        }
    }

    Ok(())
}

pub fn read_text_file(sftp: &Sftp, path: &str) -> AppResult<String> {
    let file_path = Path::new(path);
    let stat = sftp.stat(file_path)?;

    if infer_kind(stat.perm).eq(&crate::models::FileKind::Directory) {
        return Err(AppError::Sftp("Cannot open a directory in the editor.".into()));
    }

    if let Some(size) = stat.size {
        if size > MAX_EDITOR_BYTES as u64 {
            return Err(AppError::Sftp(format!(
                "File is too large to open in the editor (limit: {} KB).",
                MAX_EDITOR_BYTES / 1024,
            )));
        }
    }

    let mut source = sftp.open(file_path)?;
    let mut bytes = Vec::with_capacity(stat.size.unwrap_or_default() as usize);
    source.read_to_end(&mut bytes)?;

    if bytes.len() > MAX_EDITOR_BYTES {
        return Err(AppError::Sftp(format!(
            "File is too large to open in the editor (limit: {} KB).",
            MAX_EDITOR_BYTES / 1024,
        )));
    }

    String::from_utf8(bytes)
        .map_err(|_| AppError::Sftp("File is not valid UTF-8 text.".into()))
}

pub fn write_text_file(sftp: &Sftp, path: &str, content: &str) -> AppResult<()> {
    let file_path = Path::new(path);

    if let Ok(stat) = sftp.stat(file_path) {
        if infer_kind(stat.perm).eq(&crate::models::FileKind::Directory) {
            return Err(AppError::Sftp("Cannot save editor contents into a directory.".into()));
        }
    }

    let mut target = sftp.create(file_path)?;
    target.write_all(content.as_bytes())?;
    target.flush()?;
    Ok(())
}

pub fn rename_entry(sftp: &Sftp, source: &str, target: &str) -> AppResult<()> {
    sftp.rename(
        Path::new(source),
        Path::new(target),
        Some(RenameFlags::OVERWRITE),
    )?;
    Ok(())
}

pub fn delete_entry(sftp: &Sftp, path: &str) -> AppResult<()> {
    let stat = sftp.stat(Path::new(path))?;
    if infer_kind(stat.perm).eq(&crate::models::FileKind::Directory) {
        for (child_path, _) in sftp.readdir(Path::new(path))? {
            let name = child_path
                .file_name()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            if name == "." || name == ".." {
                continue;
            }
            delete_entry(sftp, &normalize_remote_string(&child_path))?;
        }
        sftp.rmdir(Path::new(path))?;
    } else {
        sftp.unlink(Path::new(path))?;
    }

    Ok(())
}

pub fn copy_entry<F>(
    sftp: &Sftp,
    source: &str,
    target: &str,
    transfer: &mut TransferProgress,
    mut on_progress: F,
) -> AppResult<()>
where
    F: FnMut(&TransferProgress),
{
    copy_entry_with_progress(sftp, source, target, transfer, &mut on_progress)
}

fn copy_entry_with_progress(
    sftp: &Sftp,
    source: &str,
    target: &str,
    transfer: &mut TransferProgress,
    on_progress: &mut dyn FnMut(&TransferProgress),
) -> AppResult<()> {
    let stat = sftp.stat(Path::new(source))?;
    if infer_kind(stat.perm).eq(&crate::models::FileKind::Directory) {
        ensure_remote_directory(sftp, Path::new(target))?;
        for (child_path, _) in sftp.readdir(Path::new(source))? {
            let name = child_path
                .file_name()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            if name == "." || name == ".." {
                continue;
            }

            let child_source = normalize_remote_string(&child_path);
            let child_target = format!("{}/{}", target.trim_end_matches('/'), name);
            copy_entry_with_progress(sftp, &child_source, &child_target, transfer, on_progress)?;
        }
        transfer.status = TransferStatus::Completed;
        on_progress(transfer);
        return Ok(());
    }

    let parent = Path::new(target)
        .parent()
        .ok_or_else(|| AppError::Sftp("Target path has no parent directory.".into()))?;
    ensure_remote_directory(sftp, parent)?;
    copy_file_contents(
        sftp,
        Path::new(source),
        Path::new(target),
        transfer,
        on_progress,
    )
}

pub fn upload_paths<F>(
    sftp: &Sftp,
    local_paths: &[PathBuf],
    remote_directory: &str,
    transfer: &mut TransferProgress,
    mut on_progress: F,
) -> AppResult<()>
where
    F: FnMut(&TransferProgress),
{
    let total_bytes = local_paths
        .iter()
        .map(|path| local_total_size(path))
        .sum::<AppResult<u64>>()?;
    transfer.total_bytes = total_bytes;
    transfer.status = TransferStatus::Running;
    on_progress(transfer);

    for local_path in local_paths {
        let file_name = local_path
            .file_name()
            .ok_or_else(|| AppError::Sftp("Local path has no file name.".into()))?;
        let remote_path = Path::new(remote_directory).join(file_name);
        upload_single_path(sftp, local_path, &remote_path, transfer, &mut on_progress)?;
    }

    transfer.status = TransferStatus::Completed;
    on_progress(transfer);
    Ok(())
}

pub fn download_entry<F>(
    sftp: &Sftp,
    remote_path: &str,
    local_directory: &Path,
    transfer: &mut TransferProgress,
    mut on_progress: F,
) -> AppResult<()>
where
    F: FnMut(&TransferProgress),
{
    let total_bytes = remote_total_size(sftp, Path::new(remote_path))?;
    transfer.total_bytes = total_bytes;
    transfer.status = TransferStatus::Running;
    on_progress(transfer);

    let name = Path::new(remote_path)
        .file_name()
        .ok_or_else(|| AppError::Sftp("Remote path has no file name.".into()))?;
    let target_path = local_directory.join(name);
    download_single_path(
        sftp,
        Path::new(remote_path),
        &target_path,
        transfer,
        &mut on_progress,
    )?;

    transfer.status = TransferStatus::Completed;
    on_progress(transfer);
    Ok(())
}

pub fn queued_transfer(
    label: impl Into<String>,
    direction: TransferDirection,
    total_bytes: u64,
) -> TransferProgress {
    TransferProgress::queued(label, direction, total_bytes)
}

fn upload_single_path(
    sftp: &Sftp,
    local_path: &Path,
    remote_path: &Path,
    transfer: &mut TransferProgress,
    on_progress: &mut dyn FnMut(&TransferProgress),
) -> AppResult<()> {
    if local_path.is_dir() {
        ensure_remote_directory(sftp, remote_path)?;
        for entry in fs::read_dir(local_path)? {
            let entry = entry?;
            upload_single_path(
                sftp,
                &entry.path(),
                &remote_path.join(entry.file_name()),
                transfer,
                on_progress,
            )?;
        }
        return Ok(());
    }

    let parent = remote_path.parent().ok_or_else(|| {
        AppError::Sftp("Remote upload target must have a parent directory.".into())
    })?;
    ensure_remote_directory(sftp, parent)?;

    let mut source = File::open(local_path)?;
    let mut target = sftp.create(remote_path)?;
    stream_copy(&mut source, &mut target, transfer, on_progress)
}

fn download_single_path(
    sftp: &Sftp,
    remote_path: &Path,
    local_path: &Path,
    transfer: &mut TransferProgress,
    on_progress: &mut dyn FnMut(&TransferProgress),
) -> AppResult<()> {
    let stat = sftp.stat(remote_path)?;
    if infer_kind(stat.perm).eq(&crate::models::FileKind::Directory) {
        fs::create_dir_all(local_path)?;
        for (child_path, _) in sftp.readdir(remote_path)? {
            let name = child_path
                .file_name()
                .map(|value| value.to_os_string())
                .unwrap_or_default();
            if name == "." || name == ".." {
                continue;
            }
            download_single_path(
                sftp,
                &child_path,
                &local_path.join(name),
                transfer,
                on_progress,
            )?;
        }
        return Ok(());
    }

    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut source = sftp.open(remote_path)?;
    let mut target = File::create(local_path)?;
    stream_copy(&mut source, &mut target, transfer, on_progress)
}

fn copy_file_contents(
    sftp: &Sftp,
    source_path: &Path,
    target_path: &Path,
    transfer: &mut TransferProgress,
    on_progress: &mut dyn FnMut(&TransferProgress),
) -> AppResult<()> {
    let mut source = sftp.open(source_path)?;
    let mut target = sftp.create(target_path)?;
    stream_copy(&mut source, &mut target, transfer, on_progress)
}

fn stream_copy<R, W>(
    reader: &mut R,
    writer: &mut W,
    transfer: &mut TransferProgress,
    on_progress: &mut dyn FnMut(&TransferProgress),
) -> AppResult<()>
where
    R: Read,
    W: Write,
{
    let mut buffer = [0_u8; 32 * 1024];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        writer.write_all(&buffer[..read])?;
        transfer.transferred_bytes = transfer.transferred_bytes.saturating_add(read as u64);
        transfer.status = TransferStatus::Running;
        on_progress(transfer);
    }

    writer.flush()?;
    Ok(())
}

fn local_total_size(path: &Path) -> AppResult<u64> {
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }

    WalkDir::new(path)
        .into_iter()
        .try_fold(0_u64, |accumulator, entry| {
            let entry = entry.map_err(std::io::Error::other).map_err(AppError::Io)?;
            if entry.path().is_file() {
                let metadata = entry
                    .metadata()
                    .map_err(std::io::Error::other)
                    .map_err(AppError::Io)?;
                Ok(accumulator + metadata.len())
            } else {
                Ok(accumulator)
            }
        })
}

fn remote_total_size(sftp: &Sftp, path: &Path) -> AppResult<u64> {
    let stat = sftp.stat(path)?;
    if !infer_kind(stat.perm).eq(&crate::models::FileKind::Directory) {
        return Ok(stat.size.unwrap_or_default());
    }

    let mut total = 0_u64;
    for (child_path, _) in sftp.readdir(path)? {
        let name = child_path
            .file_name()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default();
        if name == "." || name == ".." {
            continue;
        }
        total = total.saturating_add(remote_total_size(sftp, &child_path)?);
    }
    Ok(total)
}

fn normalize_remote_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
