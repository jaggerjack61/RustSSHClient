pub mod messages;
pub mod state;
pub mod update;
pub mod view;

use iced::Theme;

use state::AppState;

fn application_theme(_: &AppState) -> Theme {
    Theme::Dark
}

fn application_title(state: &AppState) -> String {
    match state.is_connected() {
        true => format!("RustSSH Client - {}", state.workspace.current_directory),
        false => "RustSSH Client".to_string(),
    }
}

pub fn run() -> iced::Result {
    iced::application(AppState::boot, update::update, view::view)
        .subscription(update::subscription)
        .theme(application_theme)
        .title(application_title)
        .window_size([1200.0, 780.0])
        .centered()
        .antialiasing(true)
        .run()
}
