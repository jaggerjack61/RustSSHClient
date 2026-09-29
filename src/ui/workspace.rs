use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};

use crate::app::messages::Message;
use crate::app::state::AppState;
use crate::models::{EditorDocument, WorkspaceTab};

use super::components::{self, icon, lucide};
use super::{editor, file_tree, terminal, theme};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let panel: Element<'_, Message> = match &state.workspace.active_tab {
        WorkspaceTab::Terminal => terminal::view(state),
        WorkspaceTab::Editor(_) => editor::view(state),
    };

    let main = column![tab_bar(state), components::horizontal_divider(), panel]
        .width(Length::Fill)
        .height(Length::Fill);

    column![
        row![file_tree::view(state), components::vertical_divider(), main].height(Length::Fill),
        components::horizontal_divider(),
        status_bar(state),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn tab_bar(state: &AppState) -> Element<'_, Message> {
    let workspace = &state.workspace;
    let terminal_active = matches!(workspace.active_tab, WorkspaceTab::Terminal);

    let mut tabs = row![tab(
        lucide::terminal(),
        "Terminal",
        terminal_active,
        false,
        Message::ActivateTerminalTab,
        None,
    )]
    .spacing(4)
    .align_y(Alignment::Center);

    for document in &workspace.editor_tabs {
        let active =
            matches!(&workspace.active_tab, WorkspaceTab::Editor(path) if *path == document.path);
        tabs = tabs.push(editor_tab(document, active));
    }

    let tabs = scrollable(tabs)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(2).scroller_width(2),
        ))
        .style(theme::scrollbar)
        .width(Length::Fill);

    let user = state.login.username.trim();
    let session = row![
        components::status_dot(theme::SUCCESS),
        text(if user.is_empty() {
            workspace.connected_peer.clone()
        } else {
            format!("{user}@{}", workspace.connected_peer)
        })
        .size(theme::TEXT_SM)
        .font(theme::MEDIUM)
        .color(theme::TEXT)
        .wrapping(text::Wrapping::None),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let disconnect = components::labeled_button(
        lucide::log_out(),
        "Disconnect",
        Some(Message::DisconnectPressed),
        theme::outline_danger_button,
    )
    .padding([4, 10]);

    container(
        row![tabs, session, disconnect]
            .spacing(14)
            .align_y(Alignment::Center),
    )
    .padding([0, 10])
    .height(theme::HEADER_HEIGHT)
    .center_y(theme::HEADER_HEIGHT)
    .width(Length::Fill)
    .style(theme::bar)
    .into()
}

fn editor_tab(document: &EditorDocument, active: bool) -> Element<'_, Message> {
    let glyph = if document.is_loading {
        lucide::loader_circle()
    } else {
        lucide::file_text()
    };

    tab(
        glyph,
        &document.title,
        active,
        document.is_dirty,
        Message::ActivateEditorTab(document.path.clone()),
        Some(Message::CloseEditorTab(document.path.clone())),
    )
}

fn tab<'a>(
    glyph: text::Text<'a>,
    label: &'a str,
    active: bool,
    dirty: bool,
    on_press: Message,
    on_close: Option<Message>,
) -> Element<'a, Message> {
    let color = if active {
        theme::TEXT
    } else {
        theme::TEXT_FAINT
    };
    let mut content = row![
        icon(glyph, 13.0, if active { theme::ACCENT_TEXT } else { color }),
        text(label)
            .size(theme::TEXT_MD)
            .font(if active {
                theme::MEDIUM
            } else {
                theme::UI_FONT
            })
            .wrapping(text::Wrapping::None),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    if dirty {
        content = content.push(components::status_dot(theme::WARNING));
    }

    if let Some(on_close) = on_close {
        content = content.push(
            button(icon(lucide::x(), 12.0, theme::TEXT_FAINT))
                .padding(3)
                .on_press(on_close)
                .style(theme::tab_close),
        );
    } else {
        content = content.push(Space::new().width(2));
    }

    let tab = button(content)
        .padding([6, 10])
        .on_press(on_press)
        .style(theme::tab(active));

    // Active tabs get an accent underline.
    column![
        tab,
        container(Space::new().height(2))
            .width(Length::Fill)
            .style(theme::fill(
                if active {
                    theme::ACCENT
                } else {
                    iced::Color::TRANSPARENT
                },
                1.0
            )),
    ]
    .width(Length::Shrink)
    .into()
}

fn status_bar(state: &AppState) -> Element<'_, Message> {
    let workspace = &state.workspace;
    let latency = workspace
        .latency_ms
        .map(|ms| format!("{ms} ms handshake"))
        .unwrap_or_default();

    let connection = row![
        components::status_dot(theme::SUCCESS),
        text("Connected")
            .size(theme::TEXT_XS)
            .color(theme::TEXT_MUTED),
        text(latency).size(theme::TEXT_XS).color(theme::TEXT_FAINT),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let details: Element<'_, Message> = match workspace.active_tab {
        WorkspaceTab::Terminal => terminal::status_items(state),
        WorkspaceTab::Editor(_) => editor::status_items(state),
    };

    container(
        row![
            container(connection).width(theme::EXPLORER_WIDTH - 12.0),
            container(details).width(Length::Fill),
        ]
        .align_y(Alignment::Center),
    )
    .padding([0, 12])
    .height(theme::STATUS_BAR_HEIGHT)
    .center_y(theme::STATUS_BAR_HEIGHT)
    .width(Length::Fill)
    .style(theme::bar)
    .into()
}
