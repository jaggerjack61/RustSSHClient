pub mod messages;
pub mod state;
pub mod update;
pub mod view;

use iced::{Size, Theme, window};

use state::AppState;

use crate::ui::theme;

fn application_theme(_: &AppState) -> Theme {
    theme::app_theme()
}

fn application_title(state: &AppState) -> String {
    if state.is_connected() {
        let user = state.login.username.trim();
        format!(
            "{user}@{}: {} \u{2014} RustSSH",
            state.workspace.connected_peer, state.workspace.current_directory
        )
    } else {
        "RustSSH".to_string()
    }
}

pub fn run() -> iced::Result {
    let mut application = iced::application(AppState::boot, update::update, view::view)
        .subscription(update::subscription)
        .theme(application_theme)
        .title(application_title)
        .default_font(theme::UI_FONT)
        .window(window::Settings {
            size: Size::new(1280.0, 800.0),
            min_size: Some(Size::new(900.0, 560.0)),
            position: window::Position::Centered,
            ..window::Settings::default()
        })
        .antialiasing(true);

    for font in theme::FONTS {
        application = application.font(font);
    }

    application.run()
}
