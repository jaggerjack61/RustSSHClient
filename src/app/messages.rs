use std::path::PathBuf;
use std::time::Instant;

use iced::widget::text_editor;
use iced::{Event, Size, mouse};
use uuid::Uuid;

use crate::models::{HostSort, SaveLifetime, SshKeyRecord};
use crate::ssh::session::{SessionEvent, SessionHandle};
use crate::storage::StorageSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileActionKind {
    Rename,
    Copy,
    Move,
    NewFolder,
}

#[derive(Debug, Clone)]
pub enum Message {
    StorageLoaded(Result<StorageSnapshot, String>),

    // Connection form
    LoginLabelChanged(String),
    LoginHostChanged(String),
    LoginPortChanged(String),
    LoginUsernameChanged(String),
    LoginPasswordChanged(String),
    TogglePasswordVisibility,
    ToggleSaveConnection(bool),
    UsePasswordAuthentication,
    UseKeyAuthentication,
    SelectSaveLifetime(SaveLifetime),
    NewConnection,
    ConnectPressed,
    SessionSpawned(Result<SessionHandle, String>),
    CancelConnect,
    OpenProjectLink,
    OpenIssuesLink,

    // Saved hosts and keys
    HostFilterChanged(String),
    HostSortChanged(HostSort),
    HostSelected(Uuid),
    HostActivated(Uuid),
    DeleteHost(Uuid),
    SelectKey(Uuid),
    DeleteKey(Uuid),
    ImportKeyPressed,
    KeyImported(Result<Option<SshKeyRecord>, String>),

    // Session plumbing
    Session { id: u64, event: SessionEvent },
    HostKeyDecision(bool),
    Tick(Instant),
    RuntimeEvent(Event),

    // Terminal
    TerminalViewportResized(Size),
    TerminalScrolled(mouse::ScrollDelta),
    TerminalScrollToBottom,
    ClearTerminal,
    CopyTerminalOutput,
    PasteTerminalInput,
    DisconnectPressed,

    // Explorer
    RefreshDirectory,
    NavigateUpDirectory,
    NavigateHome,
    NavigateTo(String),
    ExplorerEntryPressed(String),
    ExplorerEntryDoubleClicked(String),
    ExplorerEntrySecondaryPressed(String),
    ExplorerScrolled(f32),
    DismissExplorerContextMenu,
    ShowProperties,
    OpenSelectedFileInEditor,
    UploadRequested,
    FilesSelected(Option<Vec<PathBuf>>),
    DownloadRequested,
    DownloadDirectorySelected(Option<PathBuf>),
    DeleteSelectedFile,
    StartFileAction(FileActionKind),
    FileActionInputChanged(String),
    ConfirmFileAction,
    ToggleTransfersPanel,
    ClearFinishedTransfers,

    // Editor
    EditorAction(String, text_editor::Action),
    SaveActiveEditor,
    ActivateTerminalTab,
    ActivateEditorTab(String),
    CloseEditorTab(String),
    ToggleMarkdownPreview,
    MarkdownLinkClicked(String),

    // Dialogs and toasts
    ConfirmModal,
    CloseModal,
    DismissNotification(u64),
}
