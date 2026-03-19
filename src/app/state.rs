use std::collections::HashSet;
use std::fs::OpenOptions;
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
use crate::ssh::session::SessionHandle;
use crate::ssh::terminal::TerminalBuffer;
use crate::storage::{StorageFacade, StorageSnapshot};

const MAX_NOTIFICATIONS: usize = 8;
const NOTIFICATION_TTL: Duration = Duration::from_secs(12);

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
    pub level: NotificationLevel,
    pub message: String,
    pub created_at: Instant,
}

#[derive(Debug, Clone)]
pub struct LoginFormState {
    pub label: String,
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub save_connection: bool,
    pub save_lifetime: SaveLifetime,
    pub auth_type: AuthType,
    pub selected_key: Option<Uuid>,
    pub connecting: bool,
    pub editing_host_id: Option<Uuid>,
}

impl Default for LoginFormState {
    fn default() -> Self {
        Self {
            label: String::new(),
            host: String::new(),
            port: "22".into(),
            username: String::new(),
            password: String::new(),
            save_connection: true,
            save_lifetime: SaveLifetime::Forever,
            auth_type: AuthType::Password,
            selected_key: None,
            connecting: false,
            editing_host_id: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingFileAction {
    pub kind: FileActionKind,
    pub value: String,
}

#[derive(Debug)]
pub struct WorkspaceState {
    pub session: Option<SessionHandle>,
    pub status: String,
    pub current_directory: String,
    pub pending_directory: Option<String>,
    pub connected_peer: String,
    pub latency_ms: Option<u128>,
    pub terminal: TerminalBuffer,
    pub files: Vec<FileEntry>,
    pub selected_file: Option<String>,
    pub explorer_context_for: Option<String>,
    pub editor_tabs: Vec<EditorDocument>,
    pub active_tab: WorkspaceTab,
    pub transfers: Vec<TransferProgress>,
    pub pending_file_action: Option<PendingFileAction>,
    pub expanded_folders: HashSet<String>,
    pub loaded_folders: HashSet<String>,
    pub loading_folders: HashSet<String>,
    pub show_properties: bool,
    pub window_size: Option<(f32, f32)>,
    pub terminal_cursor_visible: bool,
    pub last_terminal_cursor_toggle: Instant,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            session: None,
            status: "Disconnected".into(),
            current_directory: "/".into(),
            pending_directory: None,
            connected_peer: String::new(),
            latency_ms: None,
            terminal: TerminalBuffer::default(),
            files: Vec::new(),
            selected_file: None,
            explorer_context_for: None,
            editor_tabs: Vec::new(),
            active_tab: WorkspaceTab::Terminal,
            transfers: Vec::new(),
            pending_file_action: None,
            expanded_folders: HashSet::new(),
            loaded_folders: HashSet::new(),
            loading_folders: HashSet::new(),
            show_properties: false,
            window_size: None,
            terminal_cursor_visible: true,
            last_terminal_cursor_toggle: Instant::now(),
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
    pub workspace: WorkspaceState,
    pub key_manager_open: bool,
    pub advanced_settings_open: bool,
    pub notifications: Vec<Notification>,
    pub last_host_click: Option<(Uuid, Instant)>,
}

impl AppState {
    pub fn boot() -> (Self, Task<Message>) {
        let storage = StorageFacade::new();
        let state = Self {
            route: Route::Login,
            storage: storage.clone(),
            login: LoginFormState::default(),
            hosts: Vec::new(),
            keys: Vec::new(),
            host_sort: HostSort::Label,
            workspace: WorkspaceState::default(),
            key_manager_open: false,
            advanced_settings_open: false,
            notifications: Vec::new(),
            last_host_click: None,
        };

        let task = Task::perform(
            async move { storage.load_snapshot().map_err(|error| error.to_string()) },
            Message::StorageLoaded,
        );

        (state, task)
    }

    pub fn snapshot(&self) -> StorageSnapshot {
        StorageSnapshot {
            hosts: self.hosts.clone(),
            keys: self.keys.clone(),
        }
    }

    pub fn notification(&mut self, level: NotificationLevel, message: impl Into<String>) {
        let message = message.into();
        self.notifications.push(Notification {
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
        self.notifications
            .retain(|item| item.created_at.elapsed() < NOTIFICATION_TTL);
    }

    fn append_notification_log(
        &self,
        level: NotificationLevel,
        message: &str,
    ) -> std::io::Result<()> {
        let log_path = self.storage.root().join("notifications.log");
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
        self.workspace
            .files
            .iter()
            .find(|entry| entry.path == selected)
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
        self.login.auth_type = host.auth_type;
        self.login.selected_key = host.key_reference;
        self.login.save_connection = true;
        self.login.save_lifetime = host.save_lifetime;
        self.login.editing_host_id = Some(host.id);
    }

    pub fn prepare_login_request(&self) -> Result<LoginRequest, String> {
        let port = self
            .login
            .port
            .trim()
            .parse::<u16>()
            .map_err(|_| "Port must be a valid number.".to_string())?;

        let request = LoginRequest {
            label: if self.login.label.trim().is_empty() {
                None
            } else {
                Some(self.login.label.clone())
            },
            host: self.login.host.clone(),
            port,
            username: self.login.username.clone(),
            password: if self.login.auth_type == AuthType::Password {
                Some(self.login.password.clone())
            } else {
                None
            },
            auth_type: self.login.auth_type,
            key_reference: self.login.selected_key,
            save_host: self.login.save_connection,
            save_lifetime: self.login.save_lifetime,
        };

        request.validate().map_err(|error| error.to_string())?;
        Ok(request)
    }

    pub fn sorted_hosts(&self) -> Vec<HostRecord> {
        let mut hosts = self.hosts.clone();
        match self.host_sort {
            HostSort::Label => hosts
                .sort_by(|left, right| left.label.to_lowercase().cmp(&right.label.to_lowercase())),
            HostSort::Host => hosts
                .sort_by(|left, right| left.host.to_lowercase().cmp(&right.host.to_lowercase())),
            HostSort::Recent => hosts.sort_by(|left, right| right.updated_at.cmp(&left.updated_at)),
        }
        hosts
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.route, Route::Workspace) && self.workspace.session.is_some()
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

        self.editor_tabs.push(EditorDocument::new_loading(path.clone()));
        self.active_tab = WorkspaceTab::Editor(path);
    }

    pub fn apply_editor_content(&mut self, path: &str, content: String) {
        if let Some(tab) = self.editor_tabs.iter_mut().find(|tab| tab.path == path) {
            tab.apply_content(content);
        } else {
            let mut tab = EditorDocument::new_loading(path.to_string());
            tab.apply_content(content);
            self.editor_tabs.push(tab);
        }

        self.active_tab = WorkspaceTab::Editor(path.to_string());
    }

    pub fn fail_editor_load(&mut self, path: &str, error: String) {
        if let Some(tab) = self.editor_tabs.iter_mut().find(|tab| tab.path == path) {
            tab.set_error(error);
        } else {
            let mut tab = EditorDocument::new_loading(path.to_string());
            tab.set_error(error);
            self.editor_tabs.push(tab);
        }

        self.active_tab = WorkspaceTab::Editor(path.to_string());
    }

    pub fn apply_editor_action(
        &mut self,
        path: &str,
        action: iced::widget::text_editor::Action,
    ) {
        if let Some(tab) = self.editor_tabs.iter_mut().find(|tab| tab.path == path) {
            tab.apply_action(action);
            self.active_tab = WorkspaceTab::Editor(path.to_string());
        }
    }

    pub fn mark_editor_saving(&mut self, path: &str) {
        if let Some(tab) = self.editor_tabs.iter_mut().find(|tab| tab.path == path) {
            tab.mark_saving();
        }
    }

    pub fn mark_editor_saved(&mut self, path: &str) {
        if let Some(tab) = self.editor_tabs.iter_mut().find(|tab| tab.path == path) {
            tab.mark_saved();
            self.active_tab = WorkspaceTab::Editor(path.to_string());
        }
    }

    pub fn mark_editor_save_failed(&mut self, path: &str) {
        if let Some(tab) = self.editor_tabs.iter_mut().find(|tab| tab.path == path) {
            tab.mark_save_failed();
            self.active_tab = WorkspaceTab::Editor(path.to_string());
        }
    }

    pub fn editor_text(&self, path: &str) -> Option<String> {
        self.editor_tabs
            .iter()
            .find(|tab| tab.path == path)
            .map(EditorDocument::current_text)
    }

    pub fn close_editor_tab(&mut self, path: &str) {
        self.editor_tabs.retain(|tab| tab.path != path);

        if matches!(&self.active_tab, WorkspaceTab::Editor(active) if active == path) {
            self.active_tab = self
                .editor_tabs
                .last()
                .map(|tab| WorkspaceTab::Editor(tab.path.clone()))
                .unwrap_or(WorkspaceTab::Terminal);
        }
    }

    pub fn reset_editor_tabs(&mut self) {
        self.editor_tabs.clear();
        self.active_tab = WorkspaceTab::Terminal;
    }
}
