use std::collections::{HashMap, HashSet};

use iced::widget::{
    Space, Text, button, column, container, mouse_area, opaque, pin, progress_bar, responsive, row,
    scrollable, stack, text,
};
use iced::{Alignment, Color, Element, Length, Size};

use crate::app::messages::{FileActionKind, Message};
use crate::app::state::AppState;
use crate::models::{FileEntry, TransferDirection, TransferProgress, TransferStatus};

use super::components::{self, icon, lucide};
use super::theme;

pub const TREE_SCROLLABLE: &str = "explorer-tree";
const ROW_HEIGHT: f32 = 26.0;
const ROW_SPACING: f32 = 1.0;
const TREE_PADDING: f32 = 6.0;
const INDENT: f32 = 14.0;
const MENU_WIDTH: f32 = 196.0;
const MENU_ITEM_HEIGHT: f32 = 30.0;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let workspace = &state.workspace;
    let can_go_up = workspace.current_directory != "/";

    let toolbar = row![
        components::caption("EXPLORER"),
        Space::new().width(Length::Fill),
        components::icon_button(
            lucide::house(),
            "Home directory",
            Some(Message::NavigateHome)
        ),
        components::icon_button(
            lucide::arrow_up(),
            "Parent folder",
            can_go_up.then_some(Message::NavigateUpDirectory),
        ),
        components::icon_button(
            lucide::refresh_cw(),
            "Refresh",
            Some(Message::RefreshDirectory)
        ),
        components::icon_button(
            lucide::folder_plus(),
            "New folder",
            Some(Message::StartFileAction(FileActionKind::NewFolder)),
        ),
        components::icon_button(
            lucide::upload(),
            "Upload files",
            Some(Message::UploadRequested)
        ),
    ]
    .spacing(1)
    .align_y(Alignment::Center);

    let header = container(toolbar)
        .padding([0, 8])
        .height(theme::HEADER_HEIGHT)
        .center_y(theme::HEADER_HEIGHT)
        .padding(iced::Padding {
            left: 14.0,
            right: 8.0,
            ..iced::Padding::ZERO
        });

    let content = column![
        header,
        breadcrumbs(state),
        components::horizontal_divider(),
        container(tree_area(state)).height(Length::Fill),
        transfers_panel(state),
    ]
    .height(Length::Fill);

    container(content)
        .width(theme::EXPLORER_WIDTH)
        .height(Length::Fill)
        .style(theme::sidebar)
        .into()
}

fn breadcrumbs(state: &AppState) -> Element<'_, Message> {
    let current = &state.workspace.current_directory;
    let mut crumbs = row![].spacing(2).align_y(Alignment::Center);

    let mut path = String::new();
    let segments = current
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    crumbs = crumbs.push(crumb("/", "/".to_string(), segments.is_empty()));
    for (index, segment) in segments.iter().enumerate() {
        path.push('/');
        path.push_str(segment);
        if index > 0 {
            crumbs = crumbs.push(icon(lucide::chevron_right(), 11.0, theme::TEXT_DISABLED));
        }
        crumbs = crumbs.push(crumb(segment, path.clone(), index + 1 == segments.len()));
    }

    let loading: Element<'_, Message> = if state.workspace.pending_directory.is_some() {
        icon(lucide::loader_circle(), 12.0, theme::TEXT_FAINT).into()
    } else {
        Space::new().into()
    };

    row![
        scrollable(crumbs)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(0).scroller_width(0),
            ))
            .anchor_right()
            .width(Length::Fill),
        loading,
    ]
    .spacing(6)
    .padding(iced::Padding {
        top: 0.0,
        right: 12.0,
        bottom: 8.0,
        left: 10.0,
    })
    .align_y(Alignment::Center)
    .into()
}

fn crumb<'a>(label: &'a str, path: String, is_last: bool) -> Element<'a, Message> {
    button(
        text(label)
            .size(theme::TEXT_SM)
            .font(if is_last {
                theme::MEDIUM
            } else {
                theme::UI_FONT
            })
            .color(if is_last {
                theme::TEXT
            } else {
                theme::TEXT_FAINT
            })
            .wrapping(text::Wrapping::None),
    )
    .padding([2, 4])
    .on_press_maybe((!is_last).then_some(Message::NavigateTo(path)))
    .style(theme::ghost_button)
    .into()
}

// ---------------------------------------------------------------------------
// Tree
// ---------------------------------------------------------------------------

/// A visible row of the tree.
struct Row<'a> {
    entry: &'a FileEntry,
    depth: u16,
}

/// Flattens the loaded entries into the rows currently visible, honouring
/// the expanded folders. Runs in O(n log n).
fn visible_rows<'a>(files: &'a [FileEntry], expanded: &HashSet<String>) -> Vec<Row<'a>> {
    let paths = files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<HashSet<_>>();

    let mut children: HashMap<Option<&str>, Vec<&FileEntry>> = HashMap::new();
    for entry in files {
        let parent = parent_path(&entry.path).filter(|parent| paths.contains(parent));
        children.entry(parent).or_default().push(entry);
    }
    for siblings in children.values_mut() {
        siblings.sort_by_cached_key(|entry| (!entry.is_directory(), entry.name.to_lowercase()));
    }

    let mut rows = Vec::with_capacity(files.len());
    let mut stack = children
        .get(&None)
        .map(|roots| {
            roots
                .iter()
                .rev()
                .map(|entry| (*entry, 0_u16))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    while let Some((entry, depth)) = stack.pop() {
        rows.push(Row { entry, depth });
        if entry.is_directory()
            && expanded.contains(&entry.path)
            && let Some(nested) = children.get(&Some(entry.path.as_str()))
        {
            stack.extend(nested.iter().rev().map(|child| (*child, depth + 1)));
        }
    }

    rows
}

fn parent_path(path: &str) -> Option<&str> {
    path.rsplit_once('/')
        .and_then(|(parent, _)| (!parent.is_empty()).then_some(parent))
}

fn tree_area(state: &AppState) -> Element<'_, Message> {
    responsive(move |size| {
        let workspace = &state.workspace;
        let rows = visible_rows(&workspace.files, &workspace.expanded_folders);

        let body: Element<'_, Message> = if rows.is_empty() {
            empty_folder(workspace.pending_directory.is_some())
        } else {
            let list = column(rows.iter().map(|row| tree_row(state, row.entry, row.depth)))
                .spacing(ROW_SPACING)
                .padding(TREE_PADDING);

            scrollable(list)
                .id(TREE_SCROLLABLE)
                .on_scroll(|viewport| Message::ExplorerScrolled(viewport.absolute_offset().y))
                .style(theme::scrollbar)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        };

        let Some(menu_path) = workspace.explorer_context_for.as_deref() else {
            return body;
        };
        let Some(index) = rows.iter().position(|row| row.entry.path == menu_path) else {
            return body;
        };

        let entry = rows[index].entry;
        let depth = rows[index].depth;
        let menu = context_menu(entry);
        let position = menu_position(
            size,
            index,
            depth,
            workspace.explorer_scroll_offset,
            menu_height(entry),
        );

        stack![
            body,
            mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                .on_press(Message::DismissExplorerContextMenu)
                .on_right_press(Message::DismissExplorerContextMenu),
            pin(opaque(menu)).position(position),
        ]
        .into()
    })
    .into()
}

/// Places the context menu just below its row, flipping above the row when
/// there is not enough space underneath.
fn menu_position(
    viewport: Size,
    index: usize,
    depth: u16,
    scroll_offset: f32,
    menu_height: f32,
) -> iced::Point {
    let row_top = TREE_PADDING + index as f32 * (ROW_HEIGHT + ROW_SPACING) - scroll_offset;
    let below = row_top + ROW_HEIGHT + 2.0;
    let y = if below + menu_height > viewport.height && row_top - menu_height - 2.0 >= 0.0 {
        row_top - menu_height - 2.0
    } else {
        below.min((viewport.height - menu_height).max(0.0))
    };
    let x = (28.0 + f32::from(depth) * INDENT).min((viewport.width - MENU_WIDTH - 6.0).max(6.0));

    iced::Point::new(x, y.max(0.0))
}

fn tree_row<'a>(state: &'a AppState, entry: &'a FileEntry, depth: u16) -> Element<'a, Message> {
    let workspace = &state.workspace;
    let selected = workspace.selected_file.as_deref() == Some(entry.path.as_str());
    let is_directory = entry.is_directory();
    let expanded = is_directory && workspace.expanded_folders.contains(&entry.path);
    let loading = is_directory && workspace.loading_folders.contains(&entry.path);

    let chevron: Element<'_, Message> = if is_directory {
        let glyph = if loading {
            lucide::loader_circle()
        } else if expanded {
            lucide::chevron_down()
        } else {
            lucide::chevron_right()
        };
        icon(glyph, 12.0, theme::TEXT_FAINT).width(12).into()
    } else {
        Space::new().width(12).into()
    };

    let (glyph, color) = file_icon(entry, expanded);
    let mut label = row![
        text(&entry.name)
            .size(theme::TEXT_MD)
            .color(if selected {
                theme::TEXT
            } else {
                theme::TEXT_MUTED
            })
            .wrapping(text::Wrapping::None),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if entry.is_symlink {
        label = label.push(icon(lucide::link(), 10.0, theme::TEXT_FAINT));
    }

    let content = row![
        Space::new().width(f32::from(depth) * INDENT),
        chevron,
        icon(glyph, 14.0, color),
        container(label).width(Length::Fill).clip(true),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    // The mouse area sits inside the button so it sees presses first; the
    // button only supplies hover styling.
    let area = mouse_area(
        container(content)
            .padding([0, 8])
            .height(ROW_HEIGHT)
            .center_y(ROW_HEIGHT)
            .width(Length::Fill),
    )
    .on_press(Message::ExplorerEntryPressed(entry.path.clone()))
    .on_double_click(Message::ExplorerEntryDoubleClicked(entry.path.clone()))
    .on_right_press(Message::ExplorerEntrySecondaryPressed(entry.path.clone()));

    button(area)
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::ExplorerEntryPressed(entry.path.clone()))
        .style(theme::tree_row(selected))
        .into()
}

fn empty_folder<'a>(loading: bool) -> Element<'a, Message> {
    let (glyph, label) = if loading {
        (lucide::loader_circle(), "Loading\u{2026}")
    } else {
        (lucide::folder_open(), "This folder is empty")
    };

    container(
        column![
            icon(glyph, 20.0, theme::TEXT_DISABLED),
            text(label).size(theme::TEXT_SM).color(theme::TEXT_FAINT),
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .padding([32, 12])
    .center_x(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Context menu
// ---------------------------------------------------------------------------

struct MenuItem {
    glyph: fn() -> Text<'static>,
    label: &'static str,
    message: Message,
    danger: bool,
}

fn menu_items(entry: &FileEntry) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if entry.is_directory() {
        items.push(MenuItem {
            glyph: lucide::folder_open,
            label: "Open",
            message: Message::NavigateTo(entry.path.clone()),
            danger: false,
        });
        items.push(MenuItem {
            glyph: lucide::folder_plus,
            label: "New folder\u{2026}",
            message: Message::StartFileAction(FileActionKind::NewFolder),
            danger: false,
        });
        items.push(MenuItem {
            glyph: lucide::upload,
            label: "Upload here\u{2026}",
            message: Message::UploadRequested,
            danger: false,
        });
    } else {
        items.push(MenuItem {
            glyph: lucide::square_pen,
            label: "Open in editor",
            message: Message::OpenSelectedFileInEditor,
            danger: false,
        });
    }

    items.extend([
        MenuItem {
            glyph: lucide::download,
            label: "Download\u{2026}",
            message: Message::DownloadRequested,
            danger: false,
        },
        MenuItem {
            glyph: lucide::pencil,
            label: "Rename\u{2026}",
            message: Message::StartFileAction(FileActionKind::Rename),
            danger: false,
        },
        MenuItem {
            glyph: lucide::copy,
            label: "Duplicate to\u{2026}",
            message: Message::StartFileAction(FileActionKind::Copy),
            danger: false,
        },
        MenuItem {
            glyph: lucide::arrow_right_left,
            label: "Move to\u{2026}",
            message: Message::StartFileAction(FileActionKind::Move),
            danger: false,
        },
        MenuItem {
            glyph: lucide::info,
            label: "Properties",
            message: Message::ShowProperties,
            danger: false,
        },
        MenuItem {
            glyph: lucide::trash_two,
            label: "Delete",
            message: Message::DeleteSelectedFile,
            danger: true,
        },
    ]);
    items
}

fn menu_height(entry: &FileEntry) -> f32 {
    // Items, one separator and the panel padding.
    menu_items(entry).len() as f32 * MENU_ITEM_HEIGHT + 9.0 + 12.0
}

fn context_menu(entry: &FileEntry) -> Element<'_, Message> {
    let items = menu_items(entry);
    let last = items.len() - 1;
    let mut menu = column![].spacing(0);

    for (index, item) in items.into_iter().enumerate() {
        if index == last {
            menu = menu.push(container(components::horizontal_divider()).padding([4, 4]));
        }
        let color = if item.danger {
            theme::DANGER
        } else {
            theme::TEXT_MUTED
        };
        menu = menu.push(
            button(
                row![
                    icon((item.glyph)(), 14.0, color),
                    text(item.label).size(theme::TEXT_MD),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([0, 10])
            .height(MENU_ITEM_HEIGHT)
            .width(Length::Fill)
            .on_press(item.message)
            .style(theme::menu_item(item.danger)),
        );
    }

    container(menu)
        .width(MENU_WIDTH)
        .padding(6)
        .style(theme::popover)
        .into()
}

// ---------------------------------------------------------------------------
// Transfers
// ---------------------------------------------------------------------------

fn transfers_panel(state: &AppState) -> Element<'_, Message> {
    let transfers = &state.workspace.transfers;
    if transfers.is_empty() {
        return Space::new().into();
    }

    let active = transfers.iter().filter(|item| !item.is_finished()).count();
    let has_finished = active < transfers.len();
    let expanded = state.workspace.transfers_expanded;

    let summary = if active > 0 {
        format!("{active} active")
    } else {
        "All done".to_string()
    };

    let header = row![
        button(
            row![
                icon(
                    if expanded {
                        lucide::chevron_down()
                    } else {
                        lucide::chevron_right()
                    },
                    12.0,
                    theme::TEXT_FAINT
                ),
                components::caption("TRANSFERS"),
                text(summary).size(theme::TEXT_XS).color(if active > 0 {
                    theme::ACCENT_TEXT
                } else {
                    theme::TEXT_FAINT
                }),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([4, 4])
        .on_press(Message::ToggleTransfersPanel)
        .style(theme::ghost_button),
        Space::new().width(Length::Fill),
        components::icon_button(
            lucide::list_checks(),
            "Clear finished",
            has_finished.then_some(Message::ClearFinishedTransfers),
        ),
    ]
    .align_y(Alignment::Center);

    let mut panel = column![header].spacing(4);
    if expanded {
        panel = panel.push(
            scrollable(column(transfers.iter().map(transfer_row)).spacing(8))
                .style(theme::scrollbar)
                .height(Length::Shrink),
        );
    }

    column![
        components::horizontal_divider(),
        container(panel)
            .padding([8, 10])
            .max_height(220)
            .width(Length::Fill),
    ]
    .into()
}

fn transfer_row(transfer: &TransferProgress) -> Element<'_, Message> {
    let (glyph, direction_color) = match transfer.direction {
        TransferDirection::Upload => (lucide::arrow_up_from_line(), theme::ACCENT_TEXT),
        TransferDirection::Download => (lucide::arrow_down_to_line(), theme::SUCCESS),
        TransferDirection::Copy => (lucide::copy(), theme::TEXT_MUTED),
    };

    let (status, status_color, bar_color): (String, Color, Color) = match &transfer.status {
        TransferStatus::Queued => ("Queued".into(), theme::TEXT_FAINT, theme::TEXT_FAINT),
        TransferStatus::Running if transfer.total_bytes > 0 => (
            format!(
                "{} of {}",
                components::format_bytes(transfer.transferred_bytes),
                components::format_bytes(transfer.total_bytes)
            ),
            theme::TEXT_FAINT,
            theme::ACCENT,
        ),
        TransferStatus::Running => ("Preparing\u{2026}".into(), theme::TEXT_FAINT, theme::ACCENT),
        TransferStatus::Completed => ("Done".into(), theme::SUCCESS, theme::SUCCESS),
        TransferStatus::Failed(error) => (error.clone(), theme::DANGER, theme::DANGER),
    };

    column![
        row![
            icon(glyph, 12.0, direction_color),
            container(
                text(&transfer.label)
                    .size(theme::TEXT_SM)
                    .color(theme::TEXT)
                    .wrapping(text::Wrapping::None)
            )
            .width(Length::Fill)
            .clip(true),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        progress_bar(0.0..=1.0, transfer.percent_complete())
            .girth(3)
            .style(theme::progress(bar_color)),
        text(status)
            .size(theme::TEXT_XS)
            .color(status_color)
            .wrapping(text::Wrapping::WordOrGlyph),
    ]
    .spacing(4)
    .into()
}

// ---------------------------------------------------------------------------
// File type icons
// ---------------------------------------------------------------------------

fn file_icon(entry: &FileEntry, is_expanded: bool) -> (Text<'static>, Color) {
    const FOLDER: Color = Color::from_rgb8(0x7a, 0xa7, 0xff);
    const CODE: Color = Color::from_rgb8(0x93, 0xc5, 0xfd);
    const DATA: Color = Color::from_rgb8(0xfb, 0xbf, 0x24);
    const MEDIA: Color = Color::from_rgb8(0xc0, 0x84, 0xfc);
    const ARCHIVE: Color = Color::from_rgb8(0xf5, 0x9e, 0x0b);
    const SHELL: Color = Color::from_rgb8(0x34, 0xd3, 0x99);

    if entry.is_directory() {
        let glyph = if entry.is_symlink {
            lucide::folder_symlink()
        } else if is_expanded {
            lucide::folder_open()
        } else {
            lucide::folder()
        };
        return (glyph, FOLDER);
    }

    let name = entry.name.to_ascii_lowercase();
    let extension = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");

    match (name.as_str(), extension) {
        ("dockerfile" | "makefile" | "procfile", _) => (lucide::file_cog(), theme::TEXT_MUTED),
        (
            _,
            "rs" | "py" | "js" | "jsx" | "ts" | "tsx" | "go" | "java" | "kt" | "c" | "h" | "cpp"
            | "cc" | "hpp" | "rb" | "php" | "swift" | "lua" | "cs" | "html" | "htm" | "css"
            | "scss" | "vue" | "svelte",
        ) => (lucide::file_code(), CODE),
        (_, "sh" | "bash" | "zsh" | "fish" | "ps1") => (lucide::file_terminal(), SHELL),
        (_, "json" | "yml" | "yaml" | "toml" | "xml" | "ini" | "cfg" | "conf" | "env") => {
            (lucide::file_cog(), DATA)
        }
        (_, "csv" | "tsv" | "xls" | "xlsx") => (lucide::file_spreadsheet(), SHELL),
        (_, "png" | "jpg" | "jpeg" | "gif" | "svg" | "bmp" | "webp" | "ico") => {
            (lucide::file_image(), MEDIA)
        }
        (_, "mp3" | "wav" | "flac" | "ogg" | "aac") => (lucide::file_audio(), MEDIA),
        (_, "mp4" | "mkv" | "avi" | "mov" | "webm") => (lucide::file_video(), MEDIA),
        (_, "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst") => {
            (lucide::file_archive(), ARCHIVE)
        }
        (_, "pem" | "key" | "crt" | "cer" | "pub" | "lock") => {
            (lucide::file_lock(), theme::TEXT_FAINT)
        }
        (_, "md" | "mdx" | "txt" | "log" | "rst") => (lucide::file_text(), theme::TEXT_MUTED),
        _ if entry.is_symlink => (lucide::file_symlink(), theme::TEXT_MUTED),
        _ => (lucide::file(), theme::TEXT_FAINT),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use iced::Size;

    use crate::models::{FileEntry, FileKind};

    fn entry(name: &str, path: &str, kind: FileKind) -> FileEntry {
        FileEntry {
            name: name.into(),
            path: path.into(),
            kind,
            is_symlink: false,
            size: 0,
            permissions: "-rw-r--r--".into(),
            owner: Some("root".into()),
            modified: None,
        }
    }

    fn files() -> Vec<FileEntry> {
        vec![
            entry("README.md", "/srv/app/README.md", FileKind::File),
            entry("src", "/srv/app/src", FileKind::Directory),
            entry("main.rs", "/srv/app/src/main.rs", FileKind::File),
            entry("assets", "/srv/app/assets", FileKind::Directory),
        ]
    }

    fn paths(rows: &[super::Row<'_>]) -> Vec<(String, u16)> {
        rows.iter()
            .map(|row| (row.entry.path.clone(), row.depth))
            .collect()
    }

    #[test]
    fn collapsed_tree_lists_directories_first() {
        let files = files();
        let rows = super::visible_rows(&files, &HashSet::new());

        assert_eq!(
            paths(&rows),
            vec![
                ("/srv/app/assets".to_string(), 0),
                ("/srv/app/src".to_string(), 0),
                ("/srv/app/README.md".to_string(), 0),
            ]
        );
    }

    #[test]
    fn expanded_folders_show_nested_children() {
        let files = files();
        let expanded = HashSet::from(["/srv/app/src".to_string()]);
        let rows = super::visible_rows(&files, &expanded);

        assert_eq!(
            paths(&rows),
            vec![
                ("/srv/app/assets".to_string(), 0),
                ("/srv/app/src".to_string(), 0),
                ("/srv/app/src/main.rs".to_string(), 1),
                ("/srv/app/README.md".to_string(), 0),
            ]
        );
    }

    #[test]
    fn context_menu_flips_above_rows_near_the_bottom() {
        let viewport = Size::new(272.0, 300.0);
        let below = super::menu_position(viewport, 0, 0, 0.0, 200.0);
        let above = super::menu_position(viewport, 9, 0, 0.0, 200.0);

        assert!(below.y > 0.0 && below.y < 40.0);
        assert!(above.y + 200.0 <= super::TREE_PADDING + 9.0 * 27.0);
    }

    #[test]
    fn directories_offer_open_and_files_offer_editor() {
        let file = entry("a.txt", "/a.txt", FileKind::File);
        let folder = entry("logs", "/logs", FileKind::Directory);

        assert!(
            super::menu_items(&file)
                .iter()
                .any(|item| item.label == "Open in editor")
        );
        assert!(
            super::menu_items(&folder)
                .iter()
                .any(|item| item.label == "Open")
        );
        assert!(
            super::menu_items(&folder)
                .last()
                .is_some_and(|item| item.danger)
        );
    }
}
