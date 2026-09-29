use std::hash::{Hash, Hasher};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError, unbounded};
use iced::Subscription;
use iced::futures::channel::mpsc;
use iced::futures::stream::{self, StreamExt};
use tracing::{error, info, warn};

use crate::error::{AppError, AppResult};
use crate::models::{
    FileEntry, LoginRequest, SshKeyRecord, TransferDirection, TransferProgress, TransferStatus,
};
use crate::sftp::client as sftp_client;
use crate::ssh::client::{self, HostKeyInfo, HostKeyPolicy};

pub const MAX_CONCURRENT_TRANSFERS: usize = 4;

pub type EventReceiver = mpsc::UnboundedReceiver<SessionEvent>;

/// `Disconnected` reasons for sessions that ended normally.
pub const DISCONNECTED_BY_USER: &str = "Disconnected";
pub const REMOTE_SHELL_EXITED: &str = "The remote shell exited";

/// Upper bound on terminal bytes coalesced into one UI event.
const MAX_OUTPUT_CHUNK: usize = 64 * 1024;
/// Shortest and longest idle waits of the shell loop. The loop wakes
/// immediately for commands, so these only bound output latency.
const MIN_IDLE_WAIT: Duration = Duration::from_millis(2);
const MAX_IDLE_WAIT: Duration = Duration::from_millis(25);
const HOST_KEY_DECISION_TIMEOUT: Duration = Duration::from_secs(300);

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static TRANSFER_SLOTS: TransferSlots = TransferSlots::new(MAX_CONCURRENT_TRANSFERS);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    SendInput(Vec<u8>),
    /// Lists a directory and makes it the explorer root. `~` and `~/...`
    /// resolve against the remote home directory.
    RefreshDirectory(String),
    LoadDirectoryChildren(String),
    ReadFile {
        remote_path: String,
    },
    WriteFile {
        remote_path: String,
        contents: String,
    },
    ResizeTerminal {
        cols: u32,
        rows: u32,
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
    CreateDirectory {
        remote_path: String,
    },
    HostKeyDecision(bool),
    Disconnect,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    HostKeyUnknown(HostKeyInfo),
    Connected {
        cwd: String,
        latency_ms: u128,
        peer: String,
    },
    Output(Vec<u8>),
    DirectoryLoaded {
        /// The path exactly as it was requested (may be `~`-relative).
        request: String,
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
        request: String,
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

/// Everything needed to open a new connection to the same server, shared by
/// the interactive session and its transfer workers.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub request: LoginRequest,
    pub key: Option<SshKeyRecord>,
    pub known_hosts: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SessionHandle {
    id: u64,
    commands: Sender<SessionCommand>,
    events: EventSource,
}

/// The receiving half of a session's event channel, handed to exactly one
/// subscription. Identity (and therefore subscription lifetime) is the
/// session id.
#[derive(Debug, Clone)]
struct EventSource {
    id: u64,
    receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<SessionEvent>>>>,
}

impl Hash for EventSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl SessionHandle {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn send(&self, command: SessionCommand) -> AppResult<()> {
        self.commands
            .send(command)
            .map_err(|_| AppError::Ssh("The SSH session is no longer running.".into()))
    }

    /// Takes the raw event receiver, for driving a session without the UI
    /// (integration tests). The subscription yields nothing afterwards.
    #[doc(hidden)]
    pub fn take_event_receiver(&self) -> Option<EventReceiver> {
        self.events
            .receiver
            .lock()
            .ok()
            .and_then(|mut receiver| receiver.take())
    }

    /// Streams this session's events. The stream ends when the worker exits.
    pub fn events(&self) -> Subscription<SessionEvent> {
        Subscription::run_with(self.events.clone(), |source| {
            let receiver = source
                .receiver
                .lock()
                .ok()
                .and_then(|mut receiver| receiver.take());
            stream::iter(receiver).flatten()
        })
    }

    /// Creates a handle that isn't backed by a connection: commands go to
    /// `commands` and events can be injected with the returned sender. Used
    /// by tests and the screenshot tool.
    #[doc(hidden)]
    pub fn from_channels(
        commands: Sender<SessionCommand>,
    ) -> (Self, mpsc::UnboundedSender<SessionEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded();
        let id = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
        (
            Self {
                id,
                commands,
                events: EventSource {
                    id,
                    receiver: Arc::new(Mutex::new(Some(event_rx))),
                },
            },
            event_tx,
        )
    }
}

#[derive(Clone)]
struct EventSink(mpsc::UnboundedSender<SessionEvent>);

impl EventSink {
    fn send(&self, event: SessionEvent) {
        let _ = self.0.unbounded_send(event);
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

pub fn spawn(config: ConnectionConfig) -> AppResult<SessionHandle> {
    let (command_tx, command_rx) = unbounded();
    let (event_tx, event_rx) = mpsc::unbounded();
    let events = EventSink(event_tx);
    let id = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);

    thread::Builder::new()
        .name(format!("rustssh-session-{id}"))
        .spawn(move || {
            if let Err(error) = run_session(&config, &command_rx, &events) {
                error!(error = %error, "session worker failed");
                events.send(SessionEvent::Disconnected(error.to_string()));
            }
        })
        .map_err(|error| AppError::Configuration(error.to_string()))?;

    Ok(SessionHandle {
        id,
        commands: command_tx,
        events: EventSource {
            id,
            receiver: Arc::new(Mutex::new(Some(event_rx))),
        },
    })
}

fn run_session(
    config: &ConnectionConfig,
    command_rx: &Receiver<SessionCommand>,
    events: &EventSink,
) -> AppResult<()> {
    let started_at = Instant::now();
    // Time spent waiting for the user is not connection latency.
    let mut waited_for_user = Duration::ZERO;
    let mut ask_user = |info: &HostKeyInfo| {
        let asked_at = Instant::now();
        let trusted = await_host_key_decision(info, command_rx, events);
        waited_for_user += asked_at.elapsed();
        trusted
    };
    let session = client::connect_session(
        &config.request,
        config.key.as_ref(),
        HostKeyPolicy::Verify {
            known_hosts: &config.known_hosts,
            on_unknown: &mut ask_user,
        },
    )?;

    // Keep the session blocking for setup; the shell loop below switches to
    // non-blocking reads.
    let sftp = session.sftp()?;
    let home = client::resolve_home_directory(&sftp)?;
    let mut cwd = home.clone();
    let mut shell = session.channel_session()?;
    shell.handle_extended_data(ssh2::ExtendedData::Merge)?;
    shell.request_pty("xterm-256color", None, Some((120, 36, 0, 0)))?;
    shell.shell()?;

    events.send(SessionEvent::Connected {
        cwd: cwd.clone(),
        latency_ms: started_at
            .elapsed()
            .saturating_sub(waited_for_user)
            .as_millis(),
        peer: config.request.host.trim().to_string(),
    });
    refresh_directory_or_report(&sftp, &cwd, &cwd, events);

    session.set_blocking(false);
    let mut blocking = false;
    let mut buffer = vec![0_u8; 32 * 1024];
    let mut output = Vec::with_capacity(MAX_OUTPUT_CHUNK);
    let mut idle_wait = MIN_IDLE_WAIT;
    let mut next_keepalive = Instant::now();
    let mut woken_by: Option<SessionCommand> = None;

    loop {
        // --- Commands --------------------------------------------------
        let mut handled_command = false;
        loop {
            let command = match woken_by
                .take()
                .map(Ok)
                .unwrap_or_else(|| command_rx.try_recv())
            {
                Ok(command) => command,
                Err(TryRecvError::Empty) => break,
                // The UI dropped its handle; tear the session down.
                Err(TryRecvError::Disconnected) => return close_shell(&session, &mut shell),
            };
            handled_command = true;

            // SFTP and channel writes need blocking I/O.
            if !blocking {
                session.set_blocking(true);
                blocking = true;
            }

            match command {
                SessionCommand::Disconnect => {
                    let _ = shell.close();
                    info!(host = %config.request.host, "session closed by user");
                    events.send(SessionEvent::Disconnected(DISCONNECTED_BY_USER.into()));
                    return Ok(());
                }
                SessionCommand::SendInput(bytes) => {
                    // No `flush()`: on an ssh2 channel it discards unread
                    // *incoming* data and corrupts the receive window.
                    shell.write_all(&bytes)?;
                }
                SessionCommand::ResizeTerminal { cols, rows } => {
                    shell.request_pty_size(cols, rows, None, None)?;
                }
                SessionCommand::RefreshDirectory(request) => {
                    let target = resolve_home_relative(&request, &home);
                    if refresh_directory_or_report(&sftp, &request, &target, events) {
                        cwd = target;
                    }
                }
                other => handle_file_command(other, config, &sftp, &cwd, events),
            }
        }

        if blocking {
            session.set_blocking(false);
            blocking = false;
        }

        // --- Shell output ----------------------------------------------
        loop {
            match shell.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    output.extend_from_slice(&buffer[..read]);
                    if output.len() >= MAX_OUTPUT_CHUNK {
                        break;
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => {
                    warn!(error = %error, "terminal stream closed");
                    flush_output(&mut output, events);
                    events.send(SessionEvent::Disconnected(format!(
                        "Connection lost: {error}"
                    )));
                    return Ok(());
                }
            }
        }

        let produced_output = !output.is_empty();
        flush_output(&mut output, events);

        if shell.eof() {
            events.send(SessionEvent::Disconnected(REMOTE_SHELL_EXITED.into()));
            return Ok(());
        }

        if events.is_closed() {
            return close_shell(&session, &mut shell);
        }

        // libssh2 only sends keepalives when asked to.
        if Instant::now() >= next_keepalive {
            match session.keepalive_send() {
                Ok(seconds) => {
                    next_keepalive =
                        Instant::now() + Duration::from_secs(u64::from(seconds.max(1)));
                }
                // LIBSSH2_ERROR_EAGAIN: retry on the next iteration.
                Err(error) if error.code() == ssh2::ErrorCode::Session(-37) => {}
                Err(error) => {
                    warn!(error = %error, "keepalive failed");
                    next_keepalive = Instant::now() + Duration::from_secs(5);
                }
            }
        }

        // --- Idle wait ---------------------------------------------------
        if produced_output || handled_command {
            idle_wait = MIN_IDLE_WAIT;
            continue;
        }

        // Wake immediately when the UI sends something; otherwise back off
        // gradually so an idle session costs next to nothing.
        match command_rx.recv_timeout(idle_wait) {
            Ok(command) => {
                woken_by = Some(command);
                idle_wait = MIN_IDLE_WAIT;
            }
            Err(RecvTimeoutError::Timeout) => {
                idle_wait = (idle_wait * 2).min(MAX_IDLE_WAIT);
            }
            Err(RecvTimeoutError::Disconnected) => return close_shell(&session, &mut shell),
        }
    }
}

fn close_shell(session: &ssh2::Session, shell: &mut ssh2::Channel) -> AppResult<()> {
    session.set_blocking(true);
    let _ = shell.close();
    Ok(())
}

fn flush_output(output: &mut Vec<u8>, events: &EventSink) {
    if !output.is_empty() {
        events.send(SessionEvent::Output(std::mem::take(output)));
        output.reserve(MAX_OUTPUT_CHUNK);
    }
}

/// Blocks the connecting worker until the UI accepts or rejects an unknown
/// host key.
fn await_host_key_decision(
    info: &HostKeyInfo,
    command_rx: &Receiver<SessionCommand>,
    events: &EventSink,
) -> bool {
    events.send(SessionEvent::HostKeyUnknown(info.clone()));
    let deadline = Instant::now() + HOST_KEY_DECISION_TIMEOUT;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match command_rx.recv_timeout(remaining) {
            Ok(SessionCommand::HostKeyDecision(accepted)) => return accepted,
            Ok(SessionCommand::Disconnect) => return false,
            // Anything else (e.g. a resize) is irrelevant before the shell exists.
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
}

fn handle_file_command(
    command: SessionCommand,
    config: &ConnectionConfig,
    sftp: &ssh2::Sftp,
    cwd: &str,
    events: &EventSink,
) {
    match command {
        SessionCommand::LoadDirectoryChildren(directory) => {
            match sftp_client::list_directory(sftp, &directory) {
                Ok(entries) => {
                    events.send(SessionEvent::DirectoryChildrenLoaded { directory, entries })
                }
                Err(error) => events.send(SessionEvent::DirectoryChildrenLoadFailed {
                    directory,
                    error: error.to_string(),
                }),
            }
        }
        SessionCommand::ReadFile { remote_path } => {
            match sftp_client::read_text_file(sftp, &remote_path) {
                Ok(contents) => events.send(SessionEvent::FileOpened {
                    path: remote_path,
                    contents,
                }),
                Err(error) => events.send(SessionEvent::FileOpenFailed {
                    path: remote_path,
                    error: error.to_string(),
                }),
            }
        }
        SessionCommand::WriteFile {
            remote_path,
            contents,
        } => match sftp_client::write_text_file(sftp, &remote_path, &contents) {
            Ok(()) => {
                events.send(SessionEvent::FileSaved { path: remote_path });
                refresh_directory_or_report(sftp, cwd, cwd, events);
            }
            Err(error) => events.send(SessionEvent::FileSaveFailed {
                path: remote_path,
                error: error.to_string(),
            }),
        },
        SessionCommand::Upload {
            local_paths,
            remote_directory,
        } => spawn_transfer_worker(
            config.clone(),
            events.clone(),
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
        ),
        SessionCommand::Download {
            remote_path,
            local_directory,
        } => spawn_transfer_worker(
            config.clone(),
            events.clone(),
            TransferDirection::Download,
            remote_file_name(&remote_path),
            move |sftp, transfer, on_progress| {
                sftp_client::download_entry(
                    sftp,
                    &remote_path,
                    &local_directory,
                    transfer,
                    on_progress,
                )
            },
        ),
        SessionCommand::Copy { source, target } => spawn_transfer_worker(
            config.clone(),
            events.clone(),
            TransferDirection::Copy,
            remote_file_name(&source),
            move |sftp, transfer, on_progress| {
                sftp_client::copy_entry(sftp, &source, &target, transfer, on_progress)
            },
        ),
        SessionCommand::Delete { remote_path } => {
            match sftp_client::delete_entry(sftp, &remote_path) {
                Ok(()) => {
                    refresh_directory_or_report(sftp, cwd, cwd, events);
                }
                Err(error) => events.send(SessionEvent::Error(format!(
                    "Unable to delete {remote_path}: {error}"
                ))),
            }
        }
        SessionCommand::Rename { source, target } => {
            match sftp_client::rename_entry(sftp, &source, &target) {
                Ok(()) => {
                    refresh_directory_or_report(sftp, cwd, cwd, events);
                }
                Err(error) => events.send(SessionEvent::Error(format!(
                    "Unable to move {source} to {target}: {error}"
                ))),
            }
        }
        SessionCommand::CreateDirectory { remote_path } => {
            match sftp_client::create_directory(sftp, &remote_path) {
                Ok(()) => {
                    refresh_directory_or_report(sftp, cwd, cwd, events);
                }
                Err(error) => events.send(SessionEvent::Error(format!(
                    "Unable to create folder {remote_path}: {error}"
                ))),
            }
        }
        // Handled by the shell loop.
        SessionCommand::SendInput(_)
        | SessionCommand::ResizeTerminal { .. }
        | SessionCommand::RefreshDirectory(_)
        | SessionCommand::HostKeyDecision(_)
        | SessionCommand::Disconnect => {}
    }
}

/// Expands `~` and `~/...` against the remote home directory.
pub fn resolve_home_relative(request: &str, home: &str) -> String {
    if request == "~" {
        return home.to_string();
    }

    match request.strip_prefix("~/") {
        Some(rest) => format!("{}/{}", home.trim_end_matches('/'), rest),
        None => request.to_string(),
    }
}

fn refresh_directory_or_report(
    sftp: &ssh2::Sftp,
    request: &str,
    directory: &str,
    events: &EventSink,
) -> bool {
    match sftp_client::list_directory(sftp, directory) {
        Ok(entries) => {
            events.send(SessionEvent::DirectoryLoaded {
                request: request.to_string(),
                cwd: directory.to_string(),
                entries,
            });
            true
        }
        Err(error) => {
            events.send(SessionEvent::DirectoryOpenFailed {
                request: request.to_string(),
                error: error.to_string(),
            });
            false
        }
    }
}

/// A counting semaphore bounding concurrent transfer connections. Transfers
/// beyond the limit wait in the `Queued` state instead of failing.
struct TransferSlots {
    limit: usize,
    active: Mutex<usize>,
    available: Condvar,
}

impl TransferSlots {
    const fn new(limit: usize) -> Self {
        Self {
            limit,
            active: Mutex::new(0),
            available: Condvar::new(),
        }
    }

    fn acquire(&'static self) -> TransferSlot {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *active >= self.limit {
            active = self
                .available
                .wait(active)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *active += 1;
        TransferSlot(self)
    }
}

struct TransferSlot(&'static TransferSlots);

impl Drop for TransferSlot {
    fn drop(&mut self) {
        let mut active = self
            .0
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *active = active.saturating_sub(1);
        self.0.available.notify_one();
    }
}

fn spawn_transfer_worker<F>(
    config: ConnectionConfig,
    events: EventSink,
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
    let mut transfer = sftp_client::queued_transfer(label, direction, 0);
    events.send(SessionEvent::Transfer(transfer.clone()));

    let spawned = thread::Builder::new()
        .name("rustssh-transfer".into())
        .spawn(move || {
            let _slot = TRANSFER_SLOTS.acquire();
            if events.is_closed() {
                return;
            }

            let result = (|| -> AppResult<()> {
                // The interactive session already verified this host, so an
                // unknown key here means the known_hosts file changed under us.
                let mut reject_unknown = |_: &HostKeyInfo| false;
                let session = client::connect_session(
                    &config.request,
                    config.key.as_ref(),
                    HostKeyPolicy::Verify {
                        known_hosts: &config.known_hosts,
                        on_unknown: &mut reject_unknown,
                    },
                )?;
                let sftp = session.sftp()?;
                let mut progress_sender = |progress: &TransferProgress| {
                    events.send(SessionEvent::Transfer(progress.clone()));
                };
                operation(&sftp, &mut transfer, &mut progress_sender)?;
                if finish_successful_transfer_if_needed(&mut transfer) {
                    events.send(SessionEvent::Transfer(transfer.clone()));
                }
                Ok(())
            })();

            if let Err(error) = result {
                transfer.status = TransferStatus::Failed(error.to_string());
                events.send(SessionEvent::Transfer(transfer));
            }
        });

    if let Err(error) = spawned {
        warn!(error = %error, "unable to start transfer worker");
    }
}

fn finish_successful_transfer_if_needed(transfer: &mut TransferProgress) -> bool {
    if matches!(transfer.status, TransferStatus::Completed) {
        return false;
    }

    transfer.status = TransferStatus::Completed;
    true
}

fn remote_file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
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

#[cfg(test)]
mod tests {
    use crate::models::{TransferDirection, TransferProgress, TransferStatus};

    #[test]
    fn finishes_running_successful_transfer() {
        let mut transfer = TransferProgress::queued("copy.txt", TransferDirection::Copy, 42);
        transfer.status = TransferStatus::Running;
        transfer.transferred_bytes = 42;

        assert!(super::finish_successful_transfer_if_needed(&mut transfer));
        assert!(matches!(transfer.status, TransferStatus::Completed));
    }

    #[test]
    fn leaves_already_completed_successful_transfer_unchanged() {
        let mut transfer = TransferProgress::queued("upload.txt", TransferDirection::Upload, 0);
        transfer.status = TransferStatus::Completed;

        assert!(!super::finish_successful_transfer_if_needed(&mut transfer));
        assert!(matches!(transfer.status, TransferStatus::Completed));
    }

    #[test]
    fn resolves_home_relative_requests() {
        assert_eq!(super::resolve_home_relative("~", "/home/dev"), "/home/dev");
        assert_eq!(
            super::resolve_home_relative("~/logs", "/home/dev/"),
            "/home/dev/logs"
        );
        assert_eq!(super::resolve_home_relative("/srv", "/home/dev"), "/srv");
    }

    #[test]
    fn transfer_slots_queue_beyond_the_limit() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Barrier};
        use std::thread;
        use std::time::Duration;

        static SLOTS: super::TransferSlots = super::TransferSlots::new(2);
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(6));

        let workers = (0..6)
            .map(|_| {
                let running = running.clone();
                let peak = peak.clone();
                let start = start.clone();
                thread::spawn(move || {
                    start.wait();
                    let _slot = SLOTS.acquire();
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(10));
                    running.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect::<Vec<_>>();

        for worker in workers {
            worker.join().expect("worker");
        }

        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }
}
