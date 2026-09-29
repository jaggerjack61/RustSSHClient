//! Design tokens and widget styles shared by every screen.

use iced::font::Weight;
use iced::theme::Palette;
use iced::widget::{
    button, checkbox, container, pick_list, progress_bar, scrollable, text_editor, text_input,
    toggler,
};
use iced::{Background, Border, Color, Font, Shadow, Theme, Vector};

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

pub const UI_FONT: Font = Font::with_name("Inter");
pub const MONO_FONT: Font = Font::with_name("JetBrains Mono");

pub const FONTS: [&[u8]; 7] = [
    include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/Inter-Bold.ttf"),
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf"),
    iced_fonts::LUCIDE_FONT_BYTES,
];

pub const fn weighted(weight: Weight) -> Font {
    Font { weight, ..UI_FONT }
}

pub const MEDIUM: Font = weighted(Weight::Medium);
pub const SEMIBOLD: Font = weighted(Weight::Semibold);
pub const BOLD: Font = weighted(Weight::Bold);

// ---------------------------------------------------------------------------
// Type scale and metrics
// ---------------------------------------------------------------------------

pub const TEXT_XS: f32 = 11.0;
pub const TEXT_SM: f32 = 12.0;
pub const TEXT_MD: f32 = 13.0;
pub const TEXT_LG: f32 = 15.0;
pub const TEXT_XL: f32 = 20.0;
pub const TEXT_2XL: f32 = 24.0;

pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 8.0;
pub const RADIUS_LG: f32 = 12.0;

pub const HEADER_HEIGHT: f32 = 44.0;
pub const STATUS_BAR_HEIGHT: f32 = 28.0;
pub const EXPLORER_WIDTH: f32 = 272.0;
pub const SIDEBAR_WIDTH: f32 = 288.0;

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

pub const BG_APP: Color = Color::from_rgb8(0x0b, 0x0e, 0x14);
pub const BG_SURFACE: Color = Color::from_rgb8(0x10, 0x14, 0x1c);
pub const BG_ELEVATED: Color = Color::from_rgb8(0x16, 0x1b, 0x25);
pub const BG_OVERLAY: Color = Color::from_rgb8(0x1b, 0x21, 0x2d);
pub const BG_INPUT: Color = Color::from_rgb8(0x0d, 0x11, 0x18);
pub const BG_TERMINAL: Color = Color::from_rgb8(0x0b, 0x0e, 0x14);
pub const BG_HOVER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0.045);
pub const BG_PRESSED: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0.08);

pub const BORDER: Color = Color::from_rgb8(0x21, 0x28, 0x34);
pub const BORDER_STRONG: Color = Color::from_rgb8(0x2d, 0x36, 0x45);

pub const TEXT: Color = Color::from_rgb8(0xe6, 0xea, 0xf2);
pub const TEXT_MUTED: Color = Color::from_rgb8(0x9a, 0xa4, 0xb2);
pub const TEXT_FAINT: Color = Color::from_rgb8(0x6b, 0x75, 0x85);
pub const TEXT_DISABLED: Color = Color::from_rgb8(0x4a, 0x52, 0x60);

pub const ACCENT: Color = Color::from_rgb8(0x4f, 0x8c, 0xff);
pub const ACCENT_HOVER: Color = Color::from_rgb8(0x6a, 0x9d, 0xff);
pub const ACCENT_PRESSED: Color = Color::from_rgb8(0x3e, 0x7b, 0xee);
pub const ACCENT_SOFT: Color = Color::from_rgba8(0x4f, 0x8c, 0xff, 0.14);
pub const ACCENT_TEXT: Color = Color::from_rgb8(0x8d, 0xb5, 0xff);

pub const SUCCESS: Color = Color::from_rgb8(0x34, 0xd3, 0x99);
pub const WARNING: Color = Color::from_rgb8(0xfb, 0xbf, 0x24);
pub const DANGER: Color = Color::from_rgb8(0xf8, 0x71, 0x71);
pub const DANGER_SOFT: Color = Color::from_rgba8(0xf8, 0x71, 0x71, 0.12);
pub const DANGER_STRONG: Color = Color::from_rgb8(0xdc, 0x3f, 0x3f);

pub fn app_theme() -> Theme {
    Theme::custom(
        "RustSSH Dark",
        Palette {
            background: BG_APP,
            text: TEXT,
            primary: ACCENT,
            success: SUCCESS,
            warning: WARNING,
            danger: DANGER,
        },
    )
}

pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

fn border(color: Color, radius: f32) -> Border {
    Border {
        color,
        width: 1.0,
        radius: radius.into(),
    }
}

fn rounded(radius: f32) -> Border {
    Border {
        radius: radius.into(),
        ..Border::default()
    }
}

fn soft_shadow(alpha: f32, y: f32, blur: f32) -> Shadow {
    Shadow {
        color: Color::from_rgba8(0, 0, 0, alpha),
        offset: Vector::new(0.0, y),
        blur_radius: blur,
    }
}

// ---------------------------------------------------------------------------
// Containers
// ---------------------------------------------------------------------------

pub fn app_background(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_APP)),
        text_color: Some(TEXT),
        ..Default::default()
    }
}

pub fn surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_SURFACE)),
        ..Default::default()
    }
}

/// A surface with a hairline on its right edge (sidebars).
pub fn sidebar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_SURFACE)),
        ..Default::default()
    }
}

pub fn divider(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BORDER)),
        ..Default::default()
    }
}

pub fn bar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_SURFACE)),
        ..Default::default()
    }
}

pub fn card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_SURFACE)),
        border: border(BORDER, RADIUS_LG),
        shadow: soft_shadow(0.35, 12.0, 32.0),
        ..Default::default()
    }
}

pub fn inset(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_INPUT)),
        border: border(BORDER, RADIUS_MD),
        ..Default::default()
    }
}

pub fn terminal(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_TERMINAL)),
        ..Default::default()
    }
}

pub fn popover(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_OVERLAY)),
        border: border(BORDER_STRONG, RADIUS_MD),
        shadow: soft_shadow(0.45, 8.0, 24.0),
        ..Default::default()
    }
}

pub fn tooltip(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_OVERLAY)),
        border: border(BORDER_STRONG, RADIUS_SM),
        shadow: soft_shadow(0.35, 4.0, 12.0),
        text_color: Some(TEXT),
        ..Default::default()
    }
}

pub fn modal_backdrop(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x03, 0x05, 0x0a, 0.66))),
        ..Default::default()
    }
}

pub fn modal(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_ELEVATED)),
        border: border(BORDER_STRONG, RADIUS_LG),
        shadow: soft_shadow(0.55, 24.0, 64.0),
        ..Default::default()
    }
}

pub fn toast(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_OVERLAY)),
        border: border(BORDER_STRONG, RADIUS_MD),
        shadow: soft_shadow(0.45, 10.0, 28.0),
        ..Default::default()
    }
}

pub fn fill(color: Color, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(color)),
        border: rounded(radius),
        ..Default::default()
    }
}

pub fn outlined(
    background: Color,
    outline: Color,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(background)),
        border: border(outline, radius),
        ..Default::default()
    }
}

pub fn banner(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(with_alpha(color, 0.10))),
        border: border(with_alpha(color, 0.35), RADIUS_MD),
        text_color: Some(color),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

fn is_active(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => ACCENT_HOVER,
        button::Status::Pressed => ACCENT_PRESSED,
        button::Status::Disabled => with_alpha(ACCENT, 0.45),
        button::Status::Active => ACCENT,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: if status == button::Status::Disabled {
            with_alpha(Color::WHITE, 0.7)
        } else {
            Color::WHITE
        },
        border: rounded(RADIUS_MD),
        shadow: if status == button::Status::Disabled {
            Shadow::default()
        } else {
            Shadow {
                color: with_alpha(ACCENT, 0.28),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 14.0,
            }
        },
        snap: true,
    }
}

pub fn danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => DANGER_STRONG,
        button::Status::Disabled => with_alpha(DANGER, 0.4),
        button::Status::Active => Color::from_rgb8(0xe5, 0x4d, 0x4d),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: Color::WHITE,
        border: rounded(RADIUS_MD),
        ..Default::default()
    }
}

pub fn secondary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, outline) = match status {
        button::Status::Hovered => (BG_PRESSED, BORDER_STRONG),
        button::Status::Pressed => (BG_HOVER, BORDER_STRONG),
        _ => (BG_HOVER, BORDER),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: if status == button::Status::Disabled {
            TEXT_DISABLED
        } else {
            TEXT
        },
        border: border(outline, RADIUS_MD),
        ..Default::default()
    }
}

pub fn ghost_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: is_active(status).then_some(Background::Color(BG_HOVER)),
        text_color: match status {
            button::Status::Disabled => TEXT_DISABLED,
            button::Status::Hovered | button::Status::Pressed => TEXT,
            button::Status::Active => TEXT_MUTED,
        },
        border: rounded(RADIUS_SM),
        ..Default::default()
    }
}

pub fn ghost_danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: is_active(status).then_some(Background::Color(DANGER_SOFT)),
        text_color: if is_active(status) {
            DANGER
        } else {
            TEXT_FAINT
        },
        border: rounded(RADIUS_SM),
        ..Default::default()
    }
}

pub fn outline_danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: is_active(status).then_some(Background::Color(DANGER_SOFT)),
        text_color: DANGER,
        border: border(
            with_alpha(DANGER, if is_active(status) { 0.6 } else { 0.35 }),
            RADIUS_SM,
        ),
        ..Default::default()
    }
}

pub fn link_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: if is_active(status) {
            ACCENT_HOVER
        } else {
            TEXT_FAINT
        },
        ..Default::default()
    }
}

pub fn accent_link_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: if is_active(status) {
            ACCENT_HOVER
        } else {
            ACCENT_TEXT
        },
        ..Default::default()
    }
}

/// A list row (hosts, keys, menu items). `selected` rows get an accent tint.
pub fn list_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let background = if selected {
            Some(Background::Color(ACCENT_SOFT))
        } else if is_active(status) {
            Some(Background::Color(BG_HOVER))
        } else {
            None
        };
        button::Style {
            background,
            text_color: if selected { TEXT } else { TEXT_MUTED },
            border: if selected {
                border(with_alpha(ACCENT, 0.35), RADIUS_MD)
            } else {
                rounded(RADIUS_MD)
            },
            ..Default::default()
        }
    }
}

pub fn tree_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let background = if selected {
            Some(Background::Color(ACCENT_SOFT))
        } else if is_active(status) {
            Some(Background::Color(BG_HOVER))
        } else {
            None
        };
        button::Style {
            background,
            text_color: if selected { TEXT } else { TEXT_MUTED },
            border: rounded(RADIUS_SM),
            ..Default::default()
        }
    }
}

pub fn menu_item(danger: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let hover = if danger { DANGER_SOFT } else { BG_PRESSED };
        button::Style {
            background: is_active(status).then_some(Background::Color(hover)),
            text_color: if danger { DANGER } else { TEXT },
            border: rounded(RADIUS_SM),
            ..Default::default()
        }
    }
}

/// One option of a segmented control.
pub fn segment(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| button::Style {
        background: if selected {
            Some(Background::Color(BG_OVERLAY))
        } else if is_active(status) {
            Some(Background::Color(BG_HOVER))
        } else {
            None
        },
        text_color: if selected { TEXT } else { TEXT_MUTED },
        border: if selected {
            border(BORDER_STRONG, RADIUS_SM)
        } else {
            rounded(RADIUS_SM)
        },
        shadow: if selected {
            soft_shadow(0.25, 1.0, 3.0)
        } else {
            Shadow::default()
        },
        snap: true,
    }
}

pub fn tab(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| button::Style {
        background: if active {
            Some(Background::Color(BG_APP))
        } else if is_active(status) {
            Some(Background::Color(BG_HOVER))
        } else {
            None
        },
        text_color: if active || is_active(status) {
            TEXT
        } else {
            TEXT_MUTED
        },
        border: rounded(RADIUS_SM),
        ..Default::default()
    }
}

pub fn tab_close(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: is_active(status).then_some(Background::Color(BG_PRESSED)),
        text_color: if is_active(status) { TEXT } else { TEXT_FAINT },
        border: rounded(4.0),
        ..Default::default()
    }
}

pub fn toolbar_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: is_active(status).then_some(Background::Color(BG_PRESSED)),
        text_color: match status {
            button::Status::Disabled => TEXT_DISABLED,
            button::Status::Hovered | button::Status::Pressed => TEXT,
            button::Status::Active => TEXT_MUTED,
        },
        border: rounded(RADIUS_SM),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

pub fn input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let outline = match status {
        text_input::Status::Focused { .. } => ACCENT,
        text_input::Status::Hovered => BORDER_STRONG,
        text_input::Status::Active => BORDER,
        text_input::Status::Disabled => BORDER,
    };
    text_input::Style {
        background: Background::Color(BG_INPUT),
        border: border(outline, RADIUS_MD),
        icon: TEXT_FAINT,
        placeholder: TEXT_DISABLED,
        value: TEXT,
        selection: with_alpha(ACCENT, 0.35),
    }
}

/// An input embedded in a container that draws its own border.
pub fn bare_input(_theme: &Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: TEXT_FAINT,
        placeholder: TEXT_DISABLED,
        value: TEXT,
        selection: with_alpha(ACCENT, 0.35),
    }
}

pub fn input_frame(focused: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(BG_INPUT)),
        border: border(if focused { ACCENT } else { BORDER }, RADIUS_MD),
        ..Default::default()
    }
}

pub fn code_editor(_theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let _ = status;
    text_editor::Style {
        background: Background::Color(BG_TERMINAL),
        border: Border::default(),
        placeholder: TEXT_DISABLED,
        value: TEXT,
        selection: with_alpha(ACCENT, 0.32),
    }
}

pub fn toggle(_theme: &Theme, status: toggler::Status) -> toggler::Style {
    let (is_toggled, hovered) = match status {
        toggler::Status::Active { is_toggled } => (is_toggled, false),
        toggler::Status::Hovered { is_toggled } => (is_toggled, true),
        toggler::Status::Disabled { is_toggled } => (is_toggled, false),
    };
    let track = match (is_toggled, hovered) {
        (true, true) => ACCENT_HOVER,
        (true, false) => ACCENT,
        (false, true) => BORDER_STRONG,
        (false, false) => BORDER,
    };
    toggler::Style {
        background: Background::Color(track),
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: Background::Color(if is_toggled { Color::WHITE } else { TEXT_MUTED }),
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
        text_color: Some(TEXT_MUTED),
        border_radius: None,
        padding_ratio: 0.12,
    }
}

pub fn select(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let outline = match status {
        pick_list::Status::Opened { .. } => ACCENT,
        pick_list::Status::Hovered => BORDER_STRONG,
        _ => BORDER,
    };
    pick_list::Style {
        text_color: TEXT,
        placeholder_color: TEXT_DISABLED,
        handle_color: TEXT_FAINT,
        background: Background::Color(BG_INPUT),
        border: border(outline, RADIUS_MD),
    }
}

pub fn select_menu(_theme: &Theme) -> iced::overlay::menu::Style {
    iced::overlay::menu::Style {
        background: Background::Color(BG_OVERLAY),
        border: border(BORDER_STRONG, RADIUS_MD),
        text_color: TEXT,
        selected_text_color: Color::WHITE,
        selected_background: Background::Color(ACCENT_SOFT),
        shadow: soft_shadow(0.45, 8.0, 24.0),
    }
}

pub fn checkbox_style(_theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let is_checked = match status {
        checkbox::Status::Active { is_checked }
        | checkbox::Status::Hovered { is_checked }
        | checkbox::Status::Disabled { is_checked } => is_checked,
    };
    checkbox::Style {
        background: Background::Color(if is_checked { ACCENT } else { BG_INPUT }),
        icon_color: Color::WHITE,
        border: border(if is_checked { ACCENT } else { BORDER_STRONG }, 4.0),
        text_color: Some(TEXT_MUTED),
    }
}

pub fn progress(color: Color) -> impl Fn(&Theme) -> progress_bar::Style {
    move |_theme| progress_bar::Style {
        background: Background::Color(BG_PRESSED),
        bar: Background::Color(color),
        border: rounded(2.0),
    }
}

pub fn scrollbar(_theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let scroller = match status {
        scrollable::Status::Dragged { .. } => with_alpha(TEXT, 0.28),
        scrollable::Status::Hovered { .. } => with_alpha(TEXT, 0.18),
        _ => with_alpha(TEXT, 0.09),
    };
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(scroller),
            border: rounded(4.0),
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(BG_OVERLAY),
            border: border(BORDER_STRONG, RADIUS_SM),
            shadow: Shadow::default(),
            icon: TEXT_MUTED,
        },
    }
}
