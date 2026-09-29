use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use iced::widget::{operation, scrollable};
use iced::{Event, Subscription, Task, event, keyboard, mouse, time, window};

use crate::app::messages::{FileActionKind, Message};
use crate::app::state::{
    AppState, Confirmation, Modal, NotificationLevel, PendingFileAction, Route, WorkspaceState,
};
use crate::models::{AuthType, HostRecord, TransferDirection, TransferStatus, WorkspaceTab};
use crate::sftp::file_tree::{
    DirectoryHint, collapse_segments, normalize_remote_path, parse_cd_command,
};
use crate::sftp::transfers::{clear_finished_transfers, merge_transfer};
use crate::ssh::session::{self, ConnectionConfig, SessionCommand, SessionEvent};
use crate::ssh::terminal;
use crate::ui;

const TERMINAL_CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(530);
const NOTIFICATION_PRUNE_INTERVAL: Duration = Duration::from_secs(1);
const LINES_PER_SCROLL_STEP: f32 = 3.0;
pub const PROJECT_URL: &str = "https://github.com/jaggerjack61/RustSSHClient";
const ISSUES_URL: &str = "https://github.com/jaggerjack61/RustSSHClient/issues";

pub fn subscription(state: &AppState) -> Subscription<Message> {
    let mut subscriptions = vec![event::listen_with(runtime_event_filter)];

    if let Some(session) = &state.workspace.session {
        subscriptions.push(
            session
                .events()
                .with(session.id())
                .map(|(id, event)| Message::Session { id, event }),
        );
    }

    // Only tick when something actually animates or expires; an idle app
    // should not wake up at all.
    let cursor_blinks = state.is_connected()
        && state.modal.is_none()
        && matches!(state.workspace.active_tab, WorkspaceTab::Terminal)
        && state.workspace.terminal.scroll_offset() == 0;
    if cursor_blinks {
        subscriptions.push(time::every(TERMINAL_CURSOR_BLINK_INTERVAL).map(Message::Tick));
    } else if !state.notifications.is_empty() {
        subscriptions.push(time::every(NOTIFICATION_PRUNE_INTERVAL).map(Message::Tick));
    }

    Subscription::batch(subscriptions)
}

/// Forwards only the runtime events the app handles. Listening to everything
/// would rebuild the whole view on every mouse move.
fn runtime_event_filter(
    event: Event,
    status: event::Status,
    _window: window::Id,
) -> Option<Message> {
    match &event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Escape),
            ..
        }) => Some(Message::RuntimeEvent(event)),
        Event::Keyboard(keyboard::Event::KeyPressed { .. }) if status == event::Status::Ignored => {
            Some(Message::RuntimeEvent(event))
        }
        Event::Window(window::Event::FileDropped(_)) => Some(Message::RuntimeEvent(event)),
        _ => None,
    }
}

pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::StorageLoaded(result) => {
            match result {
                Ok(snapshot) => {
                    state.hosts = snapshot.hosts;
                    state.keys = snapshot.keys;
                }
                Err(error) => state.notification(
                    NotificationLevel::Error,
                    format!("Unable to load saved connections: {error}"),
                ),
            }
            Task::none()
        }

        // --- Connection form ---------------------------------------------
        Message::LoginLabelChanged(value) => {
            state.login.label = value;
            Task::none()
        }
        Message::LoginHostChanged(value) => {
            state.login.host = value;
            state.login.error = None;
            Task::none()
        }
        Message::LoginPortChanged(value) => {
            state.login.port = value.chars().filter(char::is_ascii_digit).take(5).collect();
            Task::none()
        }
        Message::LoginUsernameChanged(value) => {
            state.login.username = value;
            state.login.error = None;
            Task::none()
        }
        Message::LoginPasswordChanged(value) => {
            state.login.password = value;
            state.login.error = None;
            Task::none()
        }
        Message::TogglePasswordVisibility => {
            state.login.password_visible = !state.login.password_visible;
            Task::none()
        }
        Message::ToggleSaveConnection(value) => {
            state.login.save_connection = value;
            Task::none()
        }
        Message::UsePasswordAuthentication => {
            if state.login.auth_type != AuthType::Password {
                state.login.auth_type = AuthType::Password;
                state.login.password.clear();
            }
            Task::none()
        }
        Message::UseKeyAuthentication => {
            if state.login.auth_type != AuthType::Key {
                state.login.auth_type = AuthType::Key;
                state.login.password.clear();
            }
            if state.selected_key().is_none() {
                state.login.selected_key = state.keys.first().map(|key| key.id);
            }
            Task::none()
        }
        Message::SelectSaveLifetime(value) => {
            state.login.save_lifetime = value;
            Task::none()
        }
        Message::NewConnection => {
            if !state.login.connecting {
                state.login = Default::default();
            }
            Task::none()
        }
        Message::ConnectPressed => begin_connect(state),
        Message::SessionSpawned(result) => {
            // The user may have cancelled while the worker was starting;
            // dropping the handle then shuts it down.
            if state.login.connecting && state.workspace.session.is_none() {
                match result {
                    Ok(handle) => {
                        state.workspace = WorkspaceState {
                            session: Some(handle),
                            ..WorkspaceState::default()
                        };
                    }
                    Err(error) => {
                        state.login.connecting = false;
                        state.login.error = Some(error);
                    }
                }
            }
            Task::none()
        }
        Message::CancelConnect => {
            // Dropping the handle makes the worker exit once it notices.
            if state.route == Route::Login {
                if let Some(session) = state.workspace.session.take() {
                    let _ = session.send(SessionCommand::Disconnect);
                }
                state.login.connecting = false;
                if matches!(state.modal, Some(Modal::HostKey(_))) {
                    state.modal = None;
                }
            }
            Task::none()
        }
        Message::OpenProjectLink => open_link(state, PROJECT_URL),
        Message::OpenIssuesLink => open_link(state, ISSUES_URL),

        // --- Saved hosts and keys ------------------------------------------
        Message::HostFilterChanged(value) => {
            state.host_filter = value;
            Task::none()
        }
        Message::HostSortChanged(sort) => {
            state.host_sort = sort;
            Task::none()
        }
        Message::HostSelected(id) => {
            if !state.login.connecting
                && let Some(host) = state.hosts.iter().find(|host| host.id == id).cloned()
            {
                state.apply_host_to_form(&host);
            }
            Task::none()
        }
        Message::HostActivated(id) => {
            if state.login.connecting {
                return Task::none();
            }
            if let Some(host) = state.hosts.iter().find(|host| host.id == id).cloned() {
                state.apply_host_to_form(&host);
                return begin_connect(state);
            }
            Task::none()
        }
        Message::DeleteHost(id) => {
            if let Some(host) = state.hosts.iter().find(|host| host.id == id) {
                state.modal = Some(Modal::Confirm(Confirmation::DeleteHost {
                    id,
                    label: host.label.clone(),
                }));
            }
            Task::none()
        }
        Message::SelectKey(id) => {
            state.login.selected_key = Some(id);
            state.login.auth_type = AuthType::Key;
            state.login.error = None;
            Task::none()
        }
        Message::DeleteKey(id) => {
            if let Some(key) = state.keys.iter().find(|key| key.id == id) {
                let used_by = state
                    .hosts
                    .iter()
                    .filter(|host| host.key_reference == Some(id))
                    .count();
                state.modal = Some(Modal::Confirm(Confirmation::DeleteKey {
                    id,
                    label: key.label.clone(),
                    used_by,
                }));
            }
            Task::none()
        }
        Message::ImportKeyPressed => Task::perform(import_key_dialog(), Message::KeyImported),
        Message::KeyImported(result) => match result {
            Ok(Some(key)) => {
                state.notification(
                    NotificationLevel::Success,
                    format!("Imported key \u{201c}{}\u{201d}.", key.label),
                );
                state.login.selected_key = Some(key.id);
                state.login.auth_type = AuthType::Key;
                state.login.error = None;
                state.keys.push(key);
                persist_snapshot(state)
            }
            Ok(None) => Task::none(),
            Err(error) => {
                state.notification(NotificationLevel::Error, error);
                Task::none()
            }
        },

        // --- Session plumbing ----------------------------------------------
        Message::Session { id, event } => {
            let is_current = state
                .workspace
                .session
                .as_ref()
                .is_some_and(|session| session.id() == id);
            if is_current {
                handle_session_event(state, event)
            } else {
                Task::none()
            }
        }
        Message::HostKeyDecision(accepted) => {
            if matches!(state.modal, Some(Modal::HostKey(_))) {
                state.modal = None;
                send_session_command(state, SessionCommand::HostKeyDecision(accepted));
            }
            Task::none()
        }
        Message::Tick(now) => {
            state.prune_notifications();

            if state.route == Route::Workspace {
                let recently_active = now.duration_since(state.workspace.last_terminal_activity)
                    < TERMINAL_CURSOR_BLINK_INTERVAL;
                state.workspace.terminal_cursor_visible =
                    recently_active || !state.workspace.terminal_cursor_visible;
            }
            Task::none()
        }
        Message::RuntimeEvent(event) => handle_runtime_event(state, event),

        // --- Terminal --------------------------------------------------------
        Message::TerminalViewportResized(size) => {
            let (cols, rows) = ui::terminal::grid_size(size);
            if state.workspace.terminal.resize(rows, cols) {
                send_session_command(
                    state,
                    SessionCommand::ResizeTerminal {
                        cols: u32::from(cols),
                        rows: u32::from(rows),
                    },
                );
            }
            Task::none()
        }
        Message::TerminalScrolled(delta) => {
            scroll_terminal(state, delta);
            Task::none()
        }
        Message::TerminalScrollToBottom => {
            state.workspace.terminal.scroll_to_bottom();
            Task::none()
        }
        Message::ClearTerminal => {
            state.workspace.terminal.clear();
            // Ctrl+L asks the shell to redraw its prompt on the fresh screen.
            send_session_command(state, SessionCommand::SendInput(vec![0x0C]));
            Task::none()
        }
        Message::CopyTerminalOutput => copy_terminal_output(state),
        Message::PasteTerminalInput => handle_terminal_paste(state),
        Message::DisconnectPressed => {
            let unsaved = state.unsaved_editor_count();
            if unsaved > 0 {
                state.modal = Some(Modal::Confirm(Confirmation::Disconnect { unsaved }));
            } else {
                disconnect(state);
            }
            Task::none()
        }

        // --- Explorer --------------------------------------------------------
        Message::RefreshDirectory => {
            state.workspace.explorer_context_for = None;
            let current = state.workspace.current_directory.clone();
            request_directory_refresh(state, current);
            Task::none()
        }
        Message::NavigateUpDirectory => {
            state.workspace.explorer_context_for = None;
            let parent = parent_directory(&state.workspace.current_directory);
            if parent != state.workspace.current_directory {
                request_directory_refresh(state, parent);
            }
            Task::none()
        }
        Message::NavigateHome => {
            state.workspace.explorer_context_for = None;
            request_directory_refresh(state, "~".into());
            Task::none()
        }
        Message::NavigateTo(path) => {
            state.workspace.explorer_context_for = None;
            request_directory_refresh(state, path);
            Task::none()
        }
        Message::DismissExplorerContextMenu => {
            state.workspace.explorer_context_for = None;
            Task::none()
        }
        Message::ExplorerScrolled(offset) => {
            state.workspace.explorer_scroll_offset = offset;
            Task::none()
        }
        Message::ExplorerEntryPressed(path) => {
            state.workspace.selected_file = Some(path.clone());
            state.workspace.explorer_context_for = None;
            if state.file(&path).is_some_and(|entry| entry.is_directory()) {
                toggle_folder(state, path);
            }
            Task::none()
        }
        Message::ExplorerEntryDoubleClicked(path) => {
            state.workspace.selected_file = Some(path.clone());
            state.workspace.explorer_context_for = None;
            match state.file(&path).map(|entry| entry.is_directory()) {
                Some(true) => {
                    request_directory_refresh(state, path);
                    Task::none()
                }
                Some(false) => open_selected_file_in_editor(state),
                None => Task::none(),
            }
        }
        Message::ExplorerEntrySecondaryPressed(path) => {
            state.workspace.selected_file = Some(path.clone());
            state.workspace.explorer_context_for = Some(path);
            Task::none()
        }
        Message::ShowProperties => {
            state.workspace.explorer_context_for = None;
            if let Some(path) = state.workspace.selected_file.clone() {
                state.modal = Some(Modal::Properties(path));
            }
            Task::none()
        }
        Message::OpenSelectedFileInEditor => open_selected_file_in_editor(state),
        Message::UploadRequested => {
            state.workspace.explorer_context_for = None;
            Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Upload files")
                        .pick_files()
                        .await
                        .map(|files| files.into_iter().map(PathBuf::from).collect())
                },
                Message::FilesSelected,
            )
        }
        Message::FilesSelected(files) => {
            if let Some(paths) = files.filter(|paths| !paths.is_empty()) {
                upload_paths(state, paths);
            }
            Task::none()
        }
        Message::DownloadRequested => {
            state.workspace.explorer_context_for = None;
            if state.selected_file().is_none() {
                state.notification(
                    NotificationLevel::Info,
                    "Select a remote file or folder first.",
                );
                return Task::none();
            }
            Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Download to folder")
                        .pick_folder()
                        .await
                        .map(PathBuf::from)
                },
                Message::DownloadDirectorySelected,
            )
        }
        Message::DownloadDirectorySelected(local_directory) => {
            if let Some(local_directory) = local_directory {
                let Some(selected) = state.selected_file().cloned() else {
                    return Task::none();
                };

                send_session_command(
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
            if let Some(selected) = state.selected_file() {
                state.modal = Some(Modal::Confirm(Confirmation::DeleteRemote {
                    path: selected.path.clone(),
                    is_directory: selected.is_directory(),
                }));
            }
            Task::none()
        }
        Message::StartFileAction(kind) => {
            state.workspace.explorer_context_for = None;
            let selected = state.selected_file().cloned();
            let action = match kind {
                FileActionKind::NewFolder => PendingFileAction {
                    kind,
                    source: selected
                        .filter(|entry| entry.is_directory())
                        .map(|entry| entry.path)
                        .unwrap_or_else(|| state.workspace.current_directory.clone()),
                    value: String::new(),
                },
                FileActionKind::Rename => {
                    let Some(entry) = selected else {
                        return Task::none();
                    };
                    PendingFileAction {
                        kind,
                        value: entry.name,
                        source: entry.path,
                    }
                }
                FileActionKind::Copy | FileActionKind::Move => {
                    let Some(entry) = selected else {
                        return Task::none();
                    };
                    PendingFileAction {
                        kind,
                        value: entry.path.clone(),
                        source: entry.path,
                    }
                }
            };
            state.modal = Some(Modal::FileAction(action));
            operation::focus(ui::modals::FILE_ACTION_INPUT)
        }
        Message::FileActionInputChanged(value) => {
            if let Some(Modal::FileAction(action)) = &mut state.modal {
                action.value = value;
            }
            Task::none()
        }
        Message::ConfirmFileAction => {
            confirm_file_action(state);
            Task::none()
        }
        Message::ToggleTransfersPanel => {
            state.workspace.transfers_expanded = !state.workspace.transfers_expanded;
            Task::none()
        }
        Message::ClearFinishedTransfers => {
            clear_finished_transfers(&mut state.workspace.transfers);
            Task::none()
        }

        // --- Editor ----------------------------------------------------------
        Message::EditorAction(path, action) => {
            state.workspace.apply_editor_action(&path, action);
            Task::none()
        }
        Message::SaveActiveEditor => save_active_editor(state),
        Message::ActivateTerminalTab => {
            state.workspace.active_tab = WorkspaceTab::Terminal;
            Task::none()
        }
        Message::ActivateEditorTab(path) => {
            if state.workspace.has_editor_tab(&path) {
                state.workspace.active_tab = WorkspaceTab::Editor(path);
            }
            Task::none()
        }
        Message::CloseEditorTab(path) => {
            let is_dirty = state
                .workspace
                .editor_tabs
                .iter()
                .any(|tab| tab.path == path && tab.is_dirty);
            if is_dirty {
                state.modal = Some(Modal::Confirm(Confirmation::CloseTab { path }));
            } else {
                state.workspace.close_editor_tab(&path);
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
            if url.starts_with("http://") || url.starts_with("https://") {
                return open_link(state, &url);
            }
            Task::none()
        }

        // --- Dialogs and toasts ----------------------------------------------
        Message::ConfirmModal => confirm_modal(state),
        Message::CloseModal => {
            if let Some(Modal::HostKey(_)) = state.modal {
                send_session_command(state, SessionCommand::HostKeyDecision(false));
            }
            state.modal = None;
            Task::none()
        }
        Message::DismissNotification(id) => {
            state.dismiss_notification(id);
            Task::none()
        }
    }
}

fn handle_runtime_event(state: &mut AppState, event: Event) -> Task<Message> {
    match event {
        Event::Window(window::Event::FileDropped(path)) => {
            if state.is_connected() && state.modal.is_none() {
                upload_paths(state, vec![path]);
            }
            Task::none()
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            text,
            ..
        }) => handle_key_press(state, key, modifiers, text.as_deref()),
        _ => Task::none(),
    }
}

fn handle_key_press(
    state: &mut AppState,
    key: Key,
    modifiers: Modifiers,
    text: Option<&str>,
) -> Task<Message> {
    let is_escape = key == Key::Named(Named::Escape);
    let is_tab = key == Key::Named(Named::Tab);

    // Dialogs and menus take precedence over everything else.
    if state.modal.is_some() {
        if is_escape {
            return update(state, Message::CloseModal);
        }
        if is_tab {
            return focus_step(modifiers);
        }
        return Task::none();
    }

    if is_escape && state.workspace.explorer_context_for.is_some() {
        state.workspace.explorer_context_for = None;
        return Task::none();
    }

    if state.route == Route::Login {
        if is_tab {
            return focus_step(modifiers);
        }
        if is_escape && state.login.connecting {
            return update(state, Message::CancelConnect);
        }
        return Task::none();
    }

    if !state.is_connected() {
        return Task::none();
    }

    let character = match &key {
        Key::Character(value) => Some(value.to_ascii_lowercase()),
        _ => None,
    };
    let shortcut = |expected: &str| character.as_deref() == Some(expected);

    // Editor tabs: only app shortcuts; the text editor handles typing.
    if matches!(state.workspace.active_tab, WorkspaceTab::Editor(_)) {
        if modifiers.command() && !modifiers.shift() && shortcut("s") {
            return save_active_editor(state);
        }
        return Task::none();
    }

    // Terminal: Cmd+C/V on macOS, Ctrl+Shift+C/V elsewhere (Ctrl+C must still
    // reach the shell as SIGINT).
    let app_shortcut = modifiers.logo() || (modifiers.control() && modifiers.shift());
    if app_shortcut && shortcut("c") {
        return copy_terminal_output(state);
    }
    if (app_shortcut && shortcut("v")) || (modifiers.shift() && key == Key::Named(Named::Insert)) {
        return handle_terminal_paste(state);
    }

    // Keep the explorer in sync with `cd` typed into the shell.
    if key == Key::Named(Named::Enter) {
        let line = state.workspace.terminal.current_cursor_line();
        let command = terminal::extract_command_from_prompt_line(&line);
        if let Some(hint) = parse_cd_command(&state.workspace.current_directory, command) {
            let request = match hint {
                DirectoryHint::ResolveHome => "~".to_string(),
                DirectoryHint::HomeRelative(sub) => format!("~/{sub}"),
                DirectoryHint::Absolute(path) => path,
            };
            request_directory_refresh(state, request);
        }
    }

    let application_cursor = state.workspace.terminal.application_cursor();
    if let Some(bytes) = terminal::key_to_bytes(&key, modifiers, text, application_cursor) {
        send_terminal_input(state, bytes);
    }
    Task::none()
}

fn focus_step(modifiers: Modifiers) -> Task<Message> {
    if modifiers.shift() {
        operation::focus_previous()
    } else {
        operation::focus_next()
    }
}

fn handle_session_event(state: &mut AppState, event: SessionEvent) -> Task<Message> {
    match event {
        SessionEvent::HostKeyUnknown(info) => {
            state.modal = Some(Modal::HostKey(info));
        }
        SessionEvent::Connected {
            cwd,
            latency_ms,
            peer,
        } => {
            state.route = Route::Workspace;
            state.login.connecting = false;
            state.login.error = None;
            state.workspace.current_directory = cwd.clone();
            state.workspace.pending_directory = Some(cwd);
            state.workspace.connected_peer = peer;
            state.workspace.latency_ms = Some(latency_ms);
            state.workspace.last_terminal_activity = Instant::now();
            state.notification(
                NotificationLevel::Success,
                format!(
                    "Connected to {}@{}",
                    state.login.username.trim(),
                    state.workspace.connected_peer
                ),
            );
        }
        SessionEvent::Output(bytes) => {
            state.workspace.terminal.feed(&bytes);
            state.workspace.terminal_cursor_visible = true;
            state.workspace.last_terminal_activity = Instant::now();
        }
        SessionEvent::DirectoryLoaded {
            request,
            cwd,
            entries,
        } => return apply_directory_listing(state, request, cwd, entries),
        SessionEvent::DirectoryChildrenLoaded { directory, entries } => {
            state.workspace.loading_folders.remove(&directory);
            state.workspace.loaded_folders.insert(directory.clone());
            merge_directory_children(&mut state.workspace.files, &directory, entries);

            // Restore nested folders that were expanded before a refresh.
            let reopen = state
                .workspace
                .files
                .iter()
                .filter(|entry| {
                    entry.is_directory()
                        && is_descendant_path(&entry.path, &directory)
                        && state.workspace.expanded_folders.contains(&entry.path)
                        && !state.workspace.loaded_folders.contains(&entry.path)
                })
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>();
            for path in reopen {
                request_directory_children(state, path);
            }
        }
        SessionEvent::FileOpened { path, contents } => {
            state.workspace.apply_editor_content(&path, contents);
        }
        SessionEvent::FileOpenFailed { path, error } => {
            state.workspace.fail_editor_load(&path, error);
        }
        SessionEvent::FileSaved { path } => {
            state.workspace.mark_editor_saved(&path);
            state.notification(
                NotificationLevel::Success,
                format!("Saved {}", crate::models::editor_title(&path)),
            );
        }
        SessionEvent::FileSaveFailed { path, error } => {
            state.workspace.mark_editor_save_failed(&path);
            state.notification(
                NotificationLevel::Error,
                format!("Unable to save {path}: {error}"),
            );
        }
        SessionEvent::DirectoryOpenFailed { request, error } => {
            if state.workspace.pending_directory.as_deref() == Some(request.as_str()) {
                state.workspace.pending_directory = None;
                state.notification(
                    NotificationLevel::Error,
                    format!("Unable to open {request}: {error}"),
                );
            }
        }
        SessionEvent::DirectoryChildrenLoadFailed { directory, error } => {
            state.workspace.loading_folders.remove(&directory);
            state.workspace.expanded_folders.remove(&directory);
            state.notification(
                NotificationLevel::Error,
                format!("Unable to open {directory}: {error}"),
            );
        }
        SessionEvent::Transfer(update) => {
            let was_running = state
                .workspace
                .transfers
                .iter()
                .any(|item| item.id == update.id && !item.is_finished());
            merge_transfer(&update, &mut state.workspace.transfers);
            state.workspace.transfers_expanded |= !was_running && !update.is_finished();

            if was_running || !update.is_finished() {
                match &update.status {
                    TransferStatus::Failed(error) => state.notification(
                        NotificationLevel::Error,
                        format!("{} failed: {error}", update.label),
                    ),
                    TransferStatus::Completed => {
                        let verb = match update.direction {
                            TransferDirection::Upload => "Uploaded",
                            TransferDirection::Download => "Downloaded",
                            TransferDirection::Copy => "Copied",
                        };
                        state.notification(
                            NotificationLevel::Success,
                            format!("{verb} {}", update.label),
                        );
                        // Show new remote files without a manual refresh.
                        if update.direction != TransferDirection::Download {
                            let current = state.workspace.current_directory.clone();
                            request_directory_refresh(state, current);
                        }
                    }
                    TransferStatus::Queued | TransferStatus::Running => {}
                }
            }
        }
        SessionEvent::Error(error) => state.notification(NotificationLevel::Error, error),
        SessionEvent::Disconnected(reason) => handle_disconnected(state, reason),
    }

    Task::none()
}

fn handle_disconnected(state: &mut AppState, reason: String) {
    let was_connecting = state.route == Route::Login;
    let lost_edits = state.unsaved_editor_count();
    let ended_normally =
        reason == session::DISCONNECTED_BY_USER || reason == session::REMOTE_SHELL_EXITED;

    state.workspace = WorkspaceState::default();
    state.route = Route::Login;
    state.login.connecting = false;
    if matches!(state.modal, Some(Modal::HostKey(_))) || !was_connecting {
        state.modal = None;
    }

    if was_connecting {
        state.login.error = Some(reason);
        return;
    }

    if ended_normally {
        state.notification(NotificationLevel::Info, format!("{reason}."));
    } else {
        state.notification(NotificationLevel::Error, format!("Session ended: {reason}"));
    }
    if lost_edits > 0 {
        state.notification(
            NotificationLevel::Error,
            format!("Unsaved changes in {lost_edits} file(s) were lost."),
        );
    }
}

fn apply_directory_listing(
    state: &mut AppState,
    request: String,
    cwd: String,
    entries: Vec<crate::models::FileEntry>,
) -> Task<Message> {
    // Discard stale listings while waiting for a specific directory.
    if let Some(pending) = &state.workspace.pending_directory
        && *pending != request
        && *pending != cwd
    {
        return Task::none();
    }

    let workspace = &mut state.workspace;
    let same_directory = workspace.current_directory == cwd;
    workspace.current_directory = cwd;
    workspace.pending_directory = None;
    workspace.files = entries;
    workspace.explorer_context_for = None;
    workspace.loaded_folders.clear();
    workspace.loading_folders.clear();

    let files = &workspace.files;
    workspace.selected_file = workspace
        .selected_file
        .take()
        .filter(|selected| files.iter().any(|entry| entry.path == *selected));

    if !same_directory {
        workspace.expanded_folders.clear();
        workspace.explorer_scroll_offset = 0.0;
        return operation::scroll_to(
            ui::file_tree::TREE_SCROLLABLE,
            scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
        );
    }

    // A refresh of the same directory keeps the tree's expansion state.
    let reopen = workspace
        .files
        .iter()
        .filter(|entry| entry.is_directory() && workspace.expanded_folders.contains(&entry.path))
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    for path in reopen {
        request_directory_children(state, path);
    }
    Task::none()
}

fn toggle_folder(state: &mut AppState, path: String) {
    let workspace = &mut state.workspace;
    if workspace.expanded_folders.remove(&path) {
        return;
    }

    workspace.expanded_folders.insert(path.clone());
    if !workspace.loaded_folders.contains(&path) {
        request_directory_children(state, path);
    }
}

fn scroll_terminal(state: &mut AppState, delta: mouse::ScrollDelta) {
    let lines = match delta {
        mouse::ScrollDelta::Lines { y, .. } => y * LINES_PER_SCROLL_STEP,
        mouse::ScrollDelta::Pixels { y, .. } => y / ui::terminal::LINE_HEIGHT,
    } + state.workspace.terminal_scroll_remainder;
    let whole = lines.trunc();
    state.workspace.terminal_scroll_remainder = lines - whole;
    let whole = whole as i32;
    if whole == 0 {
        return;
    }

    let terminal = &mut state.workspace.terminal;
    if terminal.alternate_screen() {
        // Full-screen programs (less, vim, htop) have no scrollback; translate
        // the wheel into cursor keys like most terminal emulators do.
        let key = Key::Named(if whole > 0 {
            Named::ArrowUp
        } else {
            Named::ArrowDown
        });
        let application_cursor = terminal.application_cursor();
        if let Some(bytes) =
            terminal::key_to_bytes(&key, Modifiers::empty(), None, application_cursor)
        {
            let repeated = bytes.repeat(whole.unsigned_abs().min(10) as usize);
            send_session_command(state, SessionCommand::SendInput(repeated));
        }
    } else {
        terminal.scroll_by(whole);
    }
}

fn persist_snapshot(state: &mut AppState) -> Task<Message> {
    if let Err(error) = state.storage.save_snapshot(&state.snapshot()) {
        state.notification(
            NotificationLevel::Error,
            format!("Unable to save connections: {error}"),
        );
    }
    Task::none()
}

async fn import_key_dialog() -> Result<Option<crate::models::SshKeyRecord>, String> {
    let mut dialog = rfd::AsyncFileDialog::new().set_title("Import SSH private key");
    if let Some(ssh_dir) = directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".ssh"))
        .filter(|dir| dir.is_dir())
    {
        dialog = dialog.set_directory(ssh_dir);
    }

    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };
    let path = PathBuf::from(file);

    let label = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported key".into());
    let pem = std::fs::read_to_string(&path)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    if !pem.contains("PRIVATE KEY-----") {
        return Err(format!(
            "{label} is not a PEM or OpenSSH private key. Public keys (.pub) and PuTTY keys (.ppk) are not supported."
        ));
    }
    Ok(Some(crate::models::SshKeyRecord::new(label, pem)))
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

fn send_terminal_input(state: &mut AppState, bytes: Vec<u8>) {
    state.workspace.terminal.scroll_to_bottom();
    state.workspace.terminal_cursor_visible = true;
    state.workspace.last_terminal_activity = Instant::now();
    send_session_command(state, SessionCommand::SendInput(bytes));
}

fn disconnect(state: &mut AppState) {
    if !send_session_command(state, SessionCommand::Disconnect) {
        handle_disconnected(state, session::DISCONNECTED_BY_USER.into());
    }
}

/// Lists `request` (an absolute path, `~` or `~/...`) as the explorer root.
fn request_directory_refresh(state: &mut AppState, request: String) -> bool {
    state.workspace.pending_directory = Some(request.clone());
    if send_session_command(state, SessionCommand::RefreshDirectory(request)) {
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

fn merge_directory_children(
    files: &mut Vec<crate::models::FileEntry>,
    directory: &str,
    entries: Vec<crate::models::FileEntry>,
) {
    files.retain(|entry| entry.path == directory || !is_descendant_path(&entry.path, directory));
    files.extend(entries);
}

fn is_descendant_path(path: &str, directory: &str) -> bool {
    let prefix = format!("{}/", directory.trim_end_matches('/'));
    path.starts_with(&prefix)
}

fn parent_directory(current_directory: &str) -> String {
    normalize_remote_path(current_directory, "..")
}

fn remote_parent(path: &str, fallback: &str) -> String {
    Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .filter(|parent| !parent.is_empty())
        .unwrap_or_else(|| fallback.to_string())
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
            let parent = remote_parent(selected_path, current_directory);
            normalize_remote_path(&parent, &trimmed)
        }
        // For new folders the "selected path" is the parent directory.
        FileActionKind::NewFolder => normalize_remote_path(selected_path, &trimmed),
        FileActionKind::Copy | FileActionKind::Move => {
            normalize_remote_path(current_directory, &trimmed)
        }
    }
}

fn normalize_remote_path_input(value: &str) -> String {
    value.trim().replace('\\', "/")
}

fn confirm_file_action(state: &mut AppState) {
    let Some(Modal::FileAction(action)) = state.modal.clone() else {
        return;
    };
    if action.value.trim().is_empty() {
        state.notification(
            NotificationLevel::Info,
            match action.kind {
                FileActionKind::NewFolder | FileActionKind::Rename => "Enter a name.",
                FileActionKind::Copy | FileActionKind::Move => "Enter a target path.",
            },
        );
        return;
    }
    if state.workspace.session.is_none() {
        return;
    }

    let target = resolve_file_action_target(
        &state.workspace.current_directory,
        &action.source,
        &action.value,
        action.kind,
    );
    if action.kind != FileActionKind::NewFolder && target == action.source {
        state.modal = None;
        return;
    }

    let command = match action.kind {
        FileActionKind::Rename | FileActionKind::Move => SessionCommand::Rename {
            source: action.source,
            target: target.clone(),
        },
        FileActionKind::Copy => SessionCommand::Copy {
            source: action.source,
            target: target.clone(),
        },
        FileActionKind::NewFolder => SessionCommand::CreateDirectory {
            remote_path: target.clone(),
        },
    };

    if send_session_command(state, command) {
        state.modal = None;
        if matches!(action.kind, FileActionKind::Rename | FileActionKind::Move) {
            state.workspace.selected_file = Some(target);
        }
    }
}

fn confirm_modal(state: &mut AppState) -> Task<Message> {
    let Some(modal) = state.modal.take() else {
        return Task::none();
    };

    match modal {
        Modal::Confirm(Confirmation::DeleteRemote { path, .. }) => {
            if send_session_command(
                state,
                SessionCommand::Delete {
                    remote_path: path.clone(),
                },
            ) && state.workspace.selected_file.as_deref() == Some(path.as_str())
            {
                state.workspace.selected_file = None;
            }
            Task::none()
        }
        Modal::Confirm(Confirmation::DeleteHost { id, .. }) => {
            state.hosts.retain(|host| host.id != id);
            if state.login.editing_host_id == Some(id) {
                state.login.editing_host_id = None;
            }
            persist_snapshot(state)
        }
        Modal::Confirm(Confirmation::DeleteKey { id, .. }) => {
            state.keys.retain(|key| key.id != id);
            if state.login.selected_key == Some(id) {
                state.login.selected_key = state.keys.first().map(|key| key.id);
            }
            persist_snapshot(state)
        }
        Modal::Confirm(Confirmation::CloseTab { path }) => {
            state.workspace.close_editor_tab(&path);
            Task::none()
        }
        Modal::Confirm(Confirmation::Disconnect { .. }) => {
            disconnect(state);
            Task::none()
        }
        Modal::HostKey(_) => {
            send_session_command(state, SessionCommand::HostKeyDecision(true));
            Task::none()
        }
        Modal::FileAction(action) => {
            state.modal = Some(Modal::FileAction(action));
            confirm_file_action(state);
            Task::none()
        }
        Modal::Properties(_) => Task::none(),
    }
}

fn upload_paths(state: &mut AppState, local_paths: Vec<PathBuf>) {
    let remote_directory = state
        .selected_file()
        .filter(|entry| entry.is_directory())
        .map(|entry| entry.path.clone())
        .unwrap_or_else(|| state.workspace.current_directory.clone());

    send_session_command(
        state,
        SessionCommand::Upload {
            local_paths,
            remote_directory,
        },
    );
}

fn handle_terminal_paste(state: &mut AppState) -> Task<Message> {
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) if !text.is_empty() => {
            let bracketed = state.workspace.terminal.bracketed_paste();
            send_terminal_input(state, terminal::paste_bytes(&text, bracketed));
        }
        Ok(_) => {}
        Err(error) => state.notification(
            NotificationLevel::Error,
            format!("Clipboard unavailable: {error}"),
        ),
    }
    Task::none()
}

fn begin_connect(state: &mut AppState) -> Task<Message> {
    if state.login.connecting || state.workspace.session.is_some() {
        return Task::none();
    }

    let request = match state.prepare_login_request() {
        Ok(request) => request,
        Err(error) => {
            state.login.error = Some(error);
            return Task::none();
        }
    };

    let mut persist = Task::none();
    if request.save_host {
        let mut host = state
            .login
            .editing_host_id
            .and_then(|id| state.hosts.iter().find(|host| host.id == id).cloned())
            .unwrap_or_else(|| HostRecord::new(&request));
        host.apply_request(&request);
        state.login.editing_host_id = Some(host.id);
        upsert_host(state, host);
        persist = persist_snapshot(state);
    }

    let config = ConnectionConfig {
        key: state.selected_key().cloned(),
        request,
        known_hosts: state.storage.known_hosts_path(),
    };
    state.login.connecting = true;
    state.login.error = None;

    Task::batch([
        persist,
        Task::perform(
            async move { crate::ssh::session::spawn(config).map_err(|error| error.to_string()) },
            Message::SessionSpawned,
        ),
    ])
}

fn open_selected_file_in_editor(state: &mut AppState) -> Task<Message> {
    state.workspace.explorer_context_for = None;

    let Some(selected) = state.selected_file().cloned() else {
        state.notification(NotificationLevel::Info, "Select a remote file first.");
        return Task::none();
    };

    if selected.is_directory() {
        state.notification(
            NotificationLevel::Info,
            "Folders cannot be opened in the editor.",
        );
        return Task::none();
    }

    // Re-opening an already loaded file just focuses it, so unsaved edits are
    // never replaced by the server copy.
    if let Some(tab) = state
        .workspace
        .editor_tabs
        .iter()
        .find(|tab| tab.path == selected.path)
        && tab.load_error.is_none()
    {
        state.workspace.active_tab = WorkspaceTab::Editor(selected.path);
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

fn save_active_editor(state: &mut AppState) -> Task<Message> {
    let Some(editor) = state.active_editor() else {
        return Task::none();
    };

    if editor.is_loading || editor.load_error.is_some() || !editor.is_dirty || editor.is_saving {
        return Task::none();
    }

    let path = editor.path.clone();
    let contents = editor.current_text();
    state.workspace.mark_editor_saving(&path, contents.clone());

    if !send_session_command(
        state,
        SessionCommand::WriteFile {
            remote_path: path.clone(),
            contents,
        },
    ) {
        state.workspace.mark_editor_save_failed(&path);
    }

    Task::none()
}

fn copy_terminal_output(state: &mut AppState) -> Task<Message> {
    let text = state.workspace.terminal.display_text();
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text)) {
        Ok(()) => state.notification(NotificationLevel::Info, "Copied terminal screen."),
        Err(error) => state.notification(
            NotificationLevel::Error,
            format!("Clipboard unavailable: {error}"),
        ),
    }

    Task::none()
}

fn open_link(state: &mut AppState, url: &str) -> Task<Message> {
    if let Err(error) = webbrowser::open(url) {
        state.notification(
            NotificationLevel::Error,
            format!("Unable to open {url}: {error}"),
        );
    }
    Task::none()
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crossbeam_channel::{Receiver, unbounded};
    use iced::widget::text_editor;

    use crate::app::messages::{FileActionKind, Message};
    use crate::app::state::{AppState, Confirmation, Modal, PendingFileAction, Route};
    use crate::models::{
        EditorLanguage, FileEntry, FileKind, SaveLifetime, TransferDirection, TransferProgress,
        TransferStatus, WorkspaceTab,
    };
    use crate::ssh::session::{SessionCommand, SessionEvent, SessionHandle};
    use crate::storage::StorageFacade;
    use tempfile::{TempDir, tempdir};

    fn test_state() -> (AppState, TempDir) {
        let dir = tempdir().expect("create tempdir");
        let state = AppState::with_storage(StorageFacade::for_root(dir.path().to_path_buf()));
        (state, dir)
    }

    fn connected_state() -> (AppState, Receiver<SessionCommand>, TempDir) {
        let (mut state, dir) = test_state();
        let (command_tx, command_rx) = unbounded();
        let (session, _events) = SessionHandle::from_channels(command_tx);
        state.route = Route::Workspace;
        state.workspace.session = Some(session);
        (state, command_rx, dir)
    }

    fn deliver(state: &mut AppState, event: SessionEvent) {
        let id = state.workspace.session.as_ref().expect("session").id();
        let _ = super::update(state, Message::Session { id, event });
    }

    fn fill_login_form(state: &mut AppState) {
        state.login.host = "prod.example.com".into();
        state.login.port = "22".into();
        state.login.username = "deploy".into();
        state.login.password = "secret".into();
    }

    #[test]
    fn updates_login_form_fields() {
        let (mut state, _dir) = test_state();
        let _ = super::update(
            &mut state,
            Message::LoginHostChanged("prod.example.com".into()),
        );
        let _ = super::update(&mut state, Message::LoginUsernameChanged("deploy".into()));
        let _ = super::update(&mut state, Message::LoginPortChanged("22a2x".into()));

        assert_eq!(state.login.host, "prod.example.com");
        assert_eq!(state.login.username, "deploy");
        assert_eq!(state.login.port, "222");
    }

    #[test]
    fn deleting_a_saved_host_requires_confirmation() {
        let (mut state, dir) = test_state();
        fill_login_form(&mut state);
        state.login.save_connection = true;

        let _ = super::update(&mut state, Message::ConnectPressed);
        let host_id = state.hosts[0].id;

        let _ = super::update(&mut state, Message::DeleteHost(host_id));
        assert_eq!(state.hosts.len(), 1);
        assert!(matches!(
            state.modal,
            Some(Modal::Confirm(Confirmation::DeleteHost { .. }))
        ));

        let _ = super::update(&mut state, Message::ConfirmModal);
        assert!(state.hosts.is_empty());

        let reloaded = StorageFacade::for_root(dir.path().to_path_buf())
            .load_snapshot()
            .expect("reload saved snapshot");
        assert!(reloaded.hosts.is_empty());
    }

    #[test]
    fn saves_host_immediately_on_connect_press() {
        let (mut state, dir) = test_state();
        fill_login_form(&mut state);
        state.login.save_connection = true;

        let _ = super::update(&mut state, Message::ConnectPressed);

        assert!(state.login.connecting);
        assert_eq!(state.route, Route::Login);
        assert_eq!(state.hosts.len(), 1);
        assert_eq!(state.hosts[0].password.as_deref(), Some("secret"));
        assert_eq!(state.login.editing_host_id, Some(state.hosts[0].id));

        let reloaded = StorageFacade::for_root(dir.path().to_path_buf())
            .load_snapshot()
            .expect("reload saved snapshot");
        assert_eq!(reloaded.hosts.len(), 1);
        assert_eq!(reloaded.hosts[0].host, "prod.example.com");
    }

    #[test]
    fn does_not_persist_host_when_save_connection_disabled() {
        let (mut state, dir) = test_state();
        fill_login_form(&mut state);
        state.login.save_connection = false;

        let _ = super::update(&mut state, Message::ConnectPressed);

        let reloaded = StorageFacade::for_root(dir.path().to_path_buf())
            .load_snapshot()
            .expect("reload snapshot");
        assert!(reloaded.hosts.is_empty());
    }

    #[test]
    fn connect_press_is_ignored_while_already_connecting() {
        let (mut state, _dir) = test_state();
        fill_login_form(&mut state);
        let _ = super::update(&mut state, Message::ConnectPressed);
        state.login.host = "other.example.com".into();

        let _ = super::update(&mut state, Message::ConnectPressed);

        assert!(state.login.connecting);
        assert_eq!(state.hosts.len(), 1);
        assert_eq!(state.hosts[0].host, "prod.example.com");
    }

    #[test]
    fn validation_errors_are_shown_inline() {
        let (mut state, _dir) = test_state();
        state.login.host = "example.com".into();

        let _ = super::update(&mut state, Message::ConnectPressed);

        assert_eq!(state.login.error.as_deref(), Some("Username is required."));
        assert!(!state.login.connecting);
    }

    #[test]
    fn stays_on_login_until_connected_then_switches_to_workspace() {
        let (mut state, _command_rx, _dir) = connected_state();
        state.route = Route::Login;
        state.login.connecting = true;

        deliver(
            &mut state,
            SessionEvent::Connected {
                cwd: "/home/deploy".into(),
                latency_ms: 12,
                peer: "prod".into(),
            },
        );

        assert_eq!(state.route, Route::Workspace);
        assert!(!state.login.connecting);
        assert_eq!(state.workspace.current_directory, "/home/deploy");
    }

    #[test]
    fn failed_connection_reports_error_on_login_form() {
        let (mut state, _command_rx, _dir) = connected_state();
        state.route = Route::Login;
        state.login.connecting = true;

        deliver(
            &mut state,
            SessionEvent::Disconnected("Authentication failed.".into()),
        );

        assert_eq!(state.route, Route::Login);
        assert!(state.workspace.session.is_none());
        assert_eq!(state.login.error.as_deref(), Some("Authentication failed."));
    }

    #[test]
    fn unknown_host_key_prompts_and_forwards_the_decision() {
        let (mut state, command_rx, _dir) = connected_state();
        state.route = Route::Login;
        let info = crate::ssh::client::HostKeyInfo {
            host: "prod".into(),
            port: 22,
            key_type: "ED25519".into(),
            fingerprint: "SHA256:abc".into(),
        };

        deliver(&mut state, SessionEvent::HostKeyUnknown(info));
        assert!(matches!(state.modal, Some(Modal::HostKey(_))));

        let _ = super::update(&mut state, Message::ConfirmModal);
        assert!(state.modal.is_none());
        assert_eq!(
            command_rx.try_recv().expect("decision"),
            SessionCommand::HostKeyDecision(true)
        );
    }

    #[test]
    fn events_from_a_previous_session_are_ignored() {
        let (mut state, _command_rx, _dir) = connected_state();
        let stale_id = state.workspace.session.as_ref().expect("session").id() + 1000;

        let _ = super::update(
            &mut state,
            Message::Session {
                id: stale_id,
                event: SessionEvent::Disconnected("old".into()),
            },
        );

        assert_eq!(state.route, Route::Workspace);
        assert!(state.workspace.session.is_some());
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
    fn new_folder_targets_are_relative_to_the_parent() {
        let target = super::resolve_file_action_target(
            "/srv/app",
            "/srv/app/logs",
            "archive",
            FileActionKind::NewFolder,
        );

        assert_eq!(target, "/srv/app/logs/archive");
    }

    #[test]
    fn confirm_file_action_rejects_empty_target() {
        let (mut state, command_rx, _dir) = connected_state();
        state.workspace.files = vec![text_file_entry("/srv/app/settings.toml", "settings.toml")];
        state.workspace.selected_file = Some("/srv/app/settings.toml".into());
        state.modal = Some(Modal::FileAction(PendingFileAction {
            kind: FileActionKind::Rename,
            source: "/srv/app/settings.toml".into(),
            value: "  ".into(),
        }));

        let _ = super::update(&mut state, Message::ConfirmFileAction);

        assert!(command_rx.try_recv().is_err());
        assert!(matches!(state.modal, Some(Modal::FileAction(_))));
        assert_eq!(state.notifications.len(), 1);
        assert_eq!(
            state.notifications[0].level,
            crate::app::state::NotificationLevel::Info
        );
    }

    #[test]
    fn rename_dispatches_command_and_closes_dialog() {
        let (mut state, command_rx, _dir) = connected_state();
        state.workspace.current_directory = "/srv/app".into();
        state.workspace.files = vec![text_file_entry("/srv/app/a.txt", "a.txt")];
        state.workspace.selected_file = Some("/srv/app/a.txt".into());

        let _ = super::update(&mut state, Message::StartFileAction(FileActionKind::Rename));
        let _ = super::update(&mut state, Message::FileActionInputChanged("b.txt".into()));
        let _ = super::update(&mut state, Message::ConfirmFileAction);

        assert!(state.modal.is_none());
        assert_eq!(
            command_rx.try_recv().expect("rename"),
            SessionCommand::Rename {
                source: "/srv/app/a.txt".into(),
                target: "/srv/app/b.txt".into(),
            }
        );
    }

    #[test]
    fn deleting_a_remote_entry_requires_confirmation() {
        let (mut state, command_rx, _dir) = connected_state();
        state.workspace.files = vec![directory_entry("/srv/app/logs", "logs")];
        state.workspace.selected_file = Some("/srv/app/logs".into());

        let _ = super::update(&mut state, Message::DeleteSelectedFile);
        assert!(command_rx.try_recv().is_err());

        let _ = super::update(&mut state, Message::ConfirmModal);
        assert_eq!(
            command_rx.try_recv().expect("delete"),
            SessionCommand::Delete {
                remote_path: "/srv/app/logs".into(),
            }
        );
        assert!(state.workspace.selected_file.is_none());
    }

    #[test]
    fn opens_explorer_context_menu_on_secondary_press() {
        let (mut state, _dir) = test_state();

        let _ = super::update(
            &mut state,
            Message::ExplorerEntrySecondaryPressed("/srv/app".into()),
        );

        assert_eq!(state.workspace.selected_file.as_deref(), Some("/srv/app"));
        assert_eq!(
            state.workspace.explorer_context_for.as_deref(),
            Some("/srv/app")
        );
    }

    #[test]
    fn dismisses_explorer_context_menu_without_clearing_selection() {
        let (mut state, _dir) = test_state();
        state.workspace.selected_file = Some("/srv/app/README.md".into());
        state.workspace.explorer_context_for = Some("/srv/app/README.md".into());

        let _ = super::update(&mut state, Message::DismissExplorerContextMenu);

        assert_eq!(
            state.workspace.selected_file.as_deref(),
            Some("/srv/app/README.md")
        );
        assert!(state.workspace.explorer_context_for.is_none());
    }

    #[test]
    fn explorer_primary_press_closes_context_menu() {
        let (mut state, _dir) = test_state();
        state.workspace.files = vec![text_file_entry("/srv/app/README.md", "README.md")];
        state.workspace.explorer_context_for = Some("/srv/app/README.md".into());

        let _ = super::update(
            &mut state,
            Message::ExplorerEntryPressed("/srv/app/README.md".into()),
        );

        assert!(state.workspace.explorer_context_for.is_none());
        assert_eq!(
            state.workspace.selected_file.as_deref(),
            Some("/srv/app/README.md")
        );
    }

    #[test]
    fn refresh_directory_closes_context_menu() {
        let (mut state, _dir) = test_state();
        state.workspace.explorer_context_for = Some("/srv/app/README.md".into());

        let _ = super::update(&mut state, Message::RefreshDirectory);

        assert!(state.workspace.explorer_context_for.is_none());
    }

    #[test]
    fn navigates_up_from_current_directory() {
        assert_eq!(super::parent_directory("/srv/app/releases"), "/srv/app");
    }

    #[test]
    fn home_relative_cd_does_not_stall_the_explorer() {
        let (mut state, command_rx, _dir) = connected_state();
        state.workspace.current_directory = "/srv".into();

        let _ = super::update(&mut state, Message::NavigateTo("~/logs".into()));
        assert_eq!(
            command_rx.try_recv().expect("refresh"),
            SessionCommand::RefreshDirectory("~/logs".into())
        );

        deliver(
            &mut state,
            SessionEvent::DirectoryLoaded {
                request: "~/logs".into(),
                cwd: "/home/deploy/logs".into(),
                entries: vec![text_file_entry("/home/deploy/logs/app.log", "app.log")],
            },
        );

        assert_eq!(state.workspace.current_directory, "/home/deploy/logs");
        assert!(state.workspace.pending_directory.is_none());
        assert_eq!(state.workspace.files.len(), 1);
    }

    #[test]
    fn stale_directory_listings_are_ignored_while_waiting() {
        let (mut state, _command_rx, _dir) = connected_state();
        state.workspace.current_directory = "/srv".into();
        state.workspace.pending_directory = Some("/var/log".into());

        deliver(
            &mut state,
            SessionEvent::DirectoryLoaded {
                request: "/srv".into(),
                cwd: "/srv".into(),
                entries: vec![text_file_entry("/srv/old.txt", "old.txt")],
            },
        );

        assert!(state.workspace.files.is_empty());
        assert_eq!(
            state.workspace.pending_directory.as_deref(),
            Some("/var/log")
        );
    }

    #[test]
    fn stale_directory_errors_are_ignored() {
        let (mut state, _command_rx, _dir) = connected_state();
        state.workspace.pending_directory = Some("/srv/app".into());

        deliver(
            &mut state,
            SessionEvent::DirectoryOpenFailed {
                request: "/srv/old".into(),
                error: "gone".into(),
            },
        );
        assert!(state.notifications.is_empty());

        deliver(
            &mut state,
            SessionEvent::DirectoryOpenFailed {
                request: "/srv/app".into(),
                error: "denied".into(),
            },
        );
        assert_eq!(state.notifications.len(), 1);
        assert!(state.workspace.pending_directory.is_none());
    }

    #[test]
    fn refreshing_the_same_directory_keeps_folders_expanded() {
        let (mut state, command_rx, _dir) = connected_state();
        state.workspace.current_directory = "/srv/app".into();
        state.workspace.files = vec![directory_entry("/srv/app/src", "src")];
        state
            .workspace
            .expanded_folders
            .insert("/srv/app/src".into());

        deliver(
            &mut state,
            SessionEvent::DirectoryLoaded {
                request: "/srv/app".into(),
                cwd: "/srv/app".into(),
                entries: vec![directory_entry("/srv/app/src", "src")],
            },
        );

        assert!(state.workspace.expanded_folders.contains("/srv/app/src"));
        assert_eq!(
            command_rx.try_recv().expect("reload children"),
            SessionCommand::LoadDirectoryChildren("/srv/app/src".into())
        );
    }

    #[test]
    fn project_link_points_to_repository() {
        assert_eq!(
            super::PROJECT_URL,
            "https://github.com/jaggerjack61/RustSSHClient"
        );
    }

    #[test]
    fn selecting_a_save_lifetime_updates_the_form() {
        let (mut state, _dir) = test_state();

        let _ = super::update(
            &mut state,
            Message::SelectSaveLifetime(crate::models::SaveLifetime::OneWeek),
        );

        assert_eq!(state.login.save_lifetime, SaveLifetime::OneWeek);
    }

    #[test]
    fn key_auth_rejects_missing_key() {
        let (mut state, _dir) = test_state();
        fill_login_form(&mut state);
        state.login.auth_type = crate::models::AuthType::Key;
        state.login.selected_key = Some(uuid::Uuid::new_v4());

        let _ = super::update(&mut state, Message::ConnectPressed);

        assert!(
            state
                .login
                .error
                .as_deref()
                .is_some_and(|error| error.contains("no longer exists"))
        );
    }

    #[test]
    fn opens_unknown_extension_file_in_editor_and_dispatches_session_command() {
        let (mut state, command_rx, _dir) = connected_state();

        state.workspace.files = vec![text_file_entry("/srv/app/Procfile", "Procfile")];
        state.workspace.selected_file = Some("/srv/app/Procfile".into());

        let _ = super::update(&mut state, Message::OpenSelectedFileInEditor);

        assert_eq!(state.workspace.editor_tabs.len(), 1);
        assert_eq!(state.workspace.editor_tabs[0].title, "Procfile");
        assert!(state.workspace.editor_tabs[0].is_loading);
        assert_eq!(
            state.workspace.active_tab,
            WorkspaceTab::Editor("/srv/app/Procfile".into())
        );
        assert_eq!(
            command_rx.try_recv().expect("editor read command"),
            SessionCommand::ReadFile {
                remote_path: "/srv/app/Procfile".into(),
            }
        );
    }

    #[test]
    fn does_not_open_directories_in_editor() {
        let (mut state, command_rx, _dir) = connected_state();

        state.workspace.files = vec![directory_entry("/srv/app/config", "config")];
        state.workspace.selected_file = Some("/srv/app/config".into());

        let _ = super::update(&mut state, Message::OpenSelectedFileInEditor);

        assert!(state.workspace.editor_tabs.is_empty());
        assert!(command_rx.try_recv().is_err());
    }

    #[test]
    fn double_clicking_a_file_opens_it_in_editor() {
        let (mut state, command_rx, _dir) = connected_state();

        state.workspace.files = vec![text_file_entry("/srv/app/README.md", "README.md")];

        let _ = super::update(
            &mut state,
            Message::ExplorerEntryDoubleClicked("/srv/app/README.md".into()),
        );

        assert_eq!(
            state.workspace.selected_file.as_deref(),
            Some("/srv/app/README.md")
        );
        assert_eq!(state.workspace.editor_tabs.len(), 1);
        assert_eq!(
            command_rx.try_recv().expect("editor read command"),
            SessionCommand::ReadFile {
                remote_path: "/srv/app/README.md".into(),
            }
        );
    }

    #[test]
    fn single_clicking_directory_requests_lazy_child_load() {
        let (mut state, command_rx, _dir) = connected_state();
        state.workspace.files = vec![directory_entry("/srv/app/src", "src")];

        let _ = super::update(
            &mut state,
            Message::ExplorerEntryPressed("/srv/app/src".into()),
        );

        assert!(state.workspace.expanded_folders.contains("/srv/app/src"));
        assert_eq!(
            command_rx.try_recv().expect("child load command"),
            SessionCommand::LoadDirectoryChildren("/srv/app/src".into())
        );

        let _ = super::update(
            &mut state,
            Message::ExplorerEntryPressed("/srv/app/src".into()),
        );
        assert!(!state.workspace.expanded_folders.contains("/srv/app/src"));
    }

    #[test]
    fn directory_child_load_merges_entries_without_changing_current_directory() {
        let (mut state, _command_rx, _dir) = connected_state();
        state.workspace.current_directory = "/srv/app".into();
        state.workspace.files = vec![
            directory_entry("/srv/app/src", "src"),
            text_file_entry("/srv/app/README.md", "README.md"),
        ];
        state
            .workspace
            .expanded_folders
            .insert("/srv/app/src".into());

        deliver(
            &mut state,
            SessionEvent::DirectoryChildrenLoaded {
                directory: "/srv/app/src".into(),
                entries: vec![text_file_entry("/srv/app/src/main.rs", "main.rs")],
            },
        );

        assert_eq!(state.workspace.current_directory, "/srv/app");
        assert!(
            state
                .workspace
                .files
                .iter()
                .any(|entry| entry.path == "/srv/app/src/main.rs")
        );
    }

    #[test]
    fn loads_editor_contents_from_session_event() {
        let (mut state, _command_rx, _dir) = connected_state();

        state.workspace.open_editor_tab("/srv/app/src/main.rs");
        deliver(
            &mut state,
            SessionEvent::FileOpened {
                path: "/srv/app/src/main.rs".into(),
                contents: "fn main() {}\n".into(),
            },
        );

        let editor = state.active_editor().expect("active editor");
        assert_eq!(editor.language, EditorLanguage::Rust);
        assert_eq!(editor.current_text(), "fn main() {}\n");
        assert!(!editor.is_loading);
    }

    #[test]
    fn late_file_contents_do_not_steal_focus() {
        let (mut state, _command_rx, _dir) = connected_state();
        state.workspace.open_editor_tab("/srv/app/README.md");
        state.workspace.active_tab = WorkspaceTab::Terminal;

        deliver(
            &mut state,
            SessionEvent::FileOpened {
                path: "/srv/app/README.md".into(),
                contents: "# hi".into(),
            },
        );

        assert_eq!(state.workspace.active_tab, WorkspaceTab::Terminal);
    }

    #[test]
    fn stores_editor_load_failures_in_tab_state() {
        let (mut state, _command_rx, _dir) = connected_state();

        state.workspace.open_editor_tab("/srv/app/README.md");
        deliver(
            &mut state,
            SessionEvent::FileOpenFailed {
                path: "/srv/app/README.md".into(),
                error: "File is not valid UTF-8 text.".into(),
            },
        );

        let editor = state.active_editor().expect("active editor");
        assert_eq!(
            editor.load_error.as_deref(),
            Some("File is not valid UTF-8 text.")
        );
        assert!(!editor.is_loading);
    }

    #[test]
    fn closes_clean_editor_tab_and_returns_to_terminal() {
        let (mut state, _dir) = test_state();
        state.workspace.open_editor_tab("/srv/app/README.md");
        state
            .workspace
            .apply_editor_content("/srv/app/README.md", "# RustSSH\n".into());

        let _ = super::update(
            &mut state,
            Message::CloseEditorTab("/srv/app/README.md".into()),
        );

        assert!(state.workspace.editor_tabs.is_empty());
        assert_eq!(state.workspace.active_tab, WorkspaceTab::Terminal);
    }

    #[test]
    fn closing_a_dirty_tab_asks_first() {
        let (mut state, _dir) = test_state();
        state.workspace.open_editor_tab("/srv/app/README.md");
        state
            .workspace
            .apply_editor_content("/srv/app/README.md", "hello".into());
        insert_text(&mut state, "/srv/app/README.md", '!');

        let _ = super::update(
            &mut state,
            Message::CloseEditorTab("/srv/app/README.md".into()),
        );

        assert_eq!(state.workspace.editor_tabs.len(), 1);
        assert!(matches!(
            state.modal,
            Some(Modal::Confirm(Confirmation::CloseTab { .. }))
        ));
    }

    #[test]
    fn editor_actions_mark_document_dirty() {
        let (mut state, _dir) = test_state();
        state.workspace.open_editor_tab("/srv/app/README.md");
        state
            .workspace
            .apply_editor_content("/srv/app/README.md", "hello".into());

        insert_text(&mut state, "/srv/app/README.md", '!');

        let editor = state.active_editor().expect("active editor");
        assert!(editor.is_dirty);
        assert_eq!(editor.current_text(), "hello!");
    }

    #[test]
    fn save_active_editor_dispatches_write_command() {
        let (mut state, command_rx, _dir) = connected_state();

        state.workspace.open_editor_tab("/srv/app/README.md");
        state
            .workspace
            .apply_editor_content("/srv/app/README.md", "hello".into());
        insert_text(&mut state, "/srv/app/README.md", '!');

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
        let (mut state, _command_rx, _dir) = connected_state();

        state.workspace.open_editor_tab("/srv/app/README.md");
        state
            .workspace
            .apply_editor_content("/srv/app/README.md", "hello".into());
        insert_text(&mut state, "/srv/app/README.md", '!');
        let _ = super::update(&mut state, Message::SaveActiveEditor);

        deliver(
            &mut state,
            SessionEvent::FileSaved {
                path: "/srv/app/README.md".into(),
            },
        );

        let editor = state.active_editor().expect("active editor");
        assert!(!editor.is_dirty);
        assert!(!editor.is_saving);
        assert_eq!(editor.saved_content, "hello!");
    }

    #[test]
    fn file_save_failure_preserves_dirty_state() {
        let (mut state, _command_rx, _dir) = connected_state();

        state.workspace.open_editor_tab("/srv/app/README.md");
        state
            .workspace
            .apply_editor_content("/srv/app/README.md", "hello".into());
        insert_text(&mut state, "/srv/app/README.md", '!');
        let _ = super::update(&mut state, Message::SaveActiveEditor);

        deliver(
            &mut state,
            SessionEvent::FileSaveFailed {
                path: "/srv/app/README.md".into(),
                error: "permission denied".into(),
            },
        );

        let editor = state.active_editor().expect("active editor");
        assert!(editor.is_dirty);
        assert!(!editor.is_saving);
    }

    #[test]
    fn reopening_a_loaded_file_keeps_unsaved_edits() {
        let (mut state, command_rx, _dir) = connected_state();

        state.workspace.files = vec![text_file_entry("/srv/app/README.md", "README.md")];
        state.workspace.selected_file = Some("/srv/app/README.md".into());

        let _ = super::update(&mut state, Message::OpenSelectedFileInEditor);
        state
            .workspace
            .apply_editor_content("/srv/app/README.md", "# RustSSH\n".into());
        insert_text(&mut state, "/srv/app/README.md", '!');
        let _ = command_rx.try_recv();
        state.workspace.active_tab = WorkspaceTab::Terminal;

        let _ = super::update(&mut state, Message::OpenSelectedFileInEditor);

        assert_eq!(state.workspace.editor_tabs.len(), 1);
        assert!(!state.workspace.editor_tabs[0].is_loading);
        assert!(state.workspace.editor_tabs[0].is_dirty);
        assert!(command_rx.try_recv().is_err());
        assert_eq!(
            state.workspace.active_tab,
            WorkspaceTab::Editor("/srv/app/README.md".into())
        );
    }

    #[test]
    fn completed_upload_refreshes_the_explorer() {
        let (mut state, command_rx, _dir) = connected_state();
        state.workspace.current_directory = "/srv/app".into();
        let mut transfer = TransferProgress::queued("a.txt", TransferDirection::Upload, 1);
        deliver(&mut state, SessionEvent::Transfer(transfer.clone()));

        transfer.status = TransferStatus::Completed;
        deliver(&mut state, SessionEvent::Transfer(transfer));

        assert_eq!(
            command_rx.try_recv().expect("refresh"),
            SessionCommand::RefreshDirectory("/srv/app".into())
        );
    }

    #[test]
    fn notifications_are_dismissed_by_id() {
        let (mut state, _dir) = test_state();
        state.notification(crate::app::state::NotificationLevel::Info, "first");
        state.notification(crate::app::state::NotificationLevel::Info, "second");
        let first = state.notifications[0].id;

        let _ = super::update(&mut state, Message::DismissNotification(first));

        assert_eq!(state.notifications.len(), 1);
        assert_eq!(state.notifications[0].message, "second");
        let _ = super::update(&mut state, Message::Tick(Instant::now()));
    }

    fn insert_text(state: &mut AppState, path: &str, character: char) {
        let _ = super::update(
            state,
            Message::EditorAction(
                path.into(),
                text_editor::Action::Move(text_editor::Motion::DocumentEnd),
            ),
        );
        let _ = super::update(
            state,
            Message::EditorAction(
                path.into(),
                text_editor::Action::Edit(text_editor::Edit::Insert(character)),
            ),
        );
    }

    fn text_file_entry(path: &str, name: &str) -> FileEntry {
        FileEntry {
            name: name.into(),
            path: path.into(),
            kind: FileKind::File,
            is_symlink: false,
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
            is_symlink: false,
            size: 0,
            permissions: "drwxr-xr-x".into(),
            owner: Some("root".into()),
            modified: None,
        }
    }
}
