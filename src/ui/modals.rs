use iced::widget::{
    Space, Text, button, column, container, mouse_area, opaque, row, text, text_input,
};
use iced::{Alignment, Color, Element, Length};

use crate::app::messages::{FileActionKind, Message};
use crate::app::state::{AppState, Confirmation, Modal, PendingFileAction};
use crate::models::editor_title;
use crate::ssh::client::HostKeyInfo;

use super::components::{self, icon, lucide};
use super::theme;

pub const FILE_ACTION_INPUT: &str = "file-action-input";

/// The active dialog, if any, rendered over a dimmed backdrop.
pub fn view(state: &AppState) -> Option<Element<'_, Message>> {
    let modal = state.modal.as_ref()?;

    let (dialog, dismissable) = match modal {
        Modal::Confirm(confirmation) => (confirm_dialog(state, confirmation), true),
        Modal::HostKey(info) => (host_key_dialog(info), false),
        Modal::FileAction(action) => (file_action_dialog(action), false),
        Modal::Properties(path) => (properties_dialog(state, path), true),
    };

    let backdrop = mouse_area(
        container(opaque(dialog))
            .center(Length::Fill)
            .padding(24)
            .style(theme::modal_backdrop),
    );
    let backdrop = if dismissable {
        backdrop.on_press(Message::CloseModal)
    } else {
        backdrop
    };

    Some(opaque(backdrop))
}

struct Dialog<'a> {
    glyph: Text<'a>,
    tone: Color,
    title: String,
    body: Element<'a, Message>,
    actions: Element<'a, Message>,
    width: f32,
}

fn dialog(spec: Dialog<'_>) -> Element<'_, Message> {
    let badge = container(icon(spec.glyph, 17.0, spec.tone))
        .center_x(Length::Fixed(36.0))
        .center_y(Length::Fixed(36.0))
        .style(theme::fill(theme::with_alpha(spec.tone, 0.14), 10.0));

    container(
        column![
            row![
                badge,
                text(spec.title)
                    .size(theme::TEXT_LG + 1.0)
                    .font(theme::SEMIBOLD)
                    .color(theme::TEXT)
            ]
            .spacing(14)
            .align_y(Alignment::Center),
            spec.body,
            container(spec.actions).align_right(Length::Fill),
        ]
        .spacing(18),
    )
    .padding(24)
    .width(Length::Fill)
    .max_width(spec.width)
    .style(theme::modal)
    .into()
}

fn paragraph<'a>(content: impl text::IntoFragment<'a>) -> Text<'a> {
    text(content).size(theme::TEXT_MD).color(theme::TEXT_MUTED)
}

fn actions<'a>(cancel: &'a str, confirm: Element<'a, Message>) -> Element<'a, Message> {
    row![
        components::text_button(cancel, Some(Message::CloseModal), theme::secondary_button),
        confirm,
    ]
    .spacing(10)
    .into()
}

fn danger_action(label: &str) -> Element<'_, Message> {
    components::text_button(label, Some(Message::ConfirmModal), theme::danger_button).into()
}

fn confirm_dialog<'a>(state: &'a AppState, confirmation: &'a Confirmation) -> Element<'a, Message> {
    let quoted = |value: &str| format!("\u{201c}{value}\u{201d}");

    let spec = match confirmation {
        Confirmation::DeleteRemote { path, is_directory } => {
            let name = editor_title(path);
            Dialog {
                glyph: lucide::trash_two(),
                tone: theme::DANGER,
                title: if *is_directory {
                    "Delete folder?".into()
                } else {
                    "Delete file?".into()
                },
                body: column![
                    paragraph(if *is_directory {
                        format!(
                            "{} and everything inside it will be permanently deleted from the server.",
                            quoted(&name)
                        )
                    } else {
                        format!(
                            "{} will be permanently deleted from the server.",
                            quoted(&name)
                        )
                    }),
                    path_box(path),
                ]
                .spacing(12)
                .into(),
                actions: actions("Cancel", danger_action("Delete")),
                width: 440.0,
            }
        }
        Confirmation::DeleteHost { label, .. } => Dialog {
            glyph: lucide::trash_two(),
            tone: theme::DANGER,
            title: "Delete saved connection?".into(),
            body: paragraph(format!(
                "{} and its stored credentials will be removed from the vault.",
                quoted(label)
            ))
            .into(),
            actions: actions("Cancel", danger_action("Delete")),
            width: 420.0,
        },
        Confirmation::DeleteKey { label, used_by, .. } => {
            let mut body = column![paragraph(format!(
                "{} will be removed from the vault.",
                quoted(label)
            ))]
            .spacing(10);
            if *used_by > 0 {
                body = body.push(warning_note(format!(
                    "{used_by} saved connection{} use this key and will need another one.",
                    if *used_by == 1 { "" } else { "s" }
                )));
            }
            Dialog {
                glyph: lucide::key_round(),
                tone: theme::DANGER,
                title: "Remove SSH key?".into(),
                body: body.into(),
                actions: actions("Cancel", danger_action("Remove")),
                width: 420.0,
            }
        }
        Confirmation::CloseTab { path } => Dialog {
            glyph: lucide::triangle_alert(),
            tone: theme::WARNING,
            title: "Discard unsaved changes?".into(),
            body: paragraph(format!(
                "{} has changes that haven\u{2019}t been saved to the server.",
                quoted(&editor_title(path))
            ))
            .into(),
            actions: actions("Keep editing", danger_action("Discard")),
            width: 420.0,
        },
        Confirmation::Disconnect { unsaved } => Dialog {
            glyph: lucide::triangle_alert(),
            tone: theme::WARNING,
            title: "Disconnect with unsaved changes?".into(),
            body: paragraph(format!(
                "{unsaved} open file{} {} unsaved changes that will be lost.",
                if *unsaved == 1 { "" } else { "s" },
                if *unsaved == 1 { "has" } else { "have" }
            ))
            .into(),
            actions: actions("Cancel", danger_action("Disconnect")),
            width: 420.0,
        },
    };

    let _ = state;
    dialog(spec)
}

fn host_key_dialog(info: &HostKeyInfo) -> Element<'_, Message> {
    let address = if info.port == 22 {
        info.host.clone()
    } else {
        format!("{}:{}", info.host, info.port)
    };

    let fingerprint = container(
        column![
            components::caption(format!("{} KEY FINGERPRINT", info.key_type)),
            text(&info.fingerprint)
                .size(theme::TEXT_MD)
                .font(theme::MONO_FONT)
                .color(theme::TEXT),
        ]
        .spacing(6),
    )
    .padding([12, 14])
    .width(Length::Fill)
    .style(theme::inset);

    let body = column![
        paragraph(format!(
            "This is the first connection to {address}. Confirm that the fingerprint matches \
             the server\u{2019}s before trusting it."
        )),
        fingerprint,
        text("Trusted keys are saved and checked on every future connection; a changed key will be blocked.")
            .size(theme::TEXT_SM)
            .color(theme::TEXT_FAINT),
    ]
    .spacing(14);

    dialog(Dialog {
        glyph: lucide::fingerprint(),
        tone: theme::ACCENT,
        title: "Verify host identity".into(),
        body: body.into(),
        actions: actions(
            "Cancel",
            components::labeled_button(
                lucide::shield_check(),
                "Trust and connect",
                Some(Message::ConfirmModal),
                theme::primary_button,
            )
            .into(),
        ),
        width: 480.0,
    })
}

fn file_action_dialog(action: &PendingFileAction) -> Element<'_, Message> {
    let name = editor_title(&action.source);
    let (glyph, title, description, confirm, placeholder) = match action.kind {
        FileActionKind::Rename => (
            lucide::pencil(),
            "Rename".to_string(),
            format!("Enter a new name for \u{201c}{name}\u{201d}."),
            "Rename",
            "New name",
        ),
        FileActionKind::Copy => (
            lucide::copy(),
            "Duplicate".to_string(),
            format!("Copy \u{201c}{name}\u{201d} to a new path on the server."),
            "Duplicate",
            "Destination path",
        ),
        FileActionKind::Move => (
            lucide::arrow_right_left(),
            "Move".to_string(),
            format!("Move \u{201c}{name}\u{201d} to a different location."),
            "Move",
            "Destination path",
        ),
        FileActionKind::NewFolder => (
            lucide::folder_plus(),
            "New folder".to_string(),
            format!("Create a folder inside {}.", action.source),
            "Create",
            "Folder name",
        ),
    };

    let input = text_input(placeholder, &action.value)
        .id(FILE_ACTION_INPUT)
        .on_input(Message::FileActionInputChanged)
        .on_submit(Message::ConfirmFileAction)
        .padding([10, 12])
        .size(theme::TEXT_MD)
        .font(theme::MONO_FONT)
        .style(theme::input);

    dialog(Dialog {
        glyph,
        tone: theme::ACCENT,
        title,
        body: column![
            paragraph(description),
            input,
            text("Relative paths are resolved against the current folder.")
                .size(theme::TEXT_SM)
                .color(theme::TEXT_FAINT),
        ]
        .spacing(12)
        .into(),
        actions: actions(
            "Cancel",
            components::text_button(
                confirm,
                (!action.value.trim().is_empty()).then_some(Message::ConfirmFileAction),
                theme::primary_button,
            )
            .into(),
        ),
        width: 460.0,
    })
}

fn properties_dialog<'a>(state: &'a AppState, path: &'a str) -> Element<'a, Message> {
    let Some(entry) = state.file(path) else {
        return dialog(Dialog {
            glyph: lucide::info(),
            tone: theme::ACCENT,
            title: "Properties".into(),
            body: paragraph("This entry is no longer available.").into(),
            actions: components::text_button(
                "Close",
                Some(Message::CloseModal),
                theme::secondary_button,
            )
            .into(),
            width: 420.0,
        });
    };

    let kind = match (entry.is_directory(), entry.is_symlink) {
        (true, true) => "Folder (symbolic link)",
        (true, false) => "Folder",
        (false, true) => "File (symbolic link)",
        (false, false) => "File",
    };
    let modified = entry
        .modified
        .map(|timestamp| {
            timestamp
                .with_timezone(&chrono::Local)
                .format("%b %-d, %Y at %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "Unknown".into());

    let rows = column![
        property("Type", kind.to_string(), false),
        property("Location", entry.path.clone(), true),
        property(
            "Size",
            if entry.is_directory() {
                "\u{2014}".into()
            } else {
                format!(
                    "{} ({} bytes)",
                    components::format_bytes(entry.size),
                    entry.size
                )
            },
            false
        ),
        property("Permissions", entry.permissions.clone(), true),
        property(
            "Owner (UID)",
            entry.owner.clone().unwrap_or_else(|| "Unknown".into()),
            true
        ),
        property("Modified", modified, false),
    ]
    .spacing(10);

    dialog(Dialog {
        glyph: if entry.is_directory() {
            lucide::folder()
        } else {
            lucide::file_text()
        },
        tone: theme::ACCENT,
        title: entry.name.clone(),
        body: container(rows)
            .padding([14, 16])
            .width(Length::Fill)
            .style(theme::inset)
            .into(),
        actions: components::text_button(
            "Close",
            Some(Message::CloseModal),
            theme::secondary_button,
        )
        .into(),
        width: 460.0,
    })
}

fn property<'a>(label: &'a str, value: String, mono: bool) -> Element<'a, Message> {
    row![
        text(label)
            .size(theme::TEXT_SM)
            .color(theme::TEXT_FAINT)
            .width(Length::Fixed(110.0)),
        text(value)
            .size(theme::TEXT_SM)
            .font(if mono {
                theme::MONO_FONT
            } else {
                theme::UI_FONT
            })
            .color(theme::TEXT)
            .width(Length::Fill),
    ]
    .spacing(12)
    .into()
}

fn path_box(path: &str) -> Element<'_, Message> {
    container(
        text(path)
            .size(theme::TEXT_SM)
            .font(theme::MONO_FONT)
            .color(theme::TEXT_MUTED),
    )
    .padding([8, 12])
    .width(Length::Fill)
    .style(theme::inset)
    .into()
}

fn warning_note<'a>(message: String) -> Element<'a, Message> {
    container(
        row![
            icon(lucide::triangle_alert(), 14.0, theme::WARNING),
            text(message).size(theme::TEXT_SM).color(theme::TEXT),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([8, 12])
    .width(Length::Fill)
    .style(theme::banner(theme::WARNING))
    .into()
}

/// Toast notifications stacked in the bottom-right corner.
pub fn notifications(state: &AppState) -> Option<Element<'_, Message>> {
    if state.notifications.is_empty() {
        return None;
    }

    let toasts = column(state.notifications.iter().map(|notification| {
        use crate::app::state::NotificationLevel;

        let (glyph, color) = match notification.level {
            NotificationLevel::Info => (lucide::info(), theme::ACCENT_TEXT),
            NotificationLevel::Success => (lucide::circle_check(), theme::SUCCESS),
            NotificationLevel::Error => (lucide::circle_alert(), theme::DANGER),
        };

        container(
            row![
                container(Space::new().width(3).height(18)).style(theme::fill(color, 2.0)),
                icon(glyph, 15.0, color),
                text(&notification.message)
                    .size(theme::TEXT_MD)
                    .color(theme::TEXT)
                    .width(Length::Fill),
                button(icon(lucide::x(), 12.0, theme::TEXT_FAINT))
                    .padding(4)
                    .on_press(Message::DismissNotification(notification.id))
                    .style(theme::tab_close),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .height(Length::Shrink),
        )
        .padding(iced::Padding {
            top: 10.0,
            right: 8.0,
            bottom: 10.0,
            left: 8.0,
        })
        .width(Length::Fixed(360.0))
        .style(theme::toast)
        .into()
    }))
    .spacing(8)
    .align_x(Alignment::End);

    Some(
        container(toasts)
            .align_right(Length::Fill)
            .align_bottom(Length::Fill)
            .padding(iced::Padding {
                top: 16.0,
                right: 16.0,
                bottom: theme::STATUS_BAR_HEIGHT + 16.0,
                left: 16.0,
            })
            .into(),
    )
}
