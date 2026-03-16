use iced::font::{Style as FontStyle, Weight};
use iced::widget::{button, column, container, progress_bar, rich_text, row, scrollable, span, text, Space};
use iced::{Background, Color, Element, Font, Length};

use crate::app::messages::Message;
use crate::app::state::AppState;
use crate::models::WorkspaceTab;

use super::{editor, file_tree, styles};

pub fn workspace_view(state: &AppState) -> Element<'_, Message> {
    let tab_bar = workspace_tab_bar(state);

    let panel_body: Element<'_, Message> = match &state.workspace.active_tab {
        WorkspaceTab::Terminal => terminal_panel(state),
        WorkspaceTab::Editor(_) => editor::view(state),
    };

    let content = column![row![
        file_tree::view(state),
        column![tab_bar, panel_body]
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)]
    .height(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::app_window)
        .into()
}

fn workspace_tab_bar(state: &AppState) -> Element<'_, Message> {
    let bash_active = matches!(state.workspace.active_tab, WorkspaceTab::Terminal);
    let connected_peer = if state.workspace.connected_peer.trim().is_empty() {
        "Awaiting session".to_string()
    } else {
        state.workspace.connected_peer.clone()
    };
    let bash_tab = container(
        row![button(
            row![
                text(">_").size(11).color(Color::WHITE),
                text("bash").size(12).color(Color::WHITE),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::ActivateTerminalTab)
        .padding([6, 14])
        .style(if bash_active {
            styles::workspace_tab_active_button
        } else {
            styles::workspace_tab_button
        })]
        .align_y(iced::Alignment::Center),
    )
    .style(if bash_active {
        styles::workspace_tab_active_container
    } else {
        styles::workspace_tab_container
    });

    let tabs = state.workspace.editor_tabs.iter().fold(
        row![bash_tab].spacing(8).align_y(iced::Alignment::Center),
        |row, editor_tab| {
            let is_active = matches!(
                &state.workspace.active_tab,
                WorkspaceTab::Editor(path) if path == &editor_tab.path
            );
            let label = if editor_tab.is_loading {
                format!("{}...", editor_tab.title)
            } else if editor_tab.is_dirty {
                format!("*{}", editor_tab.title)
            } else {
                editor_tab.title.clone()
            };

            row.push(
                container(
                    row![
                        button(text(label).size(12).color(Color::WHITE))
                            .on_press(Message::ActivateEditorTab(editor_tab.path.clone()))
                            .padding([6, 14])
                            .style(if is_active {
                                styles::workspace_tab_active_button
                            } else {
                                styles::workspace_tab_button
                            }),
                        button(text("x").size(11).color(styles::text_slate_500()))
                            .on_press(Message::CloseEditorTab(editor_tab.path.clone()))
                            .padding([6, 8])
                            .style(styles::workspace_tab_close_button),
                    ]
                    .spacing(2)
                    .align_y(iced::Alignment::Center),
                )
                .style(if is_active {
                    styles::workspace_tab_active_container
                } else {
                    styles::workspace_tab_container
                }),
            )
        },
    );

    container(
        container(
            row![
                tabs,
                Space::new().width(Length::Fill),
                column![
                    text("ACTIVE SESSION")
                        .size(10)
                        .color(styles::text_slate_500()),
                    text(connected_peer)
                        .size(12)
                        .color(styles::text_slate_300()),
                ]
                .spacing(2)
                .align_x(iced::Alignment::End),
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .center_y(Length::Fill),
    )
    .padding([8, 16])
    .height(Length::Fixed(styles::workspace_header_height()))
    .width(Length::Fill)
    .style(styles::terminal_header)
    .into()
}

fn terminal_panel(state: &AppState) -> Element<'_, Message> {
    let terminal_content = rich_text(terminal_spans(state))
        .size(14)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::None);

    let terminal_output = container(
        scrollable(terminal_content)
            .anchor_bottom()
            .style(styles::dark_scrollable)
            .height(Length::Fill)
            .width(Length::Fill),
    )
    .padding([20, 22])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(styles::terminal_area);

    let transfers: Element<'_, Message> = if state.workspace.transfers.is_empty() {
        Space::new().into()
    } else {
        state
            .workspace
            .transfers
            .iter()
            .fold(column![].spacing(4), |col, transfer| {
                let progress = transfer.percent_complete();
                col.push(
                    row![
                        text(&transfer.label)
                            .size(11)
                            .color(styles::text_slate_400())
                            .width(Length::FillPortion(3)),
                        container(progress_bar(0.0..=1.0, progress).girth(4))
                            .width(Length::FillPortion(2)),
                        text(format!("{:>3}%", (progress * 100.0).round() as u8))
                            .size(11)
                            .color(styles::text_slate_500()),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
            })
            .into()
    };

    let transfers = if state.workspace.transfers.is_empty() {
        transfers
    } else {
        container(
            column![
                text("TRANSFER QUEUE")
                    .size(10)
                    .color(styles::text_slate_500()),
                transfers,
            ]
            .spacing(8),
        )
            .padding([16, 10])
            .width(Length::Fill)
            .into()
    };

    let status_bar = container(
        container(
            row![
                row![
                    container(Space::new().width(5).height(5)).style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(styles::primary())),
                        border: iced::Border {
                            radius: 3.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                    text("Live shell")
                        .size(11)
                        .color(styles::text_slate_300()),
                    text("UTF-8")
                        .size(11)
                        .color(styles::text_slate_500()),
                    text("SSHv2")
                        .size(11)
                        .color(styles::text_slate_500()),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
                Space::new().width(Length::Fill),
                row![
                    button(text("Clear").size(11).color(styles::text_slate_500()))
                        .on_press(Message::ClearTerminal)
                        .padding([4, 8])
                        .style(styles::status_bar_button),
                    button(text("Scrollback").size(11).color(styles::text_slate_500()))
                        .on_press(Message::CopyTerminalOutput)
                        .padding([4, 8])
                        .style(styles::status_bar_button),
                    button(
                        text("New Session")
                            .size(11)
                            .color(styles::primary()),
                    )
                    .on_press(Message::DisconnectPressed)
                    .padding([4, 8])
                    .style(styles::new_session_button),
                ]
                .spacing(12),
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .center_y(Length::Fill),
    )
    .padding([8, 16])
    .height(Length::Fixed(styles::workspace_footer_height()))
    .width(Length::Fill)
    .style(styles::status_bar);

    column![
        terminal_output,
        transfers,
        status_bar,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn terminal_spans(state: &AppState) -> Vec<iced::widget::text::Span<'static>> {
    state
        .workspace
        .terminal
        .styled_spans_with_cursor(state.workspace.terminal_cursor_visible)
        .into_iter()
        .map(|segment| {
            let font = terminal_font(&segment);
            let mut text_span = span(segment.text).font(font);

            let foreground = segment
                .foreground
                .map(|(red, green, blue)| terminal_foreground(red, green, blue, segment.dim))
                .unwrap_or_else(|| {
                    let mut color = styles::text_slate_300();
                    if segment.dim {
                        color.a = 0.72;
                    }
                    color
                });

            text_span = text_span.color(foreground);

            if segment.underline {
                text_span = text_span.underline(true);
            }

            if let Some((red, green, blue)) = segment.background {
                text_span = text_span.background(Background::Color(Color::from_rgb8(
                    red, green, blue,
                )));
            }

            text_span
        })
        .collect()
}

fn terminal_font(segment: &crate::ssh::terminal::TerminalStyleSpan) -> Font {
    Font {
        family: Font::MONOSPACE.family,
        weight: if segment.bold {
            Weight::Bold
        } else {
            Weight::Normal
        },
        stretch: Font::MONOSPACE.stretch,
        style: if segment.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
    }
}

fn terminal_foreground(red: u8, green: u8, blue: u8, dim: bool) -> Color {
    let alpha = if dim { 0.72 } else { 1.0 };
    Color::from_rgba8(red, green, blue, alpha)
}
