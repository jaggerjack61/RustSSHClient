use std::path::PathBuf;
use std::time::Instant;

use iced::Event;
use iced::widget::text_editor;
use uuid::Uuid;

use crate::models::{HostSort, SaveLifetime, SshKeyRecord};
use crate::ssh::session::SessionHandle;
use crate::storage::StorageSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileActionKind {
    Rename,
    Copy,
    Move,
}

#[derive(Debug, Clone)]
pub enum Message {
    StorageLoaded(Result<StorageSnapshot, String>),
    LoginLabelChanged(String),
    LoginHostChanged(String),
    LoginPortChanged(String),
    LoginUsernameChanged(String),
    LoginPasswordChanged(String),
    ToggleSaveConnection(bool),
    UsePasswordAuthentication,
    UseKeyAuthentication,
    ToggleKeyManager,
    OpenAdvancedSettings,
    CloseAdvancedSettings,
    SelectSaveLifetime(SaveLifetime),
    OpenProjectLink,
    ConnectPressed,
    SessionSpawned(Result<SessionHandle, String>),
    HostCardPressed(Uuid),
    DeleteHost(Uuid),
    EditHost(Uuid),
    HostSortChanged(HostSort),
    SelectKey(Uuid),
    DeleteKey(Uuid),
    ImportKeyPressed,
    KeyImported(Result<Option<SshKeyRecord>, String>),
    Tick(Instant),
    RuntimeEvent(Event),
    ClearTerminal,
    CopyTerminalOutput,
    PasteTerminalInput,
    DisconnectPressed,
    RefreshDirectory,
    NavigateUpDirectory,
    DismissExplorerContextMenu,
    ExplorerEntryPressed(String),
    ExplorerEntrySecondaryPressed(String),
    OpenSelectedFileInEditor,
    EditorAction(String, text_editor::Action),
    SaveActiveEditor,
    ActivateTerminalTab,
    ActivateEditorTab(String),
    CloseEditorTab(String),
    UploadRequested,
    FilesSelected(Option<Vec<PathBuf>>),
    DownloadRequested,
    DeleteSelectedFile,
    StartFileAction(FileActionKind),
    FileActionInputChanged(String),
    ConfirmFileAction,
    CancelFileAction,
    DismissNotification(usize),
}
