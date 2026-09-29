use iced::Element;
use iced::widget::{container, stack};

use crate::app::messages::Message;
use crate::app::state::{AppState, Route};
use crate::ui;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let content = match state.route {
        Route::Login => ui::login::view(state),
        Route::Workspace => ui::workspace::view(state),
    };

    let mut layers = stack![content];
    if let Some(toasts) = ui::modals::notifications(state) {
        layers = layers.push(toasts);
    }
    if let Some(modal) = ui::modals::view(state) {
        layers = layers.push(modal);
    }

    container(layers).style(ui::theme::app_background).into()
}
