//! Small reusable building blocks for the views.

use iced::widget::{Space, Text, button, column, container, row, text, tooltip};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::app::messages::Message;

use super::theme;

pub use iced_fonts::lucide;

/// Sizes an icon glyph and tints it.
pub fn icon<'a>(glyph: Text<'a>, size: f32, color: Color) -> Text<'a> {
    glyph
        .size(size)
        .color(color)
        .line_height(iced::widget::text::LineHeight::Relative(1.0))
}

/// Small uppercase section / field label.
pub fn caption<'a>(label: impl text::IntoFragment<'a>) -> Text<'a> {
    text(label)
        .size(theme::TEXT_XS)
        .font(theme::SEMIBOLD)
        .color(theme::TEXT_FAINT)
}

pub fn field<'a>(label: &'a str, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![
        text(label)
            .size(theme::TEXT_SM)
            .font(theme::MEDIUM)
            .color(theme::TEXT_MUTED),
        content.into()
    ]
    .spacing(6)
    .width(Length::Fill)
    .into()
}

/// A square toolbar button showing an icon, with a tooltip.
pub fn icon_button<'a>(
    glyph: Text<'a>,
    tip: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let color = if on_press.is_some() {
        theme::TEXT_MUTED
    } else {
        theme::TEXT_DISABLED
    };
    let control = button(
        container(icon(glyph, 15.0, color))
            .center_x(Length::Fixed(18.0))
            .center_y(Length::Fixed(18.0)),
    )
    .padding(5)
    .on_press_maybe(on_press)
    .style(theme::toolbar_button);

    with_tooltip(control, tip)
}

pub fn with_tooltip<'a>(
    content: impl Into<Element<'a, Message>>,
    tip: &'a str,
) -> Element<'a, Message> {
    tooltip(
        content,
        container(text(tip).size(theme::TEXT_SM).color(theme::TEXT))
            .padding([5, 8])
            .style(theme::tooltip),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

/// A button with a leading icon and a label.
pub fn labeled_button<'a>(
    glyph: Text<'a>,
    label: &'a str,
    on_press: Option<Message>,
    style: impl Fn(&iced::Theme, button::Status) -> button::Style + 'a,
) -> iced::widget::Button<'a, Message> {
    button(
        row![
            glyph
                .size(14)
                .line_height(iced::widget::text::LineHeight::Relative(1.0)),
            text(label).size(theme::TEXT_MD).font(theme::MEDIUM)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([7, 12])
    .on_press_maybe(on_press)
    .style(style)
}

pub fn text_button<'a>(
    label: &'a str,
    on_press: Option<Message>,
    style: impl Fn(&iced::Theme, button::Status) -> button::Style + 'a,
) -> iced::widget::Button<'a, Message> {
    button(text(label).size(theme::TEXT_MD).font(theme::MEDIUM))
        .padding([7, 14])
        .on_press_maybe(on_press)
        .style(style)
}

pub fn badge<'a>(label: impl text::IntoFragment<'a>, color: Color) -> Element<'a, Message> {
    container(
        text(label)
            .size(theme::TEXT_XS)
            .font(theme::MEDIUM)
            .color(color),
    )
    .padding([2, 7])
    .style(theme::fill(theme::with_alpha(color, 0.13), 999.0))
    .into()
}

pub fn status_dot<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new().width(7).height(7))
        .style(theme::fill(color, 4.0))
        .into()
}

pub fn horizontal_divider<'a>() -> Element<'a, Message> {
    container(Space::new().width(Length::Fill).height(1))
        .style(theme::divider)
        .into()
}

pub fn vertical_divider<'a>() -> Element<'a, Message> {
    container(Space::new().width(1).height(Length::Fill))
        .style(theme::divider)
        .into()
}

/// A keyboard shortcut hint, e.g. `⌘S`.
pub fn kbd<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    container(
        text(label)
            .size(theme::TEXT_XS)
            .font(theme::MONO_FONT)
            .color(theme::TEXT_MUTED),
    )
    .padding(Padding::from([1, 5]))
    .style(theme::outlined(
        theme::BG_ELEVATED,
        theme::BORDER_STRONG,
        4.0,
    ))
    .into()
}

/// Platform-appropriate label for the primary shortcut modifier.
pub fn command_key(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("\u{2318}{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

/// A deterministic accent color for an avatar, derived from a label.
pub fn avatar_color(seed: &str) -> Color {
    const COLORS: [Color; 6] = [
        Color::from_rgb8(0x4f, 0x8c, 0xff),
        Color::from_rgb8(0x34, 0xd3, 0x99),
        Color::from_rgb8(0xc0, 0x84, 0xfc),
        Color::from_rgb8(0xfb, 0xbf, 0x24),
        Color::from_rgb8(0x22, 0xd3, 0xee),
        Color::from_rgb8(0xf4, 0x72, 0xb6),
    ];
    let hash = seed.bytes().fold(0_u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u32)
    });
    COLORS[hash as usize % COLORS.len()]
}

pub fn initials(label: &str) -> String {
    let mut letters = label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>();
    if letters.is_empty() {
        letters.push('?');
    }
    letters.to_uppercase()
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit_index = 0;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    format!("{value:.1} {}", UNITS[unit_index])
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats_byte_sizes() {
        assert_eq!(super::format_bytes(512), "512 B");
        assert_eq!(super::format_bytes(1536), "1.5 KB");
        assert_eq!(super::format_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn derives_initials_from_labels() {
        assert_eq!(super::initials("prod db"), "PD");
        assert_eq!(super::initials("deploy@web-01"), "DW");
        assert_eq!(super::initials("---"), "?");
    }
}
