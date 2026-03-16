use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input, tooltip, Space};
use iced::{Color, Element, Length};

use crate::app::messages::{FileActionKind, Message};
use crate::app::state::AppState;
use crate::models::{FileEntry, FileKind};

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
    let header = container(
        container(
            row![
                column![
                    text("EXPLORER")
                        .size(10)
                        .color(styles::text_slate_500()),
                    row![
                        up_button,
                        text(&state.workspace.current_directory)
                            .size(12)
                            .color(Color::WHITE)
                            .width(Length::Fill),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                ]
                .spacing(2)
                .width(Length::Fill),
                refresh_button,
                upload_button,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .center_y(Length::Fill),
    )
    .padding([8, 16])
    .height(Length::Fixed(styles::workspace_header_height()))
    .style(styles::title_bar);

    // --- File entries (hierarchical tree) ---
    let tree = build_tree(&state.workspace.files);
    let entries = render_tree_nodes(state, &tree, 0);

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

    // --- Properties panel (only visible when show_properties is true) ---
    let details: Element<'_, Message> = if state.workspace.show_properties {
        if let Some(entry) = state.selected_file() {
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

            let panel_content = container(panel)
                .width(Length::Fill)
                .padding(14)
                .style(styles::details_panel);

            panel_content.into()
        } else {
            let panel_content = container(
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
            .style(styles::details_panel);

            panel_content.into()
        }
    } else {
        // Properties panel is hidden – show nothing
        Space::new().height(0).into()
    };

    // --- Footer: connected status ---
    let footer = container(
        container(
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
        .width(Length::Fill)
        .center_y(Length::Fill),
    )
    .padding([8, 16])
    .height(Length::Fixed(styles::workspace_footer_height()))
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

// ---------------------------------------------------------------------------
// Tree data structure
// ---------------------------------------------------------------------------

struct TreeNode<'a> {
    entry: &'a FileEntry,
    children: Vec<TreeNode<'a>>,
}

fn build_tree<'a>(files: &'a [FileEntry]) -> Vec<TreeNode<'a>> {
    let entry_paths = files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<std::collections::HashSet<_>>();

    build_children(files, None, &entry_paths)
}

fn build_children<'a>(
    files: &'a [FileEntry],
    parent: Option<&str>,
    entry_paths: &std::collections::HashSet<&'a str>,
) -> Vec<TreeNode<'a>> {
    let mut children = files
        .iter()
        .filter(|entry| match parent {
            Some(parent_entry_path) => parent_path(&entry.path) == Some(parent_entry_path),
            None => parent_path(&entry.path).is_none_or(|path| !entry_paths.contains(path)),
        })
        .collect::<Vec<_>>();

    children.sort_by(|left, right| {
        left.is_directory()
            .cmp(&right.is_directory())
            .reverse()
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });

    children
        .into_iter()
        .map(|entry| TreeNode {
            entry,
            children: if entry.is_directory() {
                build_children(files, Some(entry.path.as_str()), entry_paths)
            } else {
                Vec::new()
            },
        })
        .collect()
}

fn parent_path(path: &str) -> Option<&str> {
    path.rsplit_once('/').and_then(|(parent, _)| {
        if parent.is_empty() {
            None
        } else {
            Some(parent)
        }
    })
}

fn render_tree_nodes<'a>(
    state: &'a AppState,
    nodes: &[TreeNode<'a>],
    depth: u16,
) -> iced::widget::Column<'a, Message> {
    let mut col = column![].spacing(2);

    for node in nodes {
        col = col.push(render_entry_row(state, node.entry, depth));

        // If this is an expanded directory, render its children indented
        if node.entry.is_directory()
            && state
                .workspace
                .expanded_folders
                .contains(&node.entry.path)
        {
            if !node.children.is_empty() {
                col = col.push(render_tree_nodes(state, &node.children, depth + 1));
            }
        }
    }

    col
}

fn render_entry_row<'a>(
    state: &'a AppState,
    entry: &'a FileEntry,
    depth: u16,
) -> Element<'a, Message> {
    let is_selected = state.workspace.selected_file.as_deref() == Some(&entry.path);
    let is_expanded = entry.is_directory()
        && state
            .workspace
            .expanded_folders
            .contains(&entry.path);
    let is_loading = entry.is_directory()
        && state
            .workspace
            .loading_folders
            .contains(&entry.path);

    let (icon, icon_color) = file_icon(entry, is_expanded);
    let indent = (depth as f32) * 16.0;

    // Build the chevron for directories
    let chevron: Element<'_, Message> = if entry.is_directory() {
        text(directory_indicator(is_expanded, is_loading))
            .size(10)
            .color(styles::text_slate_500())
            .into()
    } else {
        Space::new().width(10).into()
    };

    let card = button(
        row![
            Space::new().width(indent),
            chevron,
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
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::ExplorerEntryPressed(entry.path.clone()))
    .padding([5, 10])
    .width(Length::Fill)
    .style(if is_selected {
        styles::file_entry_active as fn(&iced::Theme, button::Status) -> button::Style
    } else {
        styles::file_entry_button
    });

    let card = mouse_area(card)
        .on_right_press(Message::ExplorerEntrySecondaryPressed(entry.path.clone()))
        .on_double_click(Message::ExplorerEntryDoubleClicked(entry.path.clone()));

    card.into()
}

fn directory_indicator(is_expanded: bool, is_loading: bool) -> &'static str {
    if is_loading {
        "..."
    } else if is_expanded {
        "\u{25BE}"
    } else {
        "\u{25B8}"
    }
}

// ---------------------------------------------------------------------------
// File type icons
// ---------------------------------------------------------------------------

fn file_icon(entry: &FileEntry, is_expanded: bool) -> (&'static str, Color) {
    if entry.is_directory() {
        if is_expanded {
            return ("\u{1F4C2}", styles::blue_400()); // 📂 open folder
        }
        return ("\u{1F4C1}", styles::blue_400()); // 📁 closed folder
    }

    if matches!(entry.kind, FileKind::Symlink) {
        return ("\u{1F517}", styles::text_slate_400()); // 🔗 link
    }

    let ext = entry.name.rsplit('.').next().unwrap_or("");
    match ext {
        // Code files
        "rs" => ("\u{2699}", Color::from_rgb8(0xDE, 0x98, 0x3B)),        // ⚙ rust orange
        "py" => ("\u{1F40D}", Color::from_rgb8(0x3B, 0x78, 0xA8)),       // 🐍 python blue
        "js" | "jsx" => ("\u{26A1}", styles::orange_400()),                // ⚡ JS yellow-orange
        "ts" | "tsx" => ("\u{1F4DC}", styles::blue_400()),                 // 📜 TS blue
        "go" => ("\u{1F439}", Color::from_rgb8(0x00, 0xAD, 0xD8)),        // 🐹 go cyan
        "java" | "kt" => ("\u{2615}", Color::from_rgb8(0xB0, 0x72, 0x19)), // ☕ java brown
        "c" | "h" => ("\u{1F6E0}", Color::from_rgb8(0x55, 0x99, 0xCC)),   // 🛠 c blue
        "cpp" | "cc" | "cxx" | "hpp" => ("\u{1F6E0}", Color::from_rgb8(0x66, 0x4E, 0xA8)), // 🛠 cpp purple
        "rb" => ("\u{1F48E}", Color::from_rgb8(0xCC, 0x34, 0x2D)),        // 💎 ruby red
        "php" => ("\u{1F418}", Color::from_rgb8(0x77, 0x7B, 0xB3)),       // 🐘 php purple
        "sh" | "bash" | "zsh" | "fish" => ("\u{1F4DF}", styles::emerald_400()), // 📟 shell green
        "lua" => ("\u{1F319}", styles::blue_400()),                        // 🌙 lua blue
        "r" | "R" => ("\u{1F4CA}", styles::blue_400()),                    // 📊 R blue
        "swift" => ("\u{1F426}", styles::orange_400()),                    // 🐦 swift orange
        "css" | "scss" | "sass" | "less" => ("\u{1F3A8}", styles::blue_400()), // 🎨 css blue
        "html" | "htm" => ("\u{1F310}", styles::orange_400()),             // 🌐 html orange

        // Data / config
        "json" => ("\u{1F4CB}", styles::orange_400()),              // 📋 json orange
        "yml" | "yaml" => ("\u{1F4CB}", Color::from_rgb8(0xCB, 0x17, 0x1E)), // 📋 yaml red
        "toml" => ("\u{1F4CB}", Color::from_rgb8(0x9C, 0x4E, 0x21)), // 📋 toml brown
        "xml" => ("\u{1F4CB}", styles::orange_400()),               // 📋 xml orange
        "csv" => ("\u{1F4CA}", styles::emerald_400()),              // 📊 csv green
        "sql" => ("\u{1F5C4}", styles::blue_400()),                 // 🗄 sql blue
        "env" | "cfg" | "conf" | "ini" => ("\u{2699}", styles::text_slate_400()), // ⚙ config grey

        // Documents
        "md" | "mdx" => ("\u{1F4DD}", styles::text_slate_400()),   // 📝 markdown grey
        "txt" | "log" => ("\u{1F4C4}", styles::text_slate_400()),  // 📄 text grey
        "pdf" => ("\u{1F4D5}", styles::red_400()),                 // 📕 pdf red
        "doc" | "docx" => ("\u{1F4C3}", styles::blue_400()),       // 📃 word blue
        "xls" | "xlsx" => ("\u{1F4CA}", styles::emerald_400()),    // 📊 excel green
        "ppt" | "pptx" => ("\u{1F4CA}", styles::orange_400()),    // 📊 ppt orange

        // Images
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "bmp" | "webp" | "ico" => {
            ("\u{1F5BC}", Color::from_rgb8(0xA7, 0x8B, 0xFA))     // 🖼 image purple
        }

        // Archives
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => {
            ("\u{1F4E6}", Color::from_rgb8(0xFB, 0xBF, 0x24))     // 📦 archive yellow
        }

        // Binary / executables
        "exe" | "dll" | "so" | "dylib" | "bin" | "o" | "a" => {
            ("\u{2699}", styles::text_slate_500())                  // ⚙ binary grey
        }

        // Lock files
        "lock" => ("\u{1F512}", styles::text_slate_500()),         // 🔒 lock grey

        // Fonts
        "ttf" | "otf" | "woff" | "woff2" => {
            ("\u{1F524}", styles::text_slate_400())                 // 🔤 font grey
        }

        // Video / audio
        "mp3" | "wav" | "flac" | "ogg" | "aac" => {
            ("\u{1F3B5}", Color::from_rgb8(0xF4, 0x72, 0xB6))     // 🎵 audio pink
        }
        "mp4" | "mkv" | "avi" | "mov" | "webm" => {
            ("\u{1F3AC}", Color::from_rgb8(0xF4, 0x72, 0xB6))     // 🎬 video pink
        }

        // Docker
        "dockerfile" | "Dockerfile" => ("\u{1F433}", styles::blue_400()), // 🐳 docker blue

        _ => ("\u{1F4C4}", styles::text_slate_400()),              // 📄 default
    }
}

// ---------------------------------------------------------------------------
// Helper widgets
// ---------------------------------------------------------------------------

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
            "\u{2139}",
            "Properties",
            Message::ShowProperties,
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
        .map(|value| format!("Connected \u{00B7} {value} ms"))
        .unwrap_or_else(|| "Connected".into())
}

#[cfg(test)]
mod tests {
    use crate::app::state::AppState;
    use crate::models::{FileEntry, FileKind};

    #[test]
    fn directory_indicator_shows_loading_state() {
        assert_eq!(super::directory_indicator(false, true), "...");
        assert_eq!(super::directory_indicator(true, false), "\u{25BE}");
        assert_eq!(super::directory_indicator(false, false), "\u{25B8}");
    }

    #[test]
    fn build_tree_nests_child_directories_under_their_parent() {
        let files = vec![
            entry("app", "/srv/app", FileKind::Directory),
            entry("src", "/srv/app/src", FileKind::Directory),
            entry("main.rs", "/srv/app/src/main.rs", FileKind::File),
            entry("README.md", "/srv/app/README.md", FileKind::File),
        ];

        let tree = super::build_tree(&files);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].entry.path, "/srv/app");
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].entry.path, "/srv/app/src");
        assert_eq!(tree[0].children[0].children.len(), 1);
        assert_eq!(tree[0].children[0].children[0].entry.path, "/srv/app/src/main.rs");
        assert_eq!(tree[0].children[1].entry.path, "/srv/app/README.md");
    }

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
        assert_eq!(super::latency_label(Some(42)), "Connected \u{00B7} 42 ms");
    }

    #[test]
    fn explorer_chrome_uses_shared_workspace_heights() {
        assert_eq!(super::styles::workspace_header_height(), 56.0);
        assert_eq!(super::styles::workspace_footer_height(), 40.0);
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

    fn entry(name: &str, path: &str, kind: FileKind) -> FileEntry {
        FileEntry {
            name: name.into(),
            path: path.into(),
            kind,
            size: 0,
            permissions: "-rw-r--r--".into(),
            owner: Some("root".into()),
            modified: None,
        }
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
