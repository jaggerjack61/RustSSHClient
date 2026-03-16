use chrono::{DateTime, Utc};
use iced::widget::{button, checkbox, container, scrollable, text, text_editor, text_input};
use iced::{Background, Border, Color, Element, Length, Shadow, Theme, Vector};

use crate::app::messages::Message;
use crate::app::state::{AppState, NotificationLevel};

const TERMINAL_TOP_BAR_HEIGHT: f32 = 56.0;
const TERMINAL_FOOTER_HEIGHT: f32 = 40.0;

pub fn terminal_top_bar_height() -> f32 {
    TERMINAL_TOP_BAR_HEIGHT
}

pub fn terminal_footer_height() -> f32 {
    TERMINAL_FOOTER_HEIGHT
}

pub fn workspace_header_height() -> f32 {
    terminal_top_bar_height()
}

pub fn workspace_footer_height() -> f32 {
    terminal_footer_height()
}

// ---------------------------------------------------------------------------
// Color palette – matches the HTML / Tailwind mockup
// ---------------------------------------------------------------------------

pub fn bg_dark() -> Color {
    Color::from_rgb8(0x0f, 0x17, 0x2a)
}

pub fn glass_bg() -> Color {
    Color::from_rgba8(0x0f, 0x17, 0x2a, 0.85)
}

pub fn sidebar_bg() -> Color {
    Color::from_rgba8(0x00, 0x00, 0x00, 0.35)
}

pub fn input_bg() -> Color {
    Color::from_rgba8(0x00, 0x00, 0x00, 0.30)
}

pub fn primary() -> Color {
    Color::from_rgb8(0x3b, 0x82, 0xf6)
}

pub fn primary_hover() -> Color {
    Color::from_rgb8(0x2b, 0x6c, 0xd6)
}

pub fn accent() -> Color {
    Color::from_rgb8(0x10, 0xb9, 0x81)
}

pub fn text_white() -> Color {
    Color::WHITE
}

pub fn text_slate_300() -> Color {
    Color::from_rgb8(0xcb, 0xd5, 0xe1)
}

pub fn text_slate_400() -> Color {
    Color::from_rgb8(0x94, 0xa3, 0xb8)
}

pub fn text_slate_500() -> Color {
    Color::from_rgb8(0x64, 0x74, 0x8b)
}

pub fn text_slate_600() -> Color {
    Color::from_rgb8(0x47, 0x55, 0x69)
}

pub fn border_subtle() -> Color {
    Color::from_rgba8(0xff, 0xff, 0xff, 0.08)
}

pub fn highlight_bg() -> Color {
    Color::from_rgba8(0xff, 0xff, 0xff, 0.08)
}

pub fn hover_bg() -> Color {
    Color::from_rgba8(0xff, 0xff, 0xff, 0.05)
}

pub fn terminal_bg() -> Color {
    Color::from_rgba8(0x00, 0x00, 0x00, 0.30)
}

pub fn red_400() -> Color {
    Color::from_rgb8(0xf8, 0x71, 0x71)
}

pub fn blue_400() -> Color {
    Color::from_rgb8(0x60, 0xa5, 0xfa)
}

pub fn emerald_400() -> Color {
    Color::from_rgb8(0x34, 0xd3, 0x99)
}

pub fn orange_400() -> Color {
    Color::from_rgb8(0xfb, 0x92, 0x3c)
}

// ---------------------------------------------------------------------------
// Container styles
// ---------------------------------------------------------------------------

pub fn root_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(bg_dark())),
        ..Default::default()
    }
}

pub fn sidebar_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(sidebar_bg())),
        ..Default::default()
    }
}

pub fn sidebar_header(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0xff, 0xff, 0xff, 0.015))),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 0.into(),
        },
        ..Default::default()
    }
}

pub fn glass_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(glass_bg())),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 18.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.4),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 28.0,
        },
        ..Default::default()
    }
}

pub fn main_area(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.10))),
        ..Default::default()
    }
}

pub fn terminal_area(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(terminal_bg())),
        ..Default::default()
    }
}

pub fn terminal_header(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.50))),
        border: Border {
            color: Color::from_rgba8(0xff, 0xff, 0xff, 0.06),
            width: 1.0,
            radius: 0.into(),
        },
        ..Default::default()
    }
}

pub fn status_bar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.25))),
        border: Border {
            color: Color::from_rgba8(0xff, 0xff, 0xff, 0.06),
            width: 1.0,
            radius: 0.into(),
        },
        ..Default::default()
    }
}

pub fn title_bar(_theme: &Theme) -> container::Style {
    container::Style {
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 0.into(),
        },
        ..Default::default()
    }
}

pub fn explorer_sidebar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.22))),
        ..Default::default()
    }
}

pub fn details_panel(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.14))),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 12.into(),
        },
        ..Default::default()
    }
}

pub fn context_menu_panel(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x0f, 0x17, 0x2a, 0.96))),
        border: Border {
            color: Color::from_rgba8(0xff, 0xff, 0xff, 0.12),
            width: 1.0,
            radius: 10.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.35),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 18.0,
        },
        ..Default::default()
    }
}

pub fn modal_backdrop(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x02, 0x06, 0x23, 0.72))),
        ..Default::default()
    }
}

pub fn settings_modal_panel(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x08, 0x12, 0x25, 0.96))),
        border: Border {
            color: Color::from_rgba8(0x7d, 0xb7, 0xff, 0.30),
            width: 1.0,
            radius: 24.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.45),
            offset: Vector::new(0.0, 14.0),
            blur_radius: 42.0,
        },
        ..Default::default()
    }
}

pub fn settings_modal_section(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0xff, 0xff, 0xff, 0.03))),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 18.into(),
        },
        ..Default::default()
    }
}

pub fn notification_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x1e, 0x29, 0x3b, 0.95))),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 8.into(),
        },
        ..Default::default()
    }
}

pub fn tooltip_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x0f, 0x17, 0x2a, 0.98))),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 8.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.35),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    }
}

pub fn app_window(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(glass_bg())),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 14.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.5),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 32.0,
        },
        ..Default::default()
    }
}

pub fn tab_active(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0xff, 0xff, 0xff, 0.05))),
        border: Border {
            color: primary(),
            width: 0.0,
            radius: 0.into(),
        },
        ..Default::default()
    }
}

pub fn workspace_tab_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0xff, 0xff, 0xff, 0.025))),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 10.into(),
        },
        ..Default::default()
    }
}

pub fn workspace_tab_active_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0xff, 0xff, 0xff, 0.065))),
        border: Border {
            color: Color::from_rgba8(0x3b, 0x82, 0xf6, 0.72),
            width: 1.0,
            radius: 10.into(),
        },
        ..Default::default()
    }
}

pub fn editor_header(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.20))),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 0.into(),
        },
        ..Default::default()
    }
}

pub fn dot_indicator(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: 3.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Button styles
// ---------------------------------------------------------------------------

pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => primary_hover(),
        _ => primary(),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: 12.into(),
            ..Default::default()
        },
        shadow: Shadow {
            color: Color::from_rgba8(0x3b, 0x82, 0xf6, 0.25),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        snap: false,
    }
}

pub fn ghost_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => hover_bg(),
        _ => Color::TRANSPARENT,
    };
    let text_color = match status {
        button::Status::Hovered | button::Status::Pressed => Color::WHITE,
        _ => text_slate_400(),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn host_card_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => hover_bg(),
        _ => Color::TRANSPARENT,
    };
    let border_color = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgba8(0xff, 0xff, 0xff, 0.07),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: text_slate_400(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 12.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn host_card_active(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(highlight_bg())),
        text_color: Color::WHITE,
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 12.into(),
        },
        ..Default::default()
    }
}

pub fn sidebar_footer_button(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => Color::WHITE,
        _ => text_slate_400(),
    };
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => hover_bg(),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: color,
        border: Border {
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn link_button(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => primary(),
        _ => text_slate_500(),
    };
    button::Style {
        background: None,
        text_color: color,
        border: Border::default(),
        ..Default::default()
    }
}

pub fn accent_link_button(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgb8(0x2b, 0x6c, 0xd6),
        _ => primary(),
    };
    button::Style {
        background: None,
        text_color: color,
        border: Border::default(),
        ..Default::default()
    }
}

pub fn file_entry_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => hover_bg(),
        _ => Color::TRANSPARENT,
    };
    let border_color = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgba8(0xff, 0xff, 0xff, 0.06),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: text_slate_400(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn file_entry_active(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(highlight_bg())),
        text_color: Color::WHITE,
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn status_bar_button(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => Color::WHITE,
        _ => text_slate_500(),
    };
    button::Style {
        background: None,
        text_color: color,
        border: Border::default(),
        ..Default::default()
    }
}

pub fn new_session_button(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => blue_400(),
        _ => primary(),
    };
    button::Style {
        background: None,
        text_color: color,
        border: Border::default(),
        ..Default::default()
    }
}

pub fn disconnect_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Color::from_rgba8(0xf8, 0x71, 0x71, 0.2)
        }
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: red_400(),
        border: Border {
            color: red_400(),
            width: 1.0,
            radius: 8.into(),
        },
        ..Default::default()
    }
}

pub fn small_action_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => hover_bg(),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: text_slate_400(),
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 6.into(),
        },
        ..Default::default()
    }
}

pub fn context_menu_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgba8(0xff, 0xff, 0xff, 0.08),
        _ => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn context_menu_danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgba8(0xf8, 0x71, 0x71, 0.16),
        _ => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: red_400(),
        border: Border {
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn workspace_tab_button(_theme: &Theme, status: button::Status) -> button::Style {
    let text_color = match status {
        button::Status::Hovered | button::Status::Pressed => Color::WHITE,
        _ => text_slate_400(),
    };

    button::Style {
        background: None,
        text_color,
        border: Border::default(),
        ..Default::default()
    }
}

pub fn workspace_tab_active_button(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: Color::WHITE,
        border: Border::default(),
        ..Default::default()
    }
}

pub fn workspace_tab_close_button(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => red_400(),
        _ => text_slate_500(),
    };
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => hover_bg(),
        _ => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: color,
        border: Border {
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn settings_option_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgba8(0xff, 0xff, 0xff, 0.08),
        _ => Color::from_rgba8(0xff, 0xff, 0xff, 0.03),
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            color: border_subtle(),
            width: 1.0,
            radius: 16.into(),
        },
        ..Default::default()
    }
}

pub fn settings_option_active_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgba8(0x3b, 0x82, 0xf6, 0.34),
        _ => Color::from_rgba8(0x3b, 0x82, 0xf6, 0.24),
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            color: Color::from_rgba8(0x7d, 0xb7, 0xff, 0.60),
            width: 1.0,
            radius: 16.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0x3b, 0x82, 0xf6, 0.20),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    }
}

pub fn window_control(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => Color::WHITE,
        _ => text_slate_500(),
    };
    button::Style {
        background: None,
        text_color: color,
        border: Border::default(),
        ..Default::default()
    }
}

pub fn close_control(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => red_400(),
        _ => text_slate_500(),
    };
    button::Style {
        background: None,
        text_color: color,
        border: Border::default(),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Text input styles
// ---------------------------------------------------------------------------

pub fn dark_input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { is_hovered: _ } => primary(),
        text_input::Status::Hovered => Color::from_rgba8(0xff, 0xff, 0xff, 0.12),
        _ => border_subtle(),
    };
    text_input::Style {
        background: Background::Color(input_bg()),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 12.into(),
        },
        icon: text_slate_500(),
        placeholder: text_slate_600(),
        value: Color::WHITE,
        selection: Color::from_rgba8(0x3b, 0x82, 0xf6, 0.3),
    }
}

pub fn dark_text_editor(_theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let border_color = match status {
        text_editor::Status::Focused { .. } => primary(),
        text_editor::Status::Hovered => Color::from_rgba8(0xff, 0xff, 0xff, 0.12),
        text_editor::Status::Disabled => text_slate_600(),
        text_editor::Status::Active => border_subtle(),
    };

    text_editor::Style {
        background: Background::Color(input_bg()),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 12.into(),
        },
        placeholder: text_slate_600(),
        value: text_slate_300(),
        selection: Color::from_rgba8(0x3b, 0x82, 0xf6, 0.28),
    }
}

pub fn terminal_input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { is_hovered: _ } => primary(),
        _ => Color::from_rgba8(0xff, 0xff, 0xff, 0.05),
    };
    text_input::Style {
        background: Background::Color(Color::from_rgba8(0, 0, 0, 0.30)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 0.into(),
        },
        icon: text_slate_500(),
        placeholder: text_slate_600(),
        value: text_slate_300(),
        selection: Color::from_rgba8(0x3b, 0x82, 0xf6, 0.3),
    }
}

// ---------------------------------------------------------------------------
// Scrollable style
// ---------------------------------------------------------------------------

pub fn dark_scrollable(_theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let scrollbar_color = match status {
        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => {
            Color::from_rgba8(0xff, 0xff, 0xff, 0.2)
        }
        _ => Color::from_rgba8(0xff, 0xff, 0xff, 0.08),
    };
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(scrollbar_color),
            border: Border {
                radius: 4.into(),
                ..Default::default()
            },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail.clone(),
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            shadow: Shadow::default(),
            icon: text_slate_500(),
        },
    }
}

// ---------------------------------------------------------------------------
// Checkbox style
// ---------------------------------------------------------------------------

pub fn dark_checkbox(_theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    match status {
        checkbox::Status::Active { is_checked } | checkbox::Status::Hovered { is_checked } => {
            checkbox::Style {
                background: Background::Color(if is_checked {
                    primary()
                } else {
                    Color::from_rgba8(0, 0, 0, 0.4)
                }),
                icon_color: Color::WHITE,
                border: Border {
                    color: if is_checked {
                        primary()
                    } else {
                        border_subtle()
                    },
                    width: 1.0,
                    radius: 4.into(),
                },
                text_color: Some(text_slate_400()),
            }
        }
        checkbox::Status::Disabled { is_checked: _ } => checkbox::Style {
            background: Background::Color(Color::from_rgba8(0, 0, 0, 0.2)),
            icon_color: text_slate_600(),
            border: Border {
                color: border_subtle(),
                width: 1.0,
                radius: 4.into(),
            },
            text_color: Some(text_slate_600()),
        },
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_index = 0;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    format!("{value:.1} {}", UNITS[unit_index])
}

pub fn format_timestamp(value: Option<DateTime<Utc>>) -> String {
    value
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Unknown".into())
}

pub fn auth_badge(
    is_selected: bool,
    label: &str,
    message: Message,
) -> iced::widget::Button<'_, Message> {
    let btn = button(
        text(label)
            .size(13)
            .color(if is_selected {
                Color::WHITE
            } else {
                text_slate_400()
            }),
    )
    .padding([8, 16])
    .width(Length::Fill);

    if is_selected {
        btn.on_press(message).style(|_theme, _status| button::Style {
            background: Some(Background::Color(primary())),
            text_color: Color::WHITE,
            border: Border {
                radius: 10.into(),
                ..Default::default()
            },
            ..Default::default()
        })
    } else {
        btn.on_press(message).style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => hover_bg(),
                _ => Color::from_rgba8(0, 0, 0, 0.20),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: text_slate_400(),
                border: Border {
                    color: border_subtle(),
                    width: 1.0,
                    radius: 8.into(),
                },
                ..Default::default()
            }
        })
    }
}

pub fn notifications(state: &AppState) -> Element<'_, Message> {
    use iced::widget::{column, row, Space};

    if state.notifications.is_empty() {
        return Space::new().into();
    }

    let content = state.notifications.iter().enumerate().fold(
        column![].spacing(8),
        |column, (index, item)| {
            let (label, color) = match item.level {
                NotificationLevel::Info => ("INFO", blue_400()),
                NotificationLevel::Success => ("OK", accent()),
                NotificationLevel::Error => ("ERR", red_400()),
            };

            column.push(
                container(
                    row![
                        text(format!("{label}")).size(11).color(color),
                        text(&item.message).size(13).color(text_slate_300()),
                        Space::new().width(Length::Fill),
                        button(text("x").size(11).color(text_slate_500()))
                            .on_press(Message::DismissNotification(index))
                            .padding([2, 8])
                            .style(ghost_button),
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::Center),
                )
                .padding([8, 16])
                .width(Length::Fill)
                .style(notification_container),
            )
        },
    );

    container(
        column![
            Space::new().height(Length::Fill),
            container(content).max_width(500),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([16, 16]),
    )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
