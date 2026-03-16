use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use tracing::{error, info, warn};

use crate::error::{AppError, AppResult};
use crate::models::{
    FileEntry, LoginRequest, SshKeyRecord, TransferDirection, TransferProgress, TransferStatus,
};
use crate::sftp::client as sftp_client;
use crate::ssh::client;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    SendInput(Vec<u8>),
    RefreshDirectory(Option<String>),
    LoadDirectoryChildren(String),
    ReadFile {
        remote_path: String,
    },
    WriteFile {
        remote_path: String,
        contents: String,
    },
    Upload {
        local_paths: Vec<PathBuf>,
        remote_directory: String,
    },
    Download {
        remote_path: String,
        local_directory: PathBuf,
    },
    Delete {
        remote_path: String,
    },
    Rename {
        source: String,
        target: String,
    },
    Copy {
        source: String,
        target: String,
    },
    Disconnect,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    Connected {
        cwd: String,
        latency_ms: u128,
        peer: String,
    },
    Output(Vec<u8>),
    DirectoryLoaded {
        cwd: String,
        entries: Vec<FileEntry>,
    },
    DirectoryChildrenLoaded {
        directory: String,
        entries: Vec<FileEntry>,
    },
    FileOpened {
        path: String,
        contents: String,
    },
    FileOpenFailed {
        path: String,
        error: String,
    },
    FileSaved {
        path: String,
    },
    FileSaveFailed {
        path: String,
        error: String,
    },
    DirectoryOpenFailed {
        path: String,
        error: String,
    },
    DirectoryChildrenLoadFailed {
        directory: String,
        error: String,
    },
    Transfer(TransferProgress),
    Error(String),
    Disconnected(String),
}

#[derive(Debug, Clone)]
pub struct SessionHandle {
    commands: Sender<SessionCommand>,
    events: Receiver<SessionEvent>,
}

impl SessionHandle {
    pub fn send(&self, command: SessionCommand) -> AppResult<()> {
        self.commands
            .send(command)
            .map_err(|error| AppError::Configuration(error.to_string()))
    }

    pub fn try_recv(&self) -> Option<SessionEvent> {
        self.events.try_recv().ok()
    }

    #[cfg(test)]
    pub(crate) fn from_channels(
        commands: Sender<SessionCommand>,
        events: Receiver<SessionEvent>,
    ) -> Self {
        Self { commands, events }
    }
}

pub fn spawn(request: LoginRequest, key: Option<SshKeyRecord>) -> AppResult<SessionHandle> {
    let (command_tx, command_rx) = unbounded();
    let (event_tx, event_rx) = unbounded();

    thread::Builder::new()
        .name("rustssh-session".into())
        .spawn(move || {
            if let Err(error) = run_session(request, key, command_rx, event_tx.clone()) {
                error!(error = %error, "session worker failed");
                let _ = event_tx.send(SessionEvent::Error(error.to_string()));
                let _ = event_tx.send(SessionEvent::Disconnected("Connection closed".into()));
            }
        })
        .map_err(|error| AppError::Configuration(error.to_string()))?;

    Ok(SessionHandle {
        commands: command_tx,
        events: event_rx,
    })
}

fn run_session(
    request: LoginRequest,
    key: Option<SshKeyRecord>,
    command_rx: Receiver<SessionCommand>,
    event_tx: Sender<SessionEvent>,
) -> AppResult<()> {
    let started_at = Instant::now();
    let session = client::connect_session(&request, key.as_ref())?;

    // Keep session in blocking mode for all setup operations
    let sftp = session.sftp()?;
    let mut cwd = client::resolve_home_directory(&session)?;
    let mut shell = session.channel_session()?;
    shell.handle_extended_data(ssh2::ExtendedData::Merge)?;
    shell.request_pty("xterm", None, Some((140, 48, 0, 0)))?;
    shell.shell()?;

    let latency_ms = started_at.elapsed().as_millis();
    let _ = event_tx.send(SessionEvent::Connected {
        cwd: cwd.clone(),
        latency_ms,
        peer: request.host.clone(),
    });

    refresh_directory_or_report(&sftp, &cwd, &event_tx);

    // Switch to non-blocking only for the main read loop
    session.set_blocking(false);

    let mut buffer = [0_u8; 16 * 1024];
    loop {
        // SFTP operations require blocking I/O — switch to blocking while
        // processing queued commands, then back to non-blocking for the
        // shell read that follows.
        session.set_blocking(true);
        while let Ok(command) = command_rx.try_recv() {
            match command {
                SessionCommand::SendInput(bytes) => {
                    shell.write_all(&bytes)?;
                    shell.flush()?;
                }
                SessionCommand::RefreshDirectory(path_hint) => {
                    let next = match path_hint {
                        Some(value) if value.starts_with("~/") => {
                            match client::resolve_home_directory(&session) {
                                Ok(home) => {
                                    let sub = value.strip_prefix("~/").unwrap_or("");
                                    format!("{}/{}", home.trim_end_matches('/'), sub)
                                }
                                Err(error) => {
                                    emit_session_error(&event_tx, error.to_string());
                                    continue;
                                }
                            }
                        }
                        Some(value) => value,
                        None => match client::resolve_home_directory(&session) {
                            Ok(path) => path,
                            Err(error) => {
                                emit_session_error(&event_tx, error.to_string());
                                continue;
                            }
                        },
                    };

                    if refresh_directory_or_report(&sftp, &next, &event_tx) {
                        cwd = next;
                    }
                }
                SessionCommand::LoadDirectoryChildren(directory) => {
                    match sftp_client::list_directory(&sftp, &directory) {
                        Ok(entries) => {
                            let _ = event_tx.send(SessionEvent::DirectoryChildrenLoaded {
                                directory,
                                entries,
                            });
                        }
                        Err(error) => {
                            let _ = event_tx.send(SessionEvent::DirectoryChildrenLoadFailed {
                                directory,
                                error: error.to_string(),
                            });
                        }
                    }
                }
                SessionCommand::ReadFile { remote_path } => {
                    match sftp_client::read_text_file(&sftp, &remote_path) {
                        Ok(contents) => {
                            let _ = event_tx.send(SessionEvent::FileOpened {
                                path: remote_path,
                                contents,
                            });
                        }
                        Err(error) => {
                            let _ = event_tx.send(SessionEvent::FileOpenFailed {
                                path: remote_path,
                                error: error.to_string(),
                            });
                        }
                    }
                }
                SessionCommand::WriteFile {
                    remote_path,
                    contents,
                } => {
                    match sftp_client::write_text_file(&sftp, &remote_path, &contents) {
                        Ok(()) => {
                            let _ = event_tx.send(SessionEvent::FileSaved { path: remote_path });
                            refresh_directory_or_report(&sftp, &cwd, &event_tx);
                        }
                        Err(error) => {
                            let _ = event_tx.send(SessionEvent::FileSaveFailed {
                                path: remote_path,
                                error: error.to_string(),
                            });
                        }
                    }
                }
                SessionCommand::Upload {
                    local_paths,
                    remote_directory,
                } => {
                    spawn_transfer_worker(
                        request.clone(),
                        key.clone(),
                        event_tx.clone(),
                        TransferDirection::Upload,
                        file_label(&local_paths),
                        move |sftp, transfer, on_progress| {
                            sftp_client::upload_paths(
                                sftp,
                                &local_paths,
                                &remote_directory,
                                transfer,
                                on_progress,
                            )
                        },
                    );
                }
                SessionCommand::Download {
                    remote_path,
                    local_directory,
                } => {
                    spawn_transfer_worker(
                        request.clone(),
                        key.clone(),
                        event_tx.clone(),
                        TransferDirection::Download,
                        Path::new(&remote_path)
                            .file_name()
                            .map(|value| value.to_string_lossy().to_string())
                            .unwrap_or_else(|| remote_path.clone()),
                        move |sftp, transfer, on_progress| {
                            sftp_client::download_entry(
                                sftp,
                                &remote_path,
                                &local_directory,
                                transfer,
                                on_progress,
                            )
                        },
                    );
                }
                SessionCommand::Delete { remote_path } => {
                    match sftp_client::delete_entry(&sftp, &remote_path) {
                        Ok(()) => {
                            refresh_directory_or_report(&sftp, &cwd, &event_tx);
                        }
                        Err(error) => {
                            emit_session_error(
                                &event_tx,
                                format!("Unable to delete {remote_path}: {error}"),
                            );
                        }
                    }
                }
                SessionCommand::Rename { source, target } => {
                    match sftp_client::rename_entry(&sftp, &source, &target) {
                        Ok(()) => {
                            refresh_directory_or_report(&sftp, &cwd, &event_tx);
                        }
                        Err(error) => {
                            emit_session_error(
                                &event_tx,
                                format!("Unable to rename {source} to {target}: {error}"),
                            );
                        }
                    }
                }
                SessionCommand::Copy { source, target } => {
                    spawn_transfer_worker(
                        request.clone(),
                        key.clone(),
                        event_tx.clone(),
                        TransferDirection::Copy,
                        Path::new(&source)
                            .file_name()
                            .map(|value| value.to_string_lossy().to_string())
                            .unwrap_or_else(|| source.clone()),
                        move |sftp, transfer, on_progress| {
                            sftp_client::copy_entry(sftp, &source, &target, transfer, on_progress)
                        },
                    );
                }
                SessionCommand::Disconnect => {
                    let _ = shell.close();
                    info!(host = %request.host, "session closed by user");
                    let _ = event_tx.send(SessionEvent::Disconnected("Disconnected".into()));
                    return Ok(());
                }
            }
        }
        session.set_blocking(false);

        match shell.read(&mut buffer) {
            Ok(read) if read > 0 => {
                let _ = event_tx.send(SessionEvent::Output(buffer[..read].to_vec()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => {
                warn!(error = %error, "terminal stream closed");
                let _ = event_tx.send(SessionEvent::Disconnected(error.to_string()));
                return Ok(());
            }
        }

        if shell.eof() {
            let _ = event_tx.send(SessionEvent::Disconnected("Remote shell ended".into()));
            return Ok(());
        }

        thread::sleep(Duration::from_millis(16));
    }
}

fn refresh_directory(
    sftp: &ssh2::Sftp,
    directory: &str,
    event_tx: &Sender<SessionEvent>,
) -> AppResult<()> {
    let entries = sftp_client::list_directory(sftp, directory)?;
    let _ = event_tx.send(SessionEvent::DirectoryLoaded {
        cwd: directory.to_string(),
        entries,
    });
    Ok(())
}

fn refresh_directory_or_report(
    sftp: &ssh2::Sftp,
    directory: &str,
    event_tx: &Sender<SessionEvent>,
) -> bool {
    match refresh_directory(sftp, directory, event_tx) {
        Ok(()) => true,
        Err(error) => {
            let _ = event_tx.send(SessionEvent::DirectoryOpenFailed {
                path: directory.to_string(),
                error: error.to_string(),
            });
            false
        }
    }
}

fn emit_session_error(event_tx: &Sender<SessionEvent>, message: String) {
    let _ = event_tx.send(SessionEvent::Error(message));
}

fn spawn_transfer_worker<F>(
    request: LoginRequest,
    key: Option<SshKeyRecord>,
    event_tx: Sender<SessionEvent>,
    direction: TransferDirection,
    label: String,
    operation: F,
) where
    F: FnOnce(
            &ssh2::Sftp,
            &mut TransferProgress,
            &mut dyn FnMut(&TransferProgress),
        ) -> AppResult<()>
        + Send
        + 'static,
{
    thread::spawn(move || {
        let mut transfer = sftp_client::queued_transfer(label, direction, 0);
        let _ = event_tx.send(SessionEvent::Transfer(transfer.clone()));

        let result = (|| -> AppResult<()> {
            let session = client::connect_session(&request, key.as_ref())?;
            let sftp = session.sftp()?;
            let mut progress_sender = |progress: &TransferProgress| {
                let _ = event_tx.send(SessionEvent::Transfer(progress.clone()));
            };
            operation(&sftp, &mut transfer, &mut progress_sender)?;
            Ok(())
        })();

        if let Err(error) = result {
            transfer.status = TransferStatus::Failed(error.to_string());
            let _ = event_tx.send(SessionEvent::Transfer(transfer));
            let _ = event_tx.send(SessionEvent::Error(error.to_string()));
        }
    });
}

fn file_label(paths: &[PathBuf]) -> String {
    if paths.len() == 1 {
        return paths[0]
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "upload".into());
    }

    format!("{} files", paths.len())
}
