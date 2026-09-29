use iced::font::{Style as FontStyle, Weight};
use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{
    Space, button, column, container, lazy, mouse_area, rich_text, row, sensor, span, stack, text,
};
use iced::{Alignment, Background, Color, Element, Font, Length, Size};

use crate::app::messages::Message;
use crate::app::state::AppState;
use crate::ssh::terminal::TerminalStyleSpan;

use super::components::{self, icon, lucide};
use super::theme;

pub const FONT_SIZE: f32 = 13.0;
/// JetBrains Mono advances exactly 0.6em per cell.
pub const CELL_WIDTH: f32 = FONT_SIZE * 0.6;
pub const LINE_HEIGHT: f32 = 17.0;
const PADDING_X: f32 = 14.0;
const PADDING_Y: f32 = 10.0;

/// Terminal grid (columns, rows) that fits in a viewport of `size`.
pub fn grid_size(size: Size) -> (u16, u16) {
    let cols = ((size.width - 2.0 * PADDING_X) / CELL_WIDTH).floor();
    let rows = ((size.height - 2.0 * PADDING_Y) / LINE_HEIGHT).floor();
    (
        cols.clamp(20.0, 500.0) as u16,
        rows.clamp(5.0, 300.0) as u16,
    )
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let terminal = &state.workspace.terminal;
    let show_cursor = state.workspace.terminal_cursor_visible && state.modal.is_none();

    // Rebuilding and re-shaping the grid is the most expensive part of the
    // UI, so only do it when the terminal actually changed.
    let screen = lazy(
        (terminal.generation(), show_cursor),
        move |&(_, show_cursor)| -> Element<'static, Message> {
            rich_text(terminal_spans(
                state
                    .workspace
                    .terminal
                    .styled_spans_with_cursor(show_cursor),
            ))
            .font(theme::MONO_FONT)
            .size(FONT_SIZE)
            .line_height(LineHeight::Absolute(LINE_HEIGHT.into()))
            .wrapping(Wrapping::None)
            .into()
        },
    );

    let viewport = container(screen)
        .padding([PADDING_Y, PADDING_X])
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(theme::terminal);

    let viewport = sensor(
        mouse_area(viewport)
            .on_scroll(Message::TerminalScrolled)
            .interaction(iced::mouse::Interaction::Text),
    )
    .on_show(Message::TerminalViewportResized)
    .on_resize(Message::TerminalViewportResized);

    let offset = terminal.scroll_offset();
    if offset == 0 {
        return viewport.into();
    }

    let jump = button(
        row![
            icon(lucide::arrow_down_to_line(), 13.0, theme::TEXT),
            text(format!("{offset} lines up \u{00b7} Jump to latest"))
                .size(theme::TEXT_SM)
                .font(theme::MEDIUM),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .on_press(Message::TerminalScrollToBottom)
    .style(theme::secondary_button);

    stack![
        viewport,
        container(container(jump).style(theme::popover))
            .align_right(Length::Fill)
            .align_bottom(Length::Fill)
            .padding(16),
    ]
    .into()
}

/// Status bar content shown while the terminal tab is active.
pub fn status_items(state: &AppState) -> Element<'_, Message> {
    let (rows, cols) = state.workspace.terminal.size();

    let actions = row![
        components::icon_button(
            lucide::copy(),
            "Copy screen",
            Some(Message::CopyTerminalOutput)
        ),
        components::icon_button(
            lucide::clipboard_paste(),
            "Paste",
            Some(Message::PasteTerminalInput)
        ),
        components::icon_button(
            lucide::eraser(),
            "Clear screen",
            Some(Message::ClearTerminal)
        ),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    row![
        text(format!("{cols}\u{00d7}{rows}"))
            .size(theme::TEXT_XS)
            .font(theme::MONO_FONT)
            .color(theme::TEXT_FAINT),
        text("xterm-256color")
            .size(theme::TEXT_XS)
            .color(theme::TEXT_FAINT),
        text("UTF-8").size(theme::TEXT_XS).color(theme::TEXT_FAINT),
        Space::new().width(Length::Fill),
        text(format!(
            "{} copy \u{00b7} {} paste",
            copy_shortcut(),
            paste_shortcut()
        ))
        .size(theme::TEXT_XS)
        .color(theme::TEXT_DISABLED),
        actions,
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .into()
}

fn copy_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "\u{2318}C"
    } else {
        "Ctrl+Shift+C"
    }
}

fn paste_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "\u{2318}V"
    } else {
        "Ctrl+Shift+V"
    }
}

fn terminal_spans(segments: Vec<TerminalStyleSpan>) -> Vec<iced::widget::text::Span<'static>> {
    let default_foreground = rgb(crate::ssh::terminal::DEFAULT_FOREGROUND);

    segments
        .into_iter()
        .map(|segment| {
            let font = terminal_font(&segment);
            let mut foreground = segment.foreground.map(rgb).unwrap_or(default_foreground);
            if segment.dim {
                foreground.a = 0.6;
            }

            let mut text_span = span(segment.text).font(font).color(foreground);

            if segment.underline {
                text_span = text_span.underline(true);
            }

            if let Some(background) = segment.background {
                text_span = text_span.background(Background::Color(rgb(background)));
            }

            text_span
        })
        .collect()
}

fn rgb((red, green, blue): (u8, u8, u8)) -> Color {
    Color::from_rgb8(red, green, blue)
}

fn terminal_font(segment: &TerminalStyleSpan) -> Font {
    Font {
        weight: if segment.bold {
            Weight::Bold
        } else {
            Weight::Normal
        },
        style: if segment.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        ..theme::MONO_FONT
    }
}

/// Placeholder shown when the terminal is not available.
pub fn empty() -> Element<'static, Message> {
    container(
        column![
            icon(lucide::terminal(), 22.0, theme::TEXT_DISABLED),
            text("No active session")
                .size(theme::TEXT_MD)
                .color(theme::TEXT_FAINT),
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .center(Length::Fill)
    .style(theme::terminal)
    .into()
}

#[cfg(test)]
mod tests {
    use iced::Size;

    #[test]
    fn grid_size_accounts_for_padding_and_cell_metrics() {
        let (cols, rows) = super::grid_size(Size::new(
            80.0 * super::CELL_WIDTH + 28.5,
            24.0 * super::LINE_HEIGHT + 20.5,
        ));
        assert_eq!((cols, rows), (80, 24));
    }

    #[test]
    fn grid_size_is_clamped_for_tiny_viewports() {
        assert_eq!(super::grid_size(Size::new(10.0, 10.0)), (20, 5));
    }
}
