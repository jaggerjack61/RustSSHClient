use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{Duration, Instant};

use chrono::Utc;
use iced::Task;
use uuid::Uuid;

use crate::app::messages::{FileActionKind, Message};
use crate::models::{
    AuthType, EditorDocument, FileEntry, HostRecord, HostSort, LoginRequest, SaveLifetime,
    SshKeyRecord, TransferProgress, WorkspaceTab,
};
use crate::ssh::client::HostKeyInfo;
use crate::ssh::session::SessionHandle;
use crate::ssh::terminal::TerminalBuffer;
use crate::storage::{StorageFacade, StorageSnapshot};

const MAX_NOTIFICATIONS: usize = 5;
const NOTIFICATION_TTL: Duration = Duration::from_secs(6);
const ERROR_NOTIFICATION_TTL: Duration = Duration::from_secs(12);
const NOTIFICATION_LOG_LIMIT_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Login,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u64,
    pub level: NotificationLevel,
    pub message: String,
    pub created_at: Instant,
}

impl Notification {
    fn is_expired(&self) -> bool {
        let ttl = match self.level {
            NotificationLevel::Error => ERROR_NOTIFICATION_TTL,
            _ => NOTIFICATION_TTL,
        };
        self.created_at.elapsed() >= ttl
    }
}

#[derive(Debug, Clone)]
pub struct LoginFormState {
    pub label: String,
    pub host: String,
    pub port: String,
    pub username: String,
    /// Password, or the optional key passphrase for key authentication.
    pub password: String,
    pub password_visible: bool,
    pub save_connection: bool,
    pub save_lifetime: SaveLifetime,
    pub auth_type: AuthType,
    pub selected_key: Option<Uuid>,
    pub connecting: bool,
    pub editing_host_id: Option<Uuid>,
    pub error: Option<String>,
}

impl Default for LoginFormState {
    fn default() -> Self {
        Self {
            label: String::new(),
            host: String::new(),
            port: "22".into(),
            username: String::new(),
            password: String::new(),
            password_visible: false,
            save_connection: true,
            save_lifetime: SaveLifetime::Forever,
            auth_type: AuthType::Password,
            selected_key: None,
            connecting: false,
            editing_host_id: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFileAction {
    pub kind: FileActionKind,
    /// The entry being acted on (the parent directory for `NewFolder`).
    pub source: String,
    pub value: String,
}

/// Something the user must confirm before it happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirmation {
    DeleteRemote {
        path: String,
        is_directory: bool,
    },
    DeleteHost {
        id: Uuid,
        label: String,
    },
    DeleteKey {
        id: Uuid,
        label: String,
        used_by: usize,
    },
    CloseTab {
        path: String,
    },
    Disconnect {
        unsaved: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Confirm(Confirmation),
    HostKey(HostKeyInfo),
    FileAction(PendingFileAction),
    Properties(String),
}

#[derive(Debug)]
pub struct WorkspaceState {
    pub session: Option<SessionHandle>,
    pub current_directory: String,
    pub pending_directory: Option<String>,
    pub connected_peer: String,
    pub latency_ms: Option<u128>,
    pub terminal: TerminalBuffer,
    pub files: Vec<FileEntry>,
    pub selected_file: Option<String>,
    pub explorer_context_for: Option<String>,
    pub explorer_scroll_offset: f32,
    pub editor_tabs: Vec<EditorDocument>,
    pub active_tab: WorkspaceTab,
    pub transfers: Vec<TransferProgress>,
    pub transfers_expanded: bool,
    pub expanded_folders: HashSet<String>,
    pub loaded_folders: HashSet<String>,
    pub loading_folders: HashSet<String>,
    pub terminal_cursor_visible: bool,
    pub last_terminal_activity: Instant,
    /// Fractional scroll carried between trackpad events.
    pub terminal_scroll_remainder: f32,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            session: None,
            current_directory: "/".into(),
            pending_directory: None,
            connected_peer: String::new(),
            latency_ms: None,
            terminal: TerminalBuffer::default(),
            files: Vec::new(),
            selected_file: None,
            explorer_context_for: None,
            explorer_scroll_offset: 0.0,
            editor_tabs: Vec::new(),
            active_tab: WorkspaceTab::Terminal,
            transfers: Vec::new(),
            transfers_expanded: true,
            expanded_folders: HashSet::new(),
            loaded_folders: HashSet::new(),
            loading_folders: HashSet::new(),
            terminal_cursor_visible: true,
            last_terminal_activity: Instant::now(),
            terminal_scroll_remainder: 0.0,
        }
    }
}

pub struct AppState {
    pub route: Route,
    pub storage: StorageFacade,
    pub login: LoginFormState,
    pub hosts: Vec<HostRecord>,
    pub keys: Vec<SshKeyRecord>,
    pub host_sort: HostSort,
    pub host_filter: String,
    pub workspace: WorkspaceState,
    pub modal: Option<Modal>,
    pub notifications: Vec<Notification>,
    next_notification_id: u64,
}

impl AppState {
    pub fn boot() -> (Self, Task<Message>) {
        let storage = StorageFacade::new();
        let state = Self::with_storage(storage.clone());

        let task = Task::perform(
            async move { storage.load_snapshot().map_err(|error| error.to_string()) },
            Message::StorageLoaded,
        );

        (state, task)
    }

    pub fn with_storage(storage: StorageFacade) -> Self {
        Self {
            route: Route::Login,
            storage,
            login: LoginFormState::default(),
            hosts: Vec::new(),
            keys: Vec::new(),
            host_sort: HostSort::Label,
            host_filter: String::new(),
            workspace: WorkspaceState::default(),
            modal: None,
            notifications: Vec::new(),
            next_notification_id: 0,
        }
    }

    pub fn snapshot(&self) -> StorageSnapshot {
        StorageSnapshot {
            hosts: self.hosts.clone(),
            keys: self.keys.clone(),
        }
    }

    pub fn notification(&mut self, level: NotificationLevel, message: impl Into<String>) {
        let message = message.into();
        self.next_notification_id += 1;
        self.notifications.push(Notification {
            id: self.next_notification_id,
            level,
            message: message.clone(),
            created_at: Instant::now(),
        });
        if self.notifications.len() > MAX_NOTIFICATIONS {
            self.notifications.remove(0);
        }

        let _ = self.append_notification_log(level, &message);
    }

    pub fn prune_notifications(&mut self) {
        self.notifications.retain(|item| !item.is_expired());
    }

    pub fn dismiss_notification(&mut self, id: u64) {
        self.notifications.retain(|item| item.id != id);
    }

    fn append_notification_log(
        &self,
        level: NotificationLevel,
        message: &str,
    ) -> std::io::Result<()> {
        let root = self.storage.root();
        fs::create_dir_all(root)?;
        let log_path = root.join("notifications.log");

        // Keep one rotated generation so the log can't grow without bound.
        if fs::metadata(&log_path).is_ok_and(|meta| meta.len() > NOTIFICATION_LOG_LIMIT_BYTES) {
            let _ = fs::rename(&log_path, root.join("notifications.log.1"));
        }

        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        writeln!(log, "{} [{level:?}] {message}", Utc::now().to_rfc3339())?;
        Ok(())
    }

    pub fn selected_key(&self) -> Option<&SshKeyRecord> {
        let id = self.login.selected_key?;
        self.keys.iter().find(|key| key.id == id)
    }

    pub fn selected_file(&self) -> Option<&FileEntry> {
        let selected = self.workspace.selected_file.as_deref()?;
        self.file(selected)
    }

    pub fn file(&self, path: &str) -> Option<&FileEntry> {
        self.workspace.files.iter().find(|entry| entry.path == path)
    }

    pub fn active_editor(&self) -> Option<&EditorDocument> {
        let WorkspaceTab::Editor(path) = &self.workspace.active_tab else {
            return None;
        };

        self.workspace
            .editor_tabs
            .iter()
            .find(|editor| editor.path == *path)
    }

    pub fn active_editor_mut(&mut self) -> Option<&mut EditorDocument> {
        let WorkspaceTab::Editor(path) = &self.workspace.active_tab else {
            return None;
        };

        self.workspace
            .editor_tabs
            .iter_mut()
            .find(|editor| editor.path == *path)
    }

    pub fn apply_host_to_form(&mut self, host: &HostRecord) {
        self.login.label = host.label.clone();
        self.login.host = host.host.clone();
        self.login.port = host.port.to_string();
        self.login.username = host.username.clone();
        self.login.password = host.password.clone().unwrap_or_default();
        self.login.password_visible = false;
        self.login.auth_type = host.auth_type;
        self.login.selected_key = host.key_reference;
        self.login.save_connection = true;
        self.login.save_lifetime = host.save_lifetime;
        self.login.editing_host_id = Some(host.id);
        self.login.error = None;
    }

    pub fn prepare_login_request(&self) -> Result<LoginRequest, String> {
        let port = self
            .login
            .port
            .trim()
            .parse::<u16>()
            .map_err(|_| "Port must be a number between 1 and 65535.".to_string())?;

        if self.login.auth_type == AuthType::Key && self.selected_key().is_none() {
            return Err(if self.login.selected_key.is_some() {
                "The selected SSH key no longer exists. Choose or import another key.".into()
            } else {
                "Select an SSH key for key-based authentication.".into()
            });
        }

        let secret = (!self.login.password.is_empty()).then(|| self.login.password.clone());
        let request = LoginRequest {
            label: if self.login.label.trim().is_empty() {
                None
            } else {
                Some(self.login.label.clone())
            },
            host: self.login.host.trim().to_string(),
            port,
            username: self.login.username.trim().to_string(),
            password: secret,
            auth_type: self.login.auth_type,
            key_reference: self.login.selected_key,
            save_host: self.login.save_connection,
            save_lifetime: self.login.save_lifetime,
        };

        request.validate().map_err(|error| match error {
            crate::error::AppError::Validation(message) => message,
            other => other.to_string(),
        })?;
        Ok(request)
    }

    /// Saved hosts matching the sidebar filter, in the chosen order.
    pub fn visible_hosts(&self) -> Vec<&HostRecord> {
        let filter = self.host_filter.trim().to_lowercase();
        let mut hosts = self
            .hosts
            .iter()
            .filter(|host| {
                filter.is_empty()
                    || host.label.to_lowercase().contains(&filter)
                    || host.host.to_lowercase().contains(&filter)
                    || host.username.to_lowercase().contains(&filter)
            })
            .collect::<Vec<_>>();

        match self.host_sort {
            HostSort::Label => hosts.sort_by_cached_key(|host| host.label.to_lowercase()),
            HostSort::Host => hosts.sort_by_cached_key(|host| host.host.to_lowercase()),
            HostSort::Recent => hosts.sort_by_key(|host| std::cmp::Reverse(host.updated_at)),
        }
        hosts
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.route, Route::Workspace) && self.workspace.session.is_some()
    }

    pub fn unsaved_editor_count(&self) -> usize {
        self.workspace
            .editor_tabs
            .iter()
            .filter(|tab| tab.is_dirty)
            .count()
    }
}

impl WorkspaceState {
    pub fn open_editor_tab(&mut self, path: impl Into<String>) {
        let path = path.into();

        if let Some(tab) = self.editor_tabs.iter_mut().find(|tab| tab.path == path) {
            tab.is_loading = true;
            tab.load_error = None;
            self.active_tab = WorkspaceTab::Editor(path);
            return;
        }

        self.editor_tabs
            .push(EditorDocument::new_loading(path.clone()));
        self.active_tab = WorkspaceTab::Editor(path);
    }

    pub fn has_editor_tab(&self, path: &str) -> bool {
        self.editor_tabs.iter().any(|tab| tab.path == path)
    }

    fn editor_tab_mut(&mut self, path: &str) -> Option<&mut EditorDocument> {
        self.editor_tabs.iter_mut().find(|tab| tab.path == path)
    }

    pub fn apply_editor_content(&mut self, path: &str, content: String) {
        // Ignore late results for tabs the user already closed.
        if let Some(tab) = self.editor_tab_mut(path) {
            tab.apply_content(content);
        }
    }

    pub fn fail_editor_load(&mut self, path: &str, error: String) {
        if let Some(tab) = self.editor_tab_mut(path) {
            tab.set_error(error);
        }
    }

    pub fn apply_editor_action(&mut self, path: &str, action: iced::widget::text_editor::Action) {
        if let Some(tab) = self.editor_tab_mut(path) {
            tab.apply_action(action);
        }
    }

    pub fn mark_editor_saving(&mut self, path: &str, contents: String) {
        if let Some(tab) = self.editor_tab_mut(path) {
            tab.mark_saving(contents);
        }
    }

    pub fn mark_editor_saved(&mut self, path: &str) {
        if let Some(tab) = self.editor_tab_mut(path) {
            tab.mark_saved();
        }
    }

    pub fn mark_editor_save_failed(&mut self, path: &str) {
        if let Some(tab) = self.editor_tab_mut(path) {
            tab.mark_save_failed();
        }
    }

    pub fn editor_text(&self, path: &str) -> Option<String> {
        self.editor_tabs
            .iter()
            .find(|tab| tab.path == path)
            .map(EditorDocument::current_text)
    }

    pub fn close_editor_tab(&mut self, path: &str) {
        let closed_index = self.editor_tabs.iter().position(|tab| tab.path == path);
        self.editor_tabs.retain(|tab| tab.path != path);

        if matches!(&self.active_tab, WorkspaceTab::Editor(active) if active == path) {
            // Activate the neighbour of the closed tab, like most editors.
            let next = closed_index
                .and_then(|index| {
                    self.editor_tabs
                        .get(index)
                        .or_else(|| index.checked_sub(1).and_then(|i| self.editor_tabs.get(i)))
                })
                .map(|tab| WorkspaceTab::Editor(tab.path.clone()));
            self.active_tab = next.unwrap_or(WorkspaceTab::Terminal);
        }
    }

    pub fn reset_editor_tabs(&mut self) {
        self.editor_tabs.clear();
        self.active_tab = WorkspaceTab::Terminal;
    }
}
