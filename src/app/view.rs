use iced::widget::{container, stack};
use iced::{Element, Length};

use crate::app::messages::Message;
use crate::app::state::{AppState, Route};
use crate::ui;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let content = match state.route {
        Route::Login => ui::login::view(state),
        Route::Workspace => ui::terminal::workspace_view(state),
    };

    container(stack([content, ui::styles::notifications(state)]))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(ui::styles::root_container)
    .into()
}
