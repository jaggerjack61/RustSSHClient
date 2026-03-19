use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ssh2::Sftp;
use walkdir::WalkDir;
use tracing::warn;

use crate::error::{AppError, AppResult};
use crate::models::{FileEntry, FileKind, TransferDirection, TransferProgress, TransferStatus};

use super::file_tree::{format_permissions, infer_kind};

const MAX_EDITOR_BYTES: usize = 512 * 1024;
const MAX_REMOTE_RECURSION_DEPTH: usize = 64;

pub fn list_directory(sftp: &Sftp, path: &str) -> AppResult<Vec<FileEntry>> {
    let mut entries = sftp
        .readdir(Path::new(path))?
        .into_iter()
        .filter_map(|(entry_path, stat)| file_entry_from_dirent(sftp, entry_path, stat))
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    entries.sort_by_key(|entry| !entry.is_directory());
    Ok(entries)
}

fn file_entry_from_dirent(sftp: &Sftp, entry_path: PathBuf, stat: ssh2::FileStat) -> Option<FileEntry> {
    let name = entry_path.file_name()?.to_str()?.to_string();
    if name == "." || name == ".." {
        return None;
    }

    let path = match remote_path_string(&entry_path) {
        Ok(path) => path,
        Err(error) => {
            warn!(error = %error, "Skipping remote entry with non-UTF-8 path");
            return None;
        }
    };

    Some(FileEntry {
        name,
        path,
        kind: remote_kind(sftp, &entry_path, stat.perm),
        size: stat.size.unwrap_or_default(),
        permissions: format_permissions(stat.perm),
        owner: stat.uid.map(|uid| uid.to_string()),
        modified: stat
            .mtime
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds as i64, 0)),
    })
}

pub fn ensure_remote_directory(sftp: &Sftp, path: &Path) -> AppResult<()> {
    let remote_path = remote_path_string(path)?;
    if remote_path.is_empty() || remote_path == "/" {
        return Ok(());
    }

    let mut current = String::new();
    let absolute = remote_path.starts_with('/');

    for segment in remote_path.split('/') {
        if segment.is_empty() {
            continue;
        }

        if absolute {
            if current.is_empty() {
                current.push('/');
            } else if !current.ends_with('/') {
                current.push('/');
            }
        } else if !current.is_empty() {
            current.push('/');
        }

        current.push_str(segment);
        let current_path = Path::new(&current);
        if sftp.stat(current_path).is_err() {
            sftp.mkdir(current_path, 0o755)?;
        }
    }

    Ok(())
}

pub fn read_text_file(sftp: &Sftp, path: &str) -> AppResult<String> {
    let file_path = Path::new(path);
    let stat = sftp.stat(file_path)?;

    if remote_kind(sftp, file_path, stat.perm).eq(&FileKind::Directory) {
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
        if remote_kind(sftp, file_path, stat.perm).eq(&FileKind::Directory) {
            return Err(AppError::Sftp("Cannot save editor contents into a directory.".into()));
        }
    }

    let mut target = sftp.create(file_path)?;
    target.write_all(content.as_bytes())?;
    target.flush()?;
    Ok(())
}

pub fn rename_entry(sftp: &Sftp, source: &str, target: &str) -> AppResult<()> {
    sftp.rename(Path::new(source), Path::new(target), None)?;
    Ok(())
}

pub fn delete_entry(sftp: &Sftp, path: &str) -> AppResult<()> {
    delete_entry_with_depth(sftp, path, 0)
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
    copy_entry_with_progress(sftp, source, target, transfer, &mut on_progress, 0)
}

fn copy_entry_with_progress(
    sftp: &Sftp,
    source: &str,
    target: &str,
    transfer: &mut TransferProgress,
    on_progress: &mut dyn FnMut(&TransferProgress),
    depth: usize,
) -> AppResult<()> {
    ensure_recursion_budget(depth)?;
    let stat = sftp.stat(Path::new(source))?;
    if remote_kind(sftp, Path::new(source), stat.perm).eq(&FileKind::Directory) {
        ensure_remote_directory(sftp, Path::new(target))?;
        for (child_path, _) in sftp.readdir(Path::new(source))? {
            let Some(name) = child_path.file_name().and_then(|value| value.to_str()) else {
                return Err(AppError::Sftp(
                    "Remote directory entry contains non-UTF-8 text and cannot be copied safely.".into(),
                ));
            };
            if name == "." || name == ".." {
                continue;
            }

            let child_source = remote_path_string(&child_path)?;
            let child_target = format!("{}/{}", target.trim_end_matches('/'), name);
            copy_entry_with_progress(
                sftp,
                &child_source,
                &child_target,
                transfer,
                on_progress,
                depth + 1,
            )?;
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
        upload_single_path(sftp, local_path, &remote_path, transfer, &mut on_progress, 0)?;
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
        0,
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
    depth: usize,
) -> AppResult<()> {
    ensure_recursion_budget(depth)?;
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
                depth + 1,
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
    depth: usize,
) -> AppResult<()> {
    ensure_recursion_budget(depth)?;
    let stat = sftp.stat(remote_path)?;
    if remote_kind(sftp, remote_path, stat.perm).eq(&FileKind::Directory) {
        fs::create_dir_all(local_path)?;
        for (child_path, _) in sftp.readdir(remote_path)? {
            let Some(name) = child_path.file_name().and_then(|value| value.to_str()) else {
                return Err(AppError::Sftp(
                    "Remote directory entry contains non-UTF-8 text and cannot be downloaded safely.".into(),
                ));
            };
            if name == "." || name == ".." {
                continue;
            }
            download_single_path(
                sftp,
                &child_path,
                &local_path.join(name),
                transfer,
                on_progress,
                depth + 1,
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
    remote_total_size_with_depth(sftp, path, 0)
}

fn remote_kind(sftp: &Sftp, path: &Path, perm: Option<u32>) -> FileKind {
    let kind = infer_kind(perm);
    if kind != FileKind::Other || perm.is_some() {
        return kind;
    }

    if sftp.opendir(path).is_ok() {
        FileKind::Directory
    } else {
        FileKind::File
    }
}

fn delete_entry_with_depth(sftp: &Sftp, path: &str, depth: usize) -> AppResult<()> {
    ensure_recursion_budget(depth)?;
    let stat = sftp.stat(Path::new(path))?;
    if remote_kind(sftp, Path::new(path), stat.perm).eq(&FileKind::Directory) {
        for (child_path, _) in sftp.readdir(Path::new(path))? {
            let Some(name) = child_path.file_name().and_then(|value| value.to_str()) else {
                return Err(AppError::Sftp(
                    "Remote directory entry contains non-UTF-8 text and cannot be deleted safely.".into(),
                ));
            };
            if name == "." || name == ".." {
                continue;
            }
            delete_entry_with_depth(sftp, &remote_path_string(&child_path)?, depth + 1)?;
        }
        sftp.rmdir(Path::new(path))?;
    } else {
        sftp.unlink(Path::new(path))?;
    }

    Ok(())
}

fn remote_total_size_with_depth(sftp: &Sftp, path: &Path, depth: usize) -> AppResult<u64> {
    ensure_recursion_budget(depth)?;
    let stat = sftp.stat(path)?;
    if !remote_kind(sftp, path, stat.perm).eq(&FileKind::Directory) {
        return Ok(stat.size.unwrap_or_default());
    }

    let mut total = 0_u64;
    for (child_path, _) in sftp.readdir(path)? {
        let Some(name) = child_path.file_name().and_then(|value| value.to_str()) else {
            return Err(AppError::Sftp(
                "Remote directory entry contains non-UTF-8 text and cannot be sized safely.".into(),
            ));
        };
        if name == "." || name == ".." {
            continue;
        }
        total = total.saturating_add(remote_total_size_with_depth(sftp, &child_path, depth + 1)?);
    }
    Ok(total)
}

fn remote_path_string(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| {
            AppError::Sftp(
                "Remote path contains non-UTF-8 text and cannot be manipulated safely.".into(),
            )
        })
}

fn ensure_recursion_budget(depth: usize) -> AppResult<()> {
    if depth > MAX_REMOTE_RECURSION_DEPTH {
        return Err(AppError::Sftp(format!(
            "Remote directory recursion exceeded the safety limit of {} levels.",
            MAX_REMOTE_RECURSION_DEPTH
        )));
    }

    Ok(())
}
