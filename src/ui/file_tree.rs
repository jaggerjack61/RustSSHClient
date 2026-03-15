use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input, tooltip, Space};
use iced::{Color, Element, Length};

use crate::app::messages::{FileActionKind, Message};
use crate::app::state::AppState;
use crate::models::FileEntry;

use super::styles;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let can_navigate_up = state.workspace.current_directory != "/";
    let up_button = if can_navigate_up {
        button(text("Up").size(12).color(Color::WHITE))
            .on_press(Message::NavigateUpDirectory)
            .padding([4, 10])
            .style(styles::ghost_button)
    } else {
        button(text("Up").size(12).color(styles::text_slate_500()))
            .padding([4, 10])
            .style(styles::ghost_button)
    };

    let refresh_button = explorer_icon_button(
        "R",
        12,
        Message::RefreshDirectory,
        "Refresh current directory",
    );
    let upload_button = explorer_icon_button(
        "+",
        14,
        Message::UploadRequested,
        "Upload files to current directory",
    );

    // --- Header ---
    let header = container(column![
        row![
            text("EXPLORER")
                .size(10)
                .color(styles::text_slate_500()),
            Space::new().width(Length::Fill),
            refresh_button,
            upload_button,
        ]
        .align_y(iced::Alignment::Center),
        row![
            up_button,
            text(&state.workspace.current_directory)
                .size(12)
                .color(Color::WHITE)
                .width(Length::Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(10))
    .padding([12, 16])
    .style(styles::title_bar);

    // --- File entries ---
    let entries = state
        .workspace
        .files
        .iter()
        .fold(column![].spacing(6), |col, entry| {
            let is_selected = state
                .workspace
                .selected_file
                .as_deref()
                == Some(&entry.path);

            let (icon, icon_color) = if entry.is_directory() {
                ("\u{1F4C1}", styles::blue_400())
            } else {
                let ext = entry
                    .name
                    .rsplit('.')
                    .next()
                    .unwrap_or("");
                match ext {
                    "js" => ("\u{1F4C4}", styles::orange_400()),
                    "ts" => ("\u{1F4C4}", styles::blue_400()),
                    "json" => ("\u{1F4C4}", styles::orange_400()),
                    "md" => ("\u{1F4C4}", styles::text_slate_400()),
                    "env" | "cfg" | "conf" => ("\u{1F4C4}", styles::red_400()),
                    _ => ("\u{1F4C4}", styles::text_slate_400()),
                }
            };

            let card = button(
                row![
                    text(icon).size(14).color(icon_color),
                    text(&entry.name)
                        .size(13)
                        .color(if is_selected {
                            Color::WHITE
                        } else {
                            styles::text_slate_400()
                        })
                        .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::ExplorerEntryPressed(entry.path.clone()))
            .padding([6, 10])
            .width(Length::Fill)
            .style(if is_selected {
                styles::file_entry_active as fn(&iced::Theme, button::Status) -> button::Style
            } else {
                styles::file_entry_button
            });

            let card = mouse_area(card)
                .on_right_press(Message::ExplorerEntrySecondaryPressed(entry.path.clone()));

            col.push(card)
        });

    let file_list = scrollable(entries)
        .style(styles::dark_scrollable)
        .height(Length::Fill);

    let file_list: Element<'_, Message> = if let Some(entry) = context_menu_entry(state) {
        stack([
            container(file_list)
                .padding([0, 8])
                .height(Length::Fill)
                .into(),
            container(
                mouse_area(container(
                    column![
                        row![
                            Space::new().width(Length::Fill),
                            opaque(context_menu(entry)),
                        ]
                        .width(Length::Fill),
                        Space::new().height(Length::Fill),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill),
                ))
                .on_press(Message::DismissExplorerContextMenu),
            )
            .padding([10, 8])
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        container(file_list)
            .padding([0, 8])
            .height(Length::Fill)
            .into()
    };

    // --- Details panel ---
    let details: Element<'_, Message> = if let Some(entry) = state.selected_file() {
        let mut panel = column![
            text("Properties")
                .size(13)
                .color(styles::text_slate_500()),
            text(&entry.name).size(14).color(Color::WHITE),
            text(if entry.is_directory() { "Folder" } else { "File" })
                .size(11)
                .color(styles::text_slate_500()),
            text(format!("Path: {}", entry.path))
                .size(12)
                .color(styles::text_slate_400()),
            text(format!("Size: {}", styles::format_bytes(entry.size)))
                .size(12)
                .color(styles::text_slate_400()),
            text(format!("Permissions: {}", entry.permissions))
                .size(12)
                .color(styles::text_slate_400()),
            text(format!(
                "Modified: {}",
                styles::format_timestamp(entry.modified)
            ))
            .size(12)
            .color(styles::text_slate_400()),
            text(format!(
                "Owner: {}",
                entry.owner.as_deref().unwrap_or("Unknown")
            ))
            .size(12)
            .color(styles::text_slate_400()),
            text("Right-click an entry to open file actions.")
                .size(11)
                .color(styles::text_slate_500()),
        ]
        .spacing(6);

        if let Some(action) = &state.workspace.pending_file_action {
            panel = panel.push(
                column![
                    text(action_label(action.kind))
                        .size(11)
                        .color(styles::text_slate_500()),
                    text_input("Target path", &action.value)
                        .on_input(Message::FileActionInputChanged)
                        .padding([8, 12])
                        .style(styles::dark_input),
                    row![
                        button(
                            text("Apply")
                                .size(12)
                                .color(Color::WHITE),
                        )
                        .on_press(Message::ConfirmFileAction)
                        .padding([6, 16])
                        .style(styles::primary_button),
                        button(
                            text("Cancel")
                                .size(12)
                                .color(styles::text_slate_400()),
                        )
                        .on_press(Message::CancelFileAction)
                        .padding([6, 16])
                        .style(styles::ghost_button),
                    ]
                    .spacing(8),
                ]
                .spacing(8),
            );
        }

        container(panel)
            .width(Length::Fill)
            .padding(14)
            .style(styles::details_panel)
            .into()
    } else {
        container(
            column![
                text("Properties")
                    .size(13)
                    .color(styles::text_slate_500()),
                text("Select a file or folder")
                    .size(14)
                    .color(Color::WHITE),
                text("Details for the current selection appear here.")
                    .size(12)
                    .color(styles::text_slate_400()),
            ]
            .spacing(6),
        )
        .width(Length::Fill)
        .padding(14)
        .style(styles::details_panel)
        .into()
    };

    // --- Footer: connected status ---
    let footer = container(
        row![
            container(Space::new().width(6).height(6)).style(|_theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(styles::emerald_400())),
                border: iced::Border {
                    radius: 3.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            column![
                text("Remote explorer")
                    .size(10)
                    .color(styles::text_slate_500()),
                text(latency_label(state.workspace.latency_ms))
                    .size(11)
                    .color(styles::text_slate_400()),
            ]
            .spacing(2),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 16])
    .width(Length::Fill)
    .style(styles::status_bar);

    // --- Assemble sidebar ---
    container(
        column![
            header,
            file_list,
            details,
            footer,
        ],
    )
    .width(260)
    .height(Length::Fill)
    .style(styles::explorer_sidebar)
    .into()
}

fn explorer_icon_button<'a>(
    label: &'static str,
    size: u32,
    message: Message,
    tooltip_text: &'static str,
) -> Element<'a, Message> {
    let button = button(text(label).size(size).color(styles::text_slate_400()))
        .on_press(message)
        .padding([2, 8])
        .style(styles::ghost_button);

    tooltip(
        button,
        container(text(tooltip_text).size(12).color(Color::WHITE))
            .padding([8, 10])
            .style(styles::tooltip_container),
        tooltip::Position::Bottom,
    )
    .style(styles::tooltip_container)
    .into()
}

fn context_menu<'a>(entry: &'a FileEntry) -> Element<'a, Message> {
    let header = column![
        text("ACTIONS")
            .size(10)
            .color(styles::text_slate_500()),
        text(&entry.name)
            .size(12)
            .color(Color::WHITE),
    ]
    .spacing(2);

    let mut menu = column![header].spacing(4);

    if can_open_in_editor(entry) {
        menu = menu.push(context_menu_button(
            "\u{270D}",
            "Open in Editor",
            Message::OpenSelectedFileInEditor,
            false,
        ));
    }

    menu = menu
        .push(context_menu_button(
            "\u{2B07}",
            "Download",
            Message::DownloadRequested,
            false,
        ))
        .push(context_menu_button(
            "\u{270E}",
            "Rename",
            Message::StartFileAction(FileActionKind::Rename),
            false,
        ))
        .push(context_menu_button(
            "\u{2398}",
            "Copy",
            Message::StartFileAction(FileActionKind::Copy),
            false,
        ))
        .push(context_menu_button(
            "\u{21C4}",
            "Move",
            Message::StartFileAction(FileActionKind::Move),
            false,
        ))
        .push(context_menu_button(
            "\u{2715}",
            "Delete",
            Message::DeleteSelectedFile,
            true,
        ));

    container(menu)
        .width(190)
        .padding(8)
        .style(styles::context_menu_panel)
        .into()
}

fn context_menu_button<'a>(
    icon: &'static str,
    label: &'static str,
    message: Message,
    destructive: bool,
) -> Element<'a, Message> {
    let content = row![
        text(icon)
            .size(12)
            .color(if destructive {
                styles::red_400()
            } else {
                styles::text_slate_400()
            }),
        text(label)
            .size(12)
            .color(if destructive {
                styles::red_400()
            } else {
                Color::WHITE
            }),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    button(content)
        .on_press(message)
        .padding([8, 10])
        .width(Length::Fill)
        .style(if destructive {
            styles::context_menu_danger_button
        } else {
            styles::context_menu_button
        })
        .into()
}

fn action_label(kind: FileActionKind) -> &'static str {
    match kind {
        FileActionKind::Rename => "Rename target",
        FileActionKind::Copy => "Copy destination",
        FileActionKind::Move => "Move destination",
    }
}

fn can_open_in_editor(entry: &FileEntry) -> bool {
    !entry.is_directory()
}

fn context_menu_entry<'a>(state: &'a AppState) -> Option<&'a FileEntry> {
    let open_path = state.workspace.explorer_context_for.as_deref()?;

    state
        .workspace
        .files
        .iter()
        .find(|entry| entry.path == open_path)
}

#[cfg(test)]
fn explorer_entry_paths(state: &AppState) -> Vec<&str> {
    state
        .workspace
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect()
}

fn latency_label(latency_ms: Option<u128>) -> String {
    latency_ms
        .map(|value| format!("Connected · {value} ms"))
        .unwrap_or_else(|| "Connected".into())
}

#[cfg(test)]
mod tests {
    use crate::app::state::AppState;
    use crate::models::{FileEntry, FileKind};

    #[test]
    fn shows_open_in_editor_for_any_file() {
        let text_entry = FileEntry {
            name: "main.rs".into(),
            path: "/srv/app/src/main.rs".into(),
            kind: FileKind::File,
            size: 32,
            permissions: "-rw-r--r--".into(),
            owner: None,
            modified: None,
        };
        let binary_entry = FileEntry {
            name: "logo.png".into(),
            path: "/srv/app/logo.png".into(),
            kind: FileKind::File,
            size: 4096,
            permissions: "-rw-r--r--".into(),
            owner: None,
            modified: None,
        };
        let directory_entry = FileEntry {
            name: "src".into(),
            path: "/srv/app/src".into(),
            kind: FileKind::Directory,
            size: 0,
            permissions: "drwxr-xr-x".into(),
            owner: None,
            modified: None,
        };

        assert!(super::can_open_in_editor(&text_entry));
        assert!(super::can_open_in_editor(&binary_entry));
        assert!(!super::can_open_in_editor(&directory_entry));
    }

    #[test]
    fn formats_latency_label_without_zero_placeholder() {
        assert_eq!(super::latency_label(None), "Connected");
        assert_eq!(super::latency_label(Some(42)), "Connected · 42 ms");
    }

    #[test]
    fn context_menu_entry_uses_overlay_path() {
        let mut state = ui_state_with_files();
        state.workspace.explorer_context_for = Some("/srv/app/README.md".into());

        let entry = super::context_menu_entry(&state).expect("context menu entry");

        assert_eq!(entry.name, "README.md");
    }

    #[test]
    fn context_menu_entry_is_none_when_path_is_missing() {
        let mut state = ui_state_with_files();
        state.workspace.explorer_context_for = Some("/srv/app/missing.txt".into());

        assert!(super::context_menu_entry(&state).is_none());
    }

    #[test]
    fn explorer_entry_paths_do_not_change_when_overlay_is_open() {
        let mut state = ui_state_with_files();
        let expected = vec!["/srv/app", "/srv/app/README.md"];

        state.workspace.explorer_context_for = Some("/srv/app/README.md".into());

        assert_eq!(super::explorer_entry_paths(&state), expected);
    }

    fn ui_state_with_files() -> AppState {
        let (mut state, _) = AppState::boot();
        state.workspace.files = vec![
            FileEntry {
                name: "app".into(),
                path: "/srv/app".into(),
                kind: FileKind::Directory,
                size: 0,
                permissions: "drwxr-xr-x".into(),
                owner: Some("root".into()),
                modified: None,
            },
            FileEntry {
                name: "README.md".into(),
                path: "/srv/app/README.md".into(),
                kind: FileKind::File,
                size: 128,
                permissions: "-rw-r--r--".into(),
                owner: Some("root".into()),
                modified: None,
            },
        ];
        state
    }
}
