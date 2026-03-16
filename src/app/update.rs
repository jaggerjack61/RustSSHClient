use std::path::Path;
use std::time::Duration;

use iced::{Event, Subscription, Task, event, keyboard, time, window};
use iced::keyboard::key::Named;
use iced::keyboard::Key;

use crate::app::messages::{FileActionKind, Message};
use crate::app::state::{AppState, NotificationLevel, PendingFileAction, Route};
use crate::models::{AuthType, HostRecord, TransferStatus, WorkspaceTab};
use crate::sftp::file_tree::{DirectoryHint, collapse_segments, normalize_remote_path, parse_cd_command};
use crate::sftp::transfers::merge_transfer;
use crate::ssh::session::{SessionCommand, SessionEvent};
use crate::ssh::terminal;

const TERMINAL_CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(530);
const PROJECT_URL: &str = "https://github.com/jaggerjack61/RustSSHClient";
const KEY_MANAGER_COMING_SOON_MESSAGE: &str = "Key Manager is coming soon.";

pub fn subscription(_state: &AppState) -> Subscription<Message> {
    Subscription::batch(vec![
        time::every(Duration::from_millis(33)).map(Message::Tick),
        event::listen().map(Message::RuntimeEvent),
    ])
}

pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::StorageLoaded(result) => match result {
            Ok(snapshot) => {
                state.hosts = snapshot.hosts;
                state.keys = snapshot.keys;
                Task::none()
            }
            Err(error) => {
                state.notification(NotificationLevel::Error, error);
                Task::none()
            }
        },
        Message::LoginLabelChanged(value) => {
            state.login.label = value;
            Task::none()
        }
        Message::LoginHostChanged(value) => {
            state.login.host = value;
            Task::none()
        }
        Message::LoginPortChanged(value) => {
            state.login.port = value;
            Task::none()
        }
        Message::LoginUsernameChanged(value) => {
            state.login.username = value;
            Task::none()
        }
        Message::LoginPasswordChanged(value) => {
            state.login.password = value;
            Task::none()
        }
        Message::ToggleSaveConnection(value) => {
            state.login.save_connection = value;
            Task::none()
        }
        Message::UsePasswordAuthentication => {
            state.login.auth_type = AuthType::Password;
            Task::none()
        }
        Message::UseKeyAuthentication => {
            state.login.auth_type = AuthType::Key;
            if state.login.selected_key.is_none() {
                state.login.selected_key = state.keys.first().map(|key| key.id);
            }
            Task::none()
        }
        Message::ToggleKeyManager => {
            state.notification(
                NotificationLevel::Info,
                KEY_MANAGER_COMING_SOON_MESSAGE,
            );
            Task::none()
        }
        Message::OpenAdvancedSettings => {
            state.advanced_settings_open = true;
            Task::none()
        }
        Message::CloseAdvancedSettings => {
            state.advanced_settings_open = false;
            Task::none()
        }
        Message::SelectSaveLifetime(value) => {
            state.login.save_lifetime = value;
            Task::none()
        }
        Message::OpenProjectLink => {
            if let Err(error) = open_project_link() {
                state.notification(NotificationLevel::Error, error);
            }
            Task::none()
        }
        Message::ConnectPressed => {
            let request = match state.prepare_login_request() {
                Ok(request) => request,
                Err(error) => {
                    state.notification(NotificationLevel::Error, error);
                    return Task::none();
                }
            };

            let key = state.selected_key().cloned();
            state.login.connecting = true;
            let saved_host = if request.save_host {
                let mut host = state
                    .login
                    .editing_host_id
                    .and_then(|id| state.hosts.iter().find(|host| host.id == id).cloned())
                    .unwrap_or_else(|| HostRecord::new(&request));
                host.apply_request(&request);
                Some(host)
            } else {
                None
            };

            if let Some(host) = saved_host {
                upsert_host(state, host);
                return Task::batch([
                    persist_snapshot(state),
                    Task::perform(
                        async move {
                            crate::ssh::session::spawn(request, key)
                                .map_err(|error| error.to_string())
                        },
                        Message::SessionSpawned,
                    ),
                ]);
            }

            Task::perform(
                async move {
                    crate::ssh::session::spawn(request, key).map_err(|error| error.to_string())
                },
                Message::SessionSpawned,
            )
        }
        Message::SessionSpawned(result) => {
            state.login.connecting = false;
            match result {
                Ok(handle) => {
                    state.route = Route::Workspace;
                    state.workspace.session = Some(handle);
                    state.workspace.status = "Connecting".into();
                    state.workspace.terminal.clear();
                    state.workspace.reset_editor_tabs();
                    state.workspace.terminal_cursor_visible = true;
                    state.workspace.last_terminal_cursor_toggle = std::time::Instant::now();
                    Task::none()
                }
                Err(error) => {
                    state.notification(NotificationLevel::Error, error);
                    Task::none()
                }
            }
        }
        Message::HostCardPressed(id) => {
            let clicked_twice = state
                .last_host_click
                .map(|(previous_id, instant)| {
                    previous_id == id && instant.elapsed() < Duration::from_millis(450)
                })
                .unwrap_or(false);
            state.last_host_click = Some((id, std::time::Instant::now()));

            if let Some(host) = state.hosts.iter().find(|host| host.id == id).cloned() {
                state.apply_host_to_form(&host);
            }

            if clicked_twice {
                update(state, Message::ConnectPressed)
            } else {
                Task::none()
            }
        }
        Message::DeleteHost(id) => {
            state.hosts.retain(|host| host.id != id);
            persist_snapshot(state)
        }
        Message::EditHost(id) => {
            if let Some(host) = state.hosts.iter().find(|host| host.id == id).cloned() {
                state.apply_host_to_form(&host);
            }
            Task::none()
        }
        Message::HostSortChanged(sort) => {
            state.host_sort = sort;
            Task::none()
        }
        Message::SelectKey(id) => {
            state.login.selected_key = Some(id);
            state.login.auth_type = AuthType::Key;
            Task::none()
        }
        Message::DeleteKey(id) => {
            state.keys.retain(|key| key.id != id);
            if state.login.selected_key == Some(id) {
                state.login.selected_key = None;
            }
            persist_snapshot(state)
        }
        Message::ImportKeyPressed => {
            Task::perform(async move { import_key_dialog() }, Message::KeyImported)
        }
        Message::KeyImported(result) => match result {
            Ok(Some(key)) => {
                state.login.selected_key = Some(key.id);
                state.keys.push(key);
                persist_snapshot(state)
            }
            Ok(None) => Task::none(),
            Err(error) => {
                state.notification(NotificationLevel::Error, error);
                Task::none()
            }
        },
        Message::Tick(now) => {
            state.prune_notifications();

            if state.route == Route::Workspace
                && now.duration_since(state.workspace.last_terminal_cursor_toggle)
                    >= TERMINAL_CURSOR_BLINK_INTERVAL
            {
                state.workspace.terminal_cursor_visible = !state.workspace.terminal_cursor_visible;
                state.workspace.last_terminal_cursor_toggle = now;
            }

            drain_session_events(state)
        }
        Message::RuntimeEvent(event) => match event {
            Event::Window(window::Event::FileDropped(path)) => {
                if state.workspace.session.is_some() {
                    let target_directory = state
                        .selected_file()
                        .filter(|entry| entry.is_directory())
                        .map(|entry| entry.path.clone())
                        .unwrap_or_else(|| state.workspace.current_directory.clone());

                    let _ = send_session_command(
                        state,
                        SessionCommand::Upload {
                            local_paths: vec![path],
                            remote_directory: target_directory,
                        },
                    );
                }
                Task::none()
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) => {
                if state.route != Route::Workspace || state.workspace.session.is_none() {
                    return Task::none();
                }

                if modifiers.control() && !modifiers.shift() {
                    if let Key::Character(ref ch) = key {
                        if ch.as_str().eq_ignore_ascii_case("s") {
                            return update(state, Message::SaveActiveEditor);
                        }
                    }
                }

                if !matches!(state.workspace.active_tab, WorkspaceTab::Terminal) {
                    return Task::none();
                }

                // Ctrl+Shift+C → copy terminal output (terminal convention).
                if modifiers.control() && modifiers.shift() {
                    if let Key::Character(ref ch) = key {
                        if ch.as_str().eq_ignore_ascii_case("c") {
                            return update(state, Message::CopyTerminalOutput);
                        }
                    }
                }

                // Ctrl+Shift+V → paste from clipboard into PTY.
                if modifiers.control() && modifiers.shift() {
                    if let Key::Character(ref ch) = key {
                        if ch.as_str().eq_ignore_ascii_case("v") {
                            return handle_terminal_paste(state);
                        }
                    }
                }

                // Detect `cd` commands before sending Enter to the PTY.
                if key == Key::Named(Named::Enter) {
                    let line = state.workspace.terminal.current_cursor_line();
                    let command = terminal::extract_command_from_prompt_line(&line);
                    if let Some(hint) =
                        parse_cd_command(&state.workspace.current_directory, command)
                    {
                        match hint {
                            DirectoryHint::ResolveHome => {
                                let _ = request_directory_refresh(state, None);
                            }
                            DirectoryHint::HomeRelative(sub) => {
                                let _ = request_directory_refresh(
                                    state,
                                    Some(format!("~/{sub}")),
                                );
                            }
                            DirectoryHint::Absolute(path) => {
                                let _ = request_directory_refresh(state, Some(path));
                            }
                        }
                    }
                }

                // Translate the key event to PTY bytes and send.
                if let Some(bytes) =
                    terminal::key_to_bytes(&key, modifiers, text.as_deref())
                {
                    let _ = send_session_command(
                        state,
                        SessionCommand::SendInput(bytes),
                    );
                }
                Task::none()
            }
            _ => Task::none(),
        },
        Message::ClearTerminal => {
            state.workspace.terminal.clear();
            Task::none()
        }
        Message::CopyTerminalOutput => {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Err(error) = clipboard.set_text(state.workspace.terminal.display_text()) {
                    state.notification(NotificationLevel::Error, error.to_string());
                }
            }
            Task::none()
        }
        Message::PasteTerminalInput => handle_terminal_paste(state),
        Message::DisconnectPressed => {
            if send_session_command(state, SessionCommand::Disconnect) {
                state.workspace.status = "Disconnecting".into();
            }
            Task::none()
        }
        Message::RefreshDirectory => {
            state.workspace.explorer_context_for = None;
            state.workspace.show_properties = false;
            let _ = request_directory_refresh(
                state,
                Some(state.workspace.current_directory.clone()),
            );
            Task::none()
        }
        Message::NavigateUpDirectory => {
            state.workspace.explorer_context_for = None;
            state.workspace.show_properties = false;
            let parent = parent_directory(&state.workspace.current_directory);
            if parent != state.workspace.current_directory {
                let _ = request_directory_refresh(state, Some(parent));
            }
            Task::none()
        }
        Message::DismissExplorerContextMenu => {
            state.workspace.explorer_context_for = None;
            Task::none()
        }
        Message::ExplorerEntryPressed(path) => {
            state.workspace.selected_file = Some(path.clone());
            state.workspace.explorer_context_for = None;
            state.workspace.show_properties = false;
            if let Some(entry) = state
                .workspace
                .files
                .iter()
                .find(|entry| entry.path == path)
            {
                if entry.is_directory() {
                    if state.workspace.expanded_folders.contains(&path) {
                        state.workspace.expanded_folders.remove(&path);
                    } else if state.workspace.loaded_folders.contains(&path) {
                        state.workspace.expanded_folders.insert(path);
                    } else {
                        state.workspace.expanded_folders.insert(path.clone());
                        let _ = request_directory_children(state, path);
                    }
                }
            }
            Task::none()
        }
        Message::ExplorerEntryDoubleClicked(path) => {
            state.workspace.selected_file = Some(path.clone());
            state.workspace.explorer_context_for = None;
            state.workspace.show_properties = false;
            if let Some(is_directory) = state
                .workspace
                .files
                .iter()
                .find(|entry| entry.path == path)
                .map(|entry| entry.is_directory())
            {
                if is_directory {
                    let _ = request_directory_refresh(state, Some(path));
                } else {
                    return update(state, Message::OpenSelectedFileInEditor);
                }
            }
            Task::none()
        }
        Message::ExplorerEntrySecondaryPressed(path) => {
            state.workspace.selected_file = Some(path.clone());
            state.workspace.explorer_context_for = Some(path);
            Task::none()
        }
        Message::ToggleExpandedFolder(path) => {
            if state.workspace.expanded_folders.contains(&path) {
                state.workspace.expanded_folders.remove(&path);
            } else {
                state.workspace.expanded_folders.insert(path);
            }
            Task::none()
        }
        Message::ShowProperties => {
            state.workspace.explorer_context_for = None;
            state.workspace.show_properties = true;
            Task::none()
        }
        Message::DismissProperties => {
            state.workspace.show_properties = false;
            Task::none()
        }
        Message::OpenSelectedFileInEditor => {
            state.workspace.explorer_context_for = None;

            let Some(selected) = state.selected_file().cloned() else {
                state.notification(
                    NotificationLevel::Info,
                    "Select a remote file first.",
                );
                return Task::none();
            };

            if selected.is_directory() {
                state.notification(
                    NotificationLevel::Info,
                    "Folders cannot be opened in the editor.",
                );
                return Task::none();
            }

            state.workspace.open_editor_tab(selected.path.clone());

            if !send_session_command(
                state,
                SessionCommand::ReadFile {
                    remote_path: selected.path.clone(),
                },
            ) {
                state.workspace.fail_editor_load(
                    &selected.path,
                    "Unable to request the remote file contents.".into(),
                );
            }

            Task::none()
        }
        Message::EditorAction(path, action) => {
            state.workspace.apply_editor_action(&path, action);
            Task::none()
        }
        Message::SaveActiveEditor => {
            let Some(editor) = state.active_editor() else {
                return Task::none();
            };

            if editor.is_loading || editor.load_error.is_some() || !editor.is_dirty || editor.is_saving {
                return Task::none();
            }

            let path = editor.path.clone();
            let contents = editor.current_text();
            state.workspace.mark_editor_saving(&path);

            if !send_session_command(
                state,
                SessionCommand::WriteFile {
                    remote_path: path.clone(),
                    contents,
                },
            ) {
                state.workspace.mark_editor_save_failed(&path);
                state.notification(
                    NotificationLevel::Error,
                    format!("Unable to save remote file {path}."),
                );
            }

            Task::none()
        }
        Message::ActivateTerminalTab => {
            state.workspace.active_tab = WorkspaceTab::Terminal;
            Task::none()
        }
        Message::ActivateEditorTab(path) => {
            if state.workspace.editor_tabs.iter().any(|tab| tab.path == path) {
                state.workspace.active_tab = WorkspaceTab::Editor(path);
            }
            Task::none()
        }
        Message::CloseEditorTab(path) => {
            state.workspace.close_editor_tab(&path);
            Task::none()
        }
        Message::UploadRequested => {
            state.workspace.explorer_context_for = None;
            let files = rfd::FileDialog::new().pick_files();
            update(state, Message::FilesSelected(files))
        }
        Message::FilesSelected(files) => {
            if let Some(paths) = files {
                let remote_directory = state
                    .selected_file()
                    .filter(|entry| entry.is_directory())
                    .map(|entry| entry.path.clone())
                    .unwrap_or_else(|| state.workspace.current_directory.clone());

                let _ = send_session_command(
                    state,
                    SessionCommand::Upload {
                        local_paths: paths,
                        remote_directory,
                    },
                );
            }
            Task::none()
        }
        Message::DownloadRequested => {
            state.workspace.explorer_context_for = None;
            let Some(selected) = state.selected_file().cloned() else {
                state.notification(
                    NotificationLevel::Info,
                    "Select a remote file or folder first.",
                );
                return Task::none();
            };

            if let Some(local_directory) = rfd::FileDialog::new().pick_folder() {
                let _ = send_session_command(
                    state,
                    SessionCommand::Download {
                        remote_path: selected.path,
                        local_directory,
                    },
                );
            }
            Task::none()
        }
        Message::DeleteSelectedFile => {
            state.workspace.explorer_context_for = None;
            let Some(selected) = state.selected_file().cloned() else {
                return Task::none();
            };

            if send_session_command(
                state,
                SessionCommand::Delete {
                    remote_path: selected.path,
                },
            ) {
                state.workspace.selected_file = None;
            }
            Task::none()
        }
        Message::StartFileAction(kind) => {
            state.workspace.explorer_context_for = None;
            let seed = state
                .selected_file()
                .map(|entry| match kind {
                    FileActionKind::Rename => entry.name.clone(),
                    FileActionKind::Copy | FileActionKind::Move => entry.path.clone(),
                })
                .unwrap_or_default();
            state.workspace.pending_file_action = Some(PendingFileAction { kind, value: seed });
            Task::none()
        }
        Message::FileActionInputChanged(value) => {
            if let Some(action) = &mut state.workspace.pending_file_action {
                action.value = value;
            }
            Task::none()
        }
        Message::ConfirmFileAction => {
            let Some(action) = state.workspace.pending_file_action.clone() else {
                return Task::none();
            };
            let Some(selected) = state.selected_file().cloned() else {
                return Task::none();
            };
            if state.workspace.session.is_none() {
                return Task::none();
            }

            let target = resolve_file_action_target(
                &state.workspace.current_directory,
                &selected.path,
                &action.value,
                action.kind,
            );

            let sent = match action.kind {
                FileActionKind::Rename | FileActionKind::Move => {
                    send_session_command(
                        state,
                        SessionCommand::Rename {
                            source: selected.path,
                            target,
                        },
                    )
                }
                FileActionKind::Copy => {
                    send_session_command(
                        state,
                        SessionCommand::Copy {
                            source: selected.path,
                            target,
                        },
                    )
                }
            };

            if sent {
                state.workspace.pending_file_action = None;
            }
            Task::none()
        }
        Message::CancelFileAction => {
            state.workspace.pending_file_action = None;
            Task::none()
        }
        Message::DismissNotification(index) => {
            if index < state.notifications.len() {
                state.notifications.remove(index);
            }
            Task::none()
        }
        Message::ToggleMarkdownPreview => {
            if let Some(editor) = state.active_editor_mut() {
                editor.markdown_preview = !editor.markdown_preview;
                if editor.markdown_preview {
                    let text = editor.current_text();
                    editor.markdown_items = iced::widget::markdown::parse(&text).collect();
                }
            }
            Task::none()
        }
        Message::MarkdownLinkClicked(url) => {
            let _ = webbrowser::open(&url);
            Task::none()
        }
    }
}

fn persist_snapshot(state: &mut AppState) -> Task<Message> {
    match state.storage.save_snapshot(&state.snapshot()) {
        Ok(()) => Task::none(),
        Err(error) => {
            state.notification(NotificationLevel::Error, error.to_string());
            Task::none()
        }
    }
}

fn import_key_dialog() -> Result<Option<crate::models::SshKeyRecord>, String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("PEM key", &["pem", "key", "rsa"])
        .pick_file()
    else {
        return Ok(None);
    };

    let label = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported key".into());
    let pem = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(Some(crate::models::SshKeyRecord::new(label, pem)))
}

fn drain_session_events(state: &mut AppState) -> Task<Message> {
    while let Some(event) = state
        .workspace
        .session
        .as_ref()
        .and_then(|session| session.try_recv())
    {
        match event {
            SessionEvent::Connected {
                cwd,
                latency_ms,
                peer,
            } => {
                state.workspace.status = "Connected".into();
                state.workspace.current_directory = cwd;
                state.workspace.pending_directory = Some(state.workspace.current_directory.clone());
                state.workspace.explorer_context_for = None;
                state.workspace.connected_peer = peer;
                state.workspace.latency_ms = Some(latency_ms);
                state.notification(NotificationLevel::Success, "SSH session established.");
            }
            SessionEvent::Output(bytes) => {
                state.workspace.terminal.feed(&bytes);
                state.workspace.terminal_cursor_visible = true;
                state.workspace.last_terminal_cursor_toggle = std::time::Instant::now();
            }
            SessionEvent::DirectoryLoaded { cwd, entries } => {
                // Discard stale directory loads — if we are waiting for a
                // specific directory and this result is for a different one,
                // skip it so we don't briefly flash the wrong listing or
                // clear `pending_directory` prematurely.
                if let Some(pending) = &state.workspace.pending_directory {
                    if *pending != cwd {
                        continue;
                    }
                }

                let selected_file = state.workspace.selected_file.clone();
                state.workspace.current_directory = cwd;
                state.workspace.pending_directory = None;
                state.workspace.files = entries;
                state.workspace.expanded_folders.clear();
                state.workspace.loaded_folders.clear();
                state.workspace.loading_folders.clear();
                state.workspace.explorer_context_for = None;
                state.workspace.selected_file = selected_file.filter(|selected| {
                    state
                        .workspace
                        .files
                        .iter()
                        .any(|entry| entry.path == *selected)
                });
            }
            SessionEvent::DirectoryChildrenLoaded { directory, entries } => {
                state.workspace.loading_folders.remove(&directory);
                state.workspace.loaded_folders.insert(directory.clone());
                merge_directory_children(&mut state.workspace.files, &directory, entries);
            }
            SessionEvent::FileOpened { path, contents } => {
                state.workspace.apply_editor_content(&path, contents);
            }
            SessionEvent::FileOpenFailed { path, error } => {
                state.workspace.fail_editor_load(&path, error.clone());
                state.notification(
                    NotificationLevel::Error,
                    format!("Unable to open remote file {path}: {error}"),
                );
            }
            SessionEvent::FileSaved { path } => {
                state.workspace.mark_editor_saved(&path);
                state.notification(
                    NotificationLevel::Success,
                    format!("Saved {path}"),
                );
            }
            SessionEvent::FileSaveFailed { path, error } => {
                state.workspace.mark_editor_save_failed(&path);
                state.notification(
                    NotificationLevel::Error,
                    format!("Unable to save remote file {path}: {error}"),
                );
            }
            SessionEvent::DirectoryOpenFailed { path, error } => {
                let should_notify = should_notify_directory_open_failure(
                    state.workspace.pending_directory.as_deref(),
                    &path,
                );
                if should_notify {
                    state.workspace.pending_directory = None;
                    state.notification(
                        NotificationLevel::Error,
                        format!("Unable to open remote directory {path}: {error}"),
                    );
                }
            }
            SessionEvent::DirectoryChildrenLoadFailed { directory, error } => {
                state.workspace.loading_folders.remove(&directory);
                state.workspace.expanded_folders.remove(&directory);
                state.notification(
                    NotificationLevel::Error,
                    format!("Unable to expand remote directory {directory}: {error}"),
                );
            }
            SessionEvent::Transfer(update) => {
                merge_transfer(&update, &mut state.workspace.transfers);
                if matches!(update.status, TransferStatus::Failed(_)) {
                    state.notification(NotificationLevel::Error, "A file transfer failed.");
                }
            }
            SessionEvent::Error(error) => state.notification(NotificationLevel::Error, error),
            SessionEvent::Disconnected(message) => {
                state.workspace.status = message;
                state.workspace.session = None;
                state.workspace.pending_directory = None;
                state.workspace.explorer_context_for = None;
                state.workspace.expanded_folders.clear();
                state.workspace.loaded_folders.clear();
                state.workspace.loading_folders.clear();
                state.workspace.reset_editor_tabs();
                state.route = Route::Login;
                state.notification(NotificationLevel::Info, "SSH session ended.");
            }
        }
    }

    Task::none()
}

fn upsert_host(state: &mut AppState, host: HostRecord) {
    if let Some(existing) = state.hosts.iter_mut().find(|item| item.id == host.id) {
        *existing = host;
    } else {
        state.hosts.push(host);
    }
}

fn send_session_command(state: &mut AppState, command: SessionCommand) -> bool {
    let Some(session) = &state.workspace.session else {
        return false;
    };

    if let Err(error) = session.send(command) {
        state.notification(NotificationLevel::Error, error.to_string());
        return false;
    }

    true
}

fn request_directory_refresh(state: &mut AppState, path: Option<String>) -> bool {
    state.workspace.pending_directory = path.clone();
    if send_session_command(state, SessionCommand::RefreshDirectory(path)) {
        true
    } else {
        state.workspace.pending_directory = None;
        false
    }
}

fn request_directory_children(state: &mut AppState, path: String) -> bool {
    if state.workspace.loading_folders.contains(&path) {
        return true;
    }

    state.workspace.loading_folders.insert(path.clone());
    if send_session_command(state, SessionCommand::LoadDirectoryChildren(path.clone())) {
        true
    } else {
        state.workspace.loading_folders.remove(&path);
        state.workspace.expanded_folders.remove(&path);
        false
    }
}

fn should_notify_directory_open_failure(pending_directory: Option<&str>, failed_path: &str) -> bool {
    pending_directory == Some(failed_path)
}

fn merge_directory_children(
    files: &mut Vec<crate::models::FileEntry>,
    directory: &str,
    entries: Vec<crate::models::FileEntry>,
) {
    files.retain(|entry| {
        entry.path == directory || !is_descendant_path(&entry.path, directory)
    });
    files.extend(entries);
}

fn is_descendant_path(path: &str, directory: &str) -> bool {
    let prefix = format!("{}/", directory.trim_end_matches('/'));
    path.starts_with(&prefix)
}

fn parent_directory(current_directory: &str) -> String {
    normalize_remote_path(current_directory, "..")
}

fn resolve_file_action_target(
    current_directory: &str,
    selected_path: &str,
    input: &str,
    kind: FileActionKind,
) -> String {
    let trimmed = normalize_remote_path_input(input);
    if trimmed.starts_with('/') {
        return collapse_segments(&trimmed);
    }

    match kind {
        FileActionKind::Rename => {
            let parent = Path::new(selected_path)
                .parent()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| current_directory.to_string());
            normalize_remote_path(&parent, &trimmed)
        }
        FileActionKind::Copy | FileActionKind::Move => {
            normalize_remote_path(current_directory, &trimmed)
        }
    }
}

fn normalize_remote_path_input(value: &str) -> String {
    value.trim().replace('\\', "/")
}

fn handle_terminal_paste(state: &mut AppState) -> Task<Message> {
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) => {
            if !text.is_empty() {
                let _ = send_session_command(
                    state,
                    SessionCommand::SendInput(text.into_bytes()),
                );
            }
        }
        Err(error) => state.notification(NotificationLevel::Error, error.to_string()),
    }
    Task::none()
}

fn open_project_link() -> Result<(), String> {
    webbrowser::open(PROJECT_URL)
        .map(|_| ())
        .map_err(|error| format!("Unable to open {PROJECT_URL}: {error}"))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crossbeam_channel::unbounded;
    use iced::widget::text_editor;

    use crate::app::messages::{FileActionKind, Message};
    use crate::app::state::{AppState, Route};
    use crate::models::{EditorLanguage, FileEntry, FileKind, SaveLifetime, WorkspaceTab};
    use crate::ssh::session::{SessionCommand, SessionEvent, SessionHandle};
    use crate::storage::StorageFacade;
    use tempfile::tempdir;

    #[test]
    fn updates_login_form_fields() {
        let (mut state, _) = AppState::boot();
        let _ = super::update(
            &mut state,
            Message::LoginHostChanged("prod.example.com".into()),
        );
        let _ = super::update(&mut state, Message::LoginUsernameChanged("deploy".into()));

        assert_eq!(state.login.host, "prod.example.com");
        assert_eq!(state.login.username, "deploy");
    }

    #[test]
    fn deletes_saved_host() {
        let tempdir = tempdir().expect("create tempdir");
        let (mut state, _) = AppState::boot();
        state.storage = StorageFacade::for_root(tempdir.path().to_path_buf());
        state.login.host = "prod.example.com".into();
        state.login.port = "22".into();
        state.login.username = "deploy".into();
        state.login.password = "secret".into();
        state.login.save_connection = true;

        let _ = super::update(&mut state, Message::ConnectPressed);
        let host_id = state.hosts[0].id;

        let _ = super::update(&mut state, Message::DeleteHost(host_id));

        assert!(state.hosts.is_empty());

        let reloaded = StorageFacade::for_root(tempdir.path().to_path_buf())
            .load_snapshot()
            .expect("reload saved snapshot");
        assert!(reloaded.hosts.is_empty());
    }

    #[test]
    fn saves_host_immediately_on_connect_press() {
        let tempdir = tempdir().expect("create tempdir");
        let (mut state, _) = AppState::boot();
        state.storage = StorageFacade::for_root(tempdir.path().to_path_buf());
        state.login.host = "prod.example.com".into();
        state.login.port = "22".into();
        state.login.username = "deploy".into();
        state.login.password = "secret".into();
        state.login.save_connection = true;

        let _ = super::update(&mut state, Message::ConnectPressed);

        assert_eq!(state.hosts.len(), 1);
        assert_eq!(state.hosts[0].host, "prod.example.com");
        assert_eq!(state.hosts[0].password.as_deref(), Some("secret"));

        let reloaded = StorageFacade::for_root(tempdir.path().to_path_buf())
            .load_snapshot()
            .expect("reload saved snapshot");
        assert_eq!(reloaded.hosts.len(), 1);
        assert_eq!(reloaded.hosts[0].host, "prod.example.com");
        assert_eq!(reloaded.hosts[0].password.as_deref(), Some("secret"));
    }

    #[test]
    fn does_not_persist_host_when_save_connection_disabled() {
        let tempdir = tempdir().expect("create tempdir");
        let (mut state, _) = AppState::boot();
        state.storage = StorageFacade::for_root(tempdir.path().to_path_buf());
        state.login.host = "prod.example.com".into();
        state.login.port = "22".into();
        state.login.username = "deploy".into();
        state.login.password = "secret".into();
        state.login.save_connection = false;

        let _ = super::update(&mut state, Message::ConnectPressed);

        let reloaded = StorageFacade::for_root(tempdir.path().to_path_buf())
            .load_snapshot()
            .expect("reload snapshot");
        assert!(reloaded.hosts.is_empty());
    }

    #[test]
    fn normalizes_windows_style_remote_targets() {
        let target = super::resolve_file_action_target(
            "/srv/app",
            "/srv/app/config/settings.toml",
            r"nested\settings.toml",
            FileActionKind::Rename,
        );

        assert_eq!(target, "/srv/app/config/nested/settings.toml");
    }

    #[test]
    fn opens_explorer_context_menu_on_secondary_press() {
        let (mut state, _) = AppState::boot();
        state.workspace.selected_file = None;

        let _ = super::update(
            &mut state,
            Message::ExplorerEntrySecondaryPressed("/srv/app".into()),
        );

        assert_eq!(state.workspace.selected_file.as_deref(), Some("/srv/app"));
        assert_eq!(state.workspace.explorer_context_for.as_deref(), Some("/srv/app"));
    }

    #[test]
    fn dismisses_explorer_context_menu_without_clearing_selection() {
        let (mut state, _) = AppState::boot();
        state.workspace.selected_file = Some("/srv/app/README.md".into());
        state.workspace.explorer_context_for = Some("/srv/app/README.md".into());

        let _ = super::update(&mut state, Message::DismissExplorerContextMenu);

        assert_eq!(state.workspace.selected_file.as_deref(), Some("/srv/app/README.md"));
        assert!(state.workspace.explorer_context_for.is_none());
    }

    #[test]
    fn explorer_primary_press_closes_context_menu() {
        let (mut state, _) = AppState::boot();
        state.workspace.files = vec![text_file_entry("/srv/app/README.md", "README.md")];
        state.workspace.explorer_context_for = Some("/srv/app/README.md".into());

        let _ = super::update(
            &mut state,
            Message::ExplorerEntryPressed("/srv/app/README.md".into()),
        );

        assert!(state.workspace.explorer_context_for.is_none());
        assert_eq!(state.workspace.selected_file.as_deref(), Some("/srv/app/README.md"));
    }

    #[test]
    fn refresh_directory_closes_context_menu() {
        let (mut state, _) = AppState::boot();
        state.workspace.explorer_context_for = Some("/srv/app/README.md".into());

        let _ = super::update(&mut state, Message::RefreshDirectory);

        assert!(state.workspace.explorer_context_for.is_none());
    }

    #[test]
    fn navigates_up_from_current_directory() {
        assert_eq!(
            super::parent_directory("/srv/app/releases"),
            "/srv/app"
        );
    }

    #[test]
    fn ignores_stale_directory_errors_after_successful_load() {
        assert!(!super::should_notify_directory_open_failure(
            None,
            "/srv/old",
        ));
        assert!(!super::should_notify_directory_open_failure(
            Some("/srv/app"),
            "/srv/old",
        ));
        assert!(super::should_notify_directory_open_failure(
            Some("/srv/app"),
            "/srv/app",
        ));
    }

    #[test]
    fn project_link_points_to_repository() {
        assert_eq!(super::PROJECT_URL, "https://github.com/jaggerjack61/RustSSHClient");
    }

    #[test]
    fn key_manager_shows_coming_soon_notification() {
        let (mut state, _) = AppState::boot();

        let _ = super::update(&mut state, Message::ToggleKeyManager);

        assert!(!state.key_manager_open);
        assert_eq!(state.notifications.len(), 1);
        assert_eq!(state.notifications[0].level, crate::app::state::NotificationLevel::Info);
        assert_eq!(state.notifications[0].message, "Key Manager is coming soon.");
    }

    #[test]
    fn advanced_settings_updates_login_save_lifetime() {
        let (mut state, _) = AppState::boot();

        let _ = super::update(&mut state, Message::OpenAdvancedSettings);
        let _ = super::update(
            &mut state,
            Message::SelectSaveLifetime(crate::models::SaveLifetime::OneWeek),
        );

        assert!(state.advanced_settings_open);
        assert_eq!(state.login.save_lifetime, SaveLifetime::OneWeek);
    }

    #[test]
    fn opens_unknown_extension_file_in_editor_and_dispatches_session_command() {
        let (command_tx, command_rx) = unbounded();
        let (_event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.files = vec![text_file_entry("/srv/app/Procfile", "Procfile")];
        state.workspace.selected_file = Some("/srv/app/Procfile".into());

        let _ = super::update(&mut state, Message::OpenSelectedFileInEditor);

        assert_eq!(state.workspace.editor_tabs.len(), 1);
        assert_eq!(state.workspace.editor_tabs[0].title, "Procfile");
        assert!(state.workspace.editor_tabs[0].is_loading);
        assert_eq!(state.workspace.active_tab, WorkspaceTab::Editor("/srv/app/Procfile".into()));
        assert_eq!(
            command_rx.try_recv().expect("editor read command"),
            SessionCommand::ReadFile {
                remote_path: "/srv/app/Procfile".into(),
            }
        );
    }

    #[test]
    fn does_not_open_directories_in_editor() {
        let (command_tx, command_rx) = unbounded();
        let (_event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.files = vec![directory_entry("/srv/app/config", "config")];
        state.workspace.selected_file = Some("/srv/app/config".into());

        let _ = super::update(&mut state, Message::OpenSelectedFileInEditor);

        assert!(state.workspace.editor_tabs.is_empty());
        assert!(command_rx.try_recv().is_err());
    }

    #[test]
    fn double_clicking_a_file_opens_it_in_editor() {
        let (command_tx, command_rx) = unbounded();
        let (_event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.files = vec![text_file_entry("/srv/app/README.md", "README.md")];

        let _ = super::update(
            &mut state,
            Message::ExplorerEntryDoubleClicked("/srv/app/README.md".into()),
        );

        assert_eq!(state.workspace.selected_file.as_deref(), Some("/srv/app/README.md"));
        assert_eq!(state.workspace.editor_tabs.len(), 1);
        assert_eq!(state.workspace.active_tab, WorkspaceTab::Editor("/srv/app/README.md".into()));
        assert_eq!(
            command_rx.try_recv().expect("editor read command"),
            SessionCommand::ReadFile {
                remote_path: "/srv/app/README.md".into(),
            }
        );
    }

    #[test]
    fn single_clicking_directory_requests_lazy_child_load() {
        let (command_tx, command_rx) = unbounded();
        let (_event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));
        state.workspace.files = vec![directory_entry("/srv/app/src", "src")];

        let _ = super::update(
            &mut state,
            Message::ExplorerEntryPressed("/srv/app/src".into()),
        );

        assert_eq!(state.workspace.selected_file.as_deref(), Some("/srv/app/src"));
        assert!(state.workspace.expanded_folders.contains("/srv/app/src"));
        assert_eq!(
            command_rx.try_recv().expect("child load command"),
            SessionCommand::LoadDirectoryChildren("/srv/app/src".into())
        );
    }

    #[test]
    fn directory_child_load_merges_entries_without_changing_current_directory() {
        let (command_tx, _command_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));
        state.workspace.current_directory = "/srv/app".into();
        state.workspace.files = vec![
            directory_entry("/srv/app/src", "src"),
            text_file_entry("/srv/app/README.md", "README.md"),
        ];
        state.workspace.expanded_folders.insert("/srv/app/src".into());

        event_tx
            .send(SessionEvent::DirectoryChildrenLoaded {
                directory: "/srv/app/src".into(),
                entries: vec![text_file_entry("/srv/app/src/main.rs", "main.rs")],
            })
            .expect("queue child directory event");

        let _ = super::update(&mut state, Message::Tick(Instant::now()));

        assert_eq!(state.workspace.current_directory, "/srv/app");
        assert!(state
            .workspace
            .files
            .iter()
            .any(|entry| entry.path == "/srv/app/src/main.rs"));
        assert!(state.workspace.expanded_folders.contains("/srv/app/src"));
    }

    #[test]
    fn loads_editor_contents_from_session_event() {
        let (command_tx, _command_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.open_editor_tab("/srv/app/src/main.rs");
        event_tx
            .send(SessionEvent::FileOpened {
                path: "/srv/app/src/main.rs".into(),
                contents: "fn main() {}\n".into(),
            })
            .expect("queue editor event");

        let _ = super::update(&mut state, Message::Tick(Instant::now()));

        let editor = state.active_editor().expect("active editor");
        assert_eq!(editor.language, EditorLanguage::Rust);
        assert_eq!(editor.current_text(), "fn main() {}\n");
        assert!(!editor.is_loading);
        assert!(editor.load_error.is_none());
    }

    #[test]
    fn stores_editor_load_failures_in_tab_state() {
        let (command_tx, _command_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.open_editor_tab("/srv/app/README.md");
        event_tx
            .send(SessionEvent::FileOpenFailed {
                path: "/srv/app/README.md".into(),
                error: "File is not valid UTF-8 text.".into(),
            })
            .expect("queue error event");

        let _ = super::update(&mut state, Message::Tick(Instant::now()));

        let editor = state.active_editor().expect("active editor");
        assert_eq!(editor.load_error.as_deref(), Some("File is not valid UTF-8 text."));
        assert!(!editor.is_loading);
    }

    #[test]
    fn closes_active_editor_tab_and_returns_to_terminal() {
        let (mut state, _) = AppState::boot();
        state.workspace.open_editor_tab("/srv/app/README.md");
        state.workspace.apply_editor_content("/srv/app/README.md", "# RustSSH\n".into());

        let _ = super::update(&mut state, Message::CloseEditorTab("/srv/app/README.md".into()));

        assert!(state.workspace.editor_tabs.is_empty());
        assert_eq!(state.workspace.active_tab, WorkspaceTab::Terminal);
    }

    #[test]
    fn editor_actions_mark_document_dirty() {
        let (mut state, _) = AppState::boot();
        state.workspace.open_editor_tab("/srv/app/README.md");
        state.workspace.apply_editor_content("/srv/app/README.md", "hello".into());

        let _ = super::update(
            &mut state,
            Message::EditorAction(
                "/srv/app/README.md".into(),
                text_editor::Action::Move(text_editor::Motion::DocumentEnd),
            ),
        );
        let _ = super::update(
            &mut state,
            Message::EditorAction(
                "/srv/app/README.md".into(),
                text_editor::Action::Edit(text_editor::Edit::Insert('!')),
            ),
        );

        let editor = state.active_editor().expect("active editor");
        assert!(editor.is_dirty);
        assert_eq!(editor.current_text(), "hello!");
    }

    #[test]
    fn save_active_editor_dispatches_write_command() {
        let (command_tx, command_rx) = unbounded();
        let (_event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.open_editor_tab("/srv/app/README.md");
        state.workspace.apply_editor_content("/srv/app/README.md", "hello".into());
        state.workspace.apply_editor_action(
            "/srv/app/README.md",
            text_editor::Action::Move(text_editor::Motion::DocumentEnd),
        );
        state.workspace.apply_editor_action(
            "/srv/app/README.md",
            text_editor::Action::Edit(text_editor::Edit::Insert('!')),
        );

        let _ = super::update(&mut state, Message::SaveActiveEditor);

        assert_eq!(
            command_rx.try_recv().expect("write command"),
            SessionCommand::WriteFile {
                remote_path: "/srv/app/README.md".into(),
                contents: "hello!".into(),
            }
        );
        assert!(state.active_editor().expect("active editor").is_saving);
    }

    #[test]
    fn file_saved_event_clears_dirty_state() {
        let (command_tx, _command_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.open_editor_tab("/srv/app/README.md");
        state.workspace.apply_editor_content("/srv/app/README.md", "hello".into());
        state.workspace.apply_editor_action(
            "/srv/app/README.md",
            text_editor::Action::Move(text_editor::Motion::DocumentEnd),
        );
        state.workspace.apply_editor_action(
            "/srv/app/README.md",
            text_editor::Action::Edit(text_editor::Edit::Insert('!')),
        );
        state.workspace.mark_editor_saving("/srv/app/README.md");

        event_tx
            .send(SessionEvent::FileSaved {
                path: "/srv/app/README.md".into(),
            })
            .expect("queue save event");

        let _ = super::update(&mut state, Message::Tick(Instant::now()));

        let editor = state.active_editor().expect("active editor");
        assert!(!editor.is_dirty);
        assert!(!editor.is_saving);
        assert_eq!(editor.saved_content, "hello!");
    }

    #[test]
    fn file_save_failure_preserves_dirty_state() {
        let (command_tx, _command_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.open_editor_tab("/srv/app/README.md");
        state.workspace.apply_editor_content("/srv/app/README.md", "hello".into());
        state.workspace.apply_editor_action(
            "/srv/app/README.md",
            text_editor::Action::Move(text_editor::Motion::DocumentEnd),
        );
        state.workspace.apply_editor_action(
            "/srv/app/README.md",
            text_editor::Action::Edit(text_editor::Edit::Insert('!')),
        );
        state.workspace.mark_editor_saving("/srv/app/README.md");

        event_tx
            .send(SessionEvent::FileSaveFailed {
                path: "/srv/app/README.md".into(),
                error: "permission denied".into(),
            })
            .expect("queue save error");

        let _ = super::update(&mut state, Message::Tick(Instant::now()));

        let editor = state.active_editor().expect("active editor");
        assert!(editor.is_dirty);
        assert!(!editor.is_saving);
    }

    #[test]
    fn reuses_existing_editor_tab_for_same_path() {
        let (command_tx, command_rx) = unbounded();
        let (_event_tx, event_rx) = unbounded();
        let mut state = workspace_state_with_session(SessionHandle::from_channels(command_tx, event_rx));

        state.workspace.files = vec![text_file_entry("/srv/app/README.md", "README.md")];
        state.workspace.selected_file = Some("/srv/app/README.md".into());

        let _ = super::update(&mut state, Message::OpenSelectedFileInEditor);
        state.workspace.apply_editor_content("/srv/app/README.md", "# RustSSH\n".into());
        let _ = command_rx.try_recv();

        let _ = super::update(&mut state, Message::OpenSelectedFileInEditor);

        assert_eq!(state.workspace.editor_tabs.len(), 1);
        assert!(state.workspace.editor_tabs[0].is_loading);
    }

    fn workspace_state_with_session(session: SessionHandle) -> AppState {
        let (mut state, _) = AppState::boot();
        state.route = Route::Workspace;
        state.workspace.session = Some(session);
        state
    }

    fn text_file_entry(path: &str, name: &str) -> FileEntry {
        FileEntry {
            name: name.into(),
            path: path.into(),
            kind: FileKind::File,
            size: 128,
            permissions: "-rw-r--r--".into(),
            owner: Some("root".into()),
            modified: None,
        }
    }

    fn directory_entry(path: &str, name: &str) -> FileEntry {
        FileEntry {
            name: name.into(),
            path: path.into(),
            kind: FileKind::Directory,
            size: 0,
            permissions: "drwxr-xr-x".into(),
            owner: Some("root".into()),
            modified: None,
        }
    }
}
