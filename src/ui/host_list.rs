use iced::widget::{
    Space, button, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};

use crate::app::messages::Message;
use crate::app::state::AppState;
use crate::models::{AuthType, HostRecord, HostSort};

use super::components::{self, icon, lucide};
use super::theme;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let brand = row![
        container(icon(lucide::square_terminal(), 18.0, iced::Color::WHITE))
            .center_x(Length::Fixed(34.0))
            .center_y(Length::Fixed(34.0))
            .style(theme::fill(theme::ACCENT, theme::RADIUS_MD)),
        column![
            text("RustSSH")
                .size(theme::TEXT_LG)
                .font(theme::SEMIBOLD)
                .color(theme::TEXT),
            text(concat!("v", env!("CARGO_PKG_VERSION")))
                .size(theme::TEXT_XS)
                .color(theme::TEXT_FAINT),
        ]
        .spacing(1),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let new_connection = components::labeled_button(
        lucide::plus(),
        "New connection",
        (!state.login.connecting).then_some(Message::NewConnection),
        theme::secondary_button,
    )
    .width(Length::Fill);

    let search = text_input("Search connections", &state.host_filter)
        .on_input(Message::HostFilterChanged)
        .padding([8, 12])
        .size(theme::TEXT_MD)
        .style(theme::input);

    let (sort_label, next_sort) = match state.host_sort {
        HostSort::Label => ("Name", HostSort::Recent),
        HostSort::Recent => ("Recent", HostSort::Host),
        HostSort::Host => ("Host", HostSort::Label),
    };
    let section = row![
        components::caption("SAVED CONNECTIONS"),
        text(state.hosts.len().to_string())
            .size(theme::TEXT_XS)
            .color(theme::TEXT_DISABLED),
        Space::new().width(Length::Fill),
        components::with_tooltip(
            button(
                row![
                    icon(lucide::arrow_up_down(), 11.0, theme::TEXT_FAINT),
                    text(sort_label)
                        .size(theme::TEXT_XS)
                        .color(theme::TEXT_FAINT),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding([3, 6])
            .on_press(Message::HostSortChanged(next_sort))
            .style(theme::ghost_button),
            "Change sort order",
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let hosts = state.visible_hosts();
    let list: Element<'_, Message> = if hosts.is_empty() {
        empty_state(state)
    } else {
        scrollable(
            column(hosts.into_iter().map(|host| host_row(state, host)))
                .spacing(2)
                .padding(iced::Padding {
                    right: 10.0,
                    ..iced::Padding::ZERO
                }),
        )
        .style(theme::scrollbar)
        .height(Length::Fill)
        .into()
    };

    let content = column![
        container(brand).padding([18, 16]),
        components::horizontal_divider(),
        column![new_connection, search]
            .spacing(10)
            .padding([14, 14]),
        container(section).padding(iced::Padding {
            top: 6.0,
            right: 14.0,
            bottom: 8.0,
            left: 16.0,
        }),
        container(list)
            .padding(iced::Padding {
                top: 0.0,
                right: 4.0,
                bottom: 12.0,
                left: 10.0,
            })
            .height(Length::Fill),
    ]
    .height(Length::Fill);

    row![
        container(content)
            .width(theme::SIDEBAR_WIDTH)
            .height(Length::Fill)
            .style(theme::sidebar),
        components::vertical_divider(),
    ]
    .into()
}

fn host_row<'a>(state: &'a AppState, host: &'a HostRecord) -> Element<'a, Message> {
    let selected = state.login.editing_host_id == Some(host.id);
    let accent = components::avatar_color(&host.label);

    let avatar = container(
        text(components::initials(&host.label))
            .size(theme::TEXT_SM)
            .font(theme::SEMIBOLD)
            .color(accent),
    )
    .center_x(Length::Fixed(32.0))
    .center_y(Length::Fixed(32.0))
    .style(theme::fill(
        theme::with_alpha(accent, 0.14),
        theme::RADIUS_MD,
    ));

    let address = if host.port == 22 {
        format!("{}@{}", host.username, host.host)
    } else {
        format!("{}@{}:{}", host.username, host.host, host.port)
    };
    let auth_icon = match host.auth_type {
        AuthType::Password => lucide::lock(),
        AuthType::Key => lucide::key_round(),
    };

    let details = column![
        text(&host.label)
            .size(theme::TEXT_MD)
            .font(theme::MEDIUM)
            .color(if selected {
                theme::TEXT
            } else {
                theme::TEXT_MUTED
            })
            .wrapping(text::Wrapping::None),
        row![
            icon(auth_icon, 10.0, theme::TEXT_FAINT),
            text(address)
                .size(theme::TEXT_XS)
                .color(theme::TEXT_FAINT)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    ]
    .spacing(3)
    .width(Length::Fill)
    .clip(true);

    // The inner mouse area receives clicks first so double-click works; the
    // outer button only provides hover styling.
    let body = mouse_area(
        container(row![avatar, details].spacing(10).align_y(Alignment::Center))
            .padding([8, 10])
            .width(Length::Fill),
    )
    .on_press(Message::HostSelected(host.id))
    .on_double_click(Message::HostActivated(host.id))
    .interaction(iced::mouse::Interaction::Pointer);

    let remove = components::with_tooltip(
        button(icon(lucide::trash_two(), 13.0, theme::TEXT_FAINT))
            .padding(6)
            .on_press(Message::DeleteHost(host.id))
            .style(theme::ghost_danger_button),
        "Delete connection",
    );

    button(
        row![body, container(remove).padding([0, 6])]
            .align_y(Alignment::Center)
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::HostSelected(host.id))
    .style(theme::list_row(selected))
    .into()
}

fn empty_state(state: &AppState) -> Element<'_, Message> {
    let (title, body) = if state.hosts.is_empty() {
        (
            "No saved connections",
            "Connections you save appear here. Double-click one to connect.",
        )
    } else {
        ("No matches", "No saved connection matches your search.")
    };

    container(
        column![
            icon(lucide::server(), 22.0, theme::TEXT_DISABLED),
            text(title)
                .size(theme::TEXT_MD)
                .font(theme::MEDIUM)
                .color(theme::TEXT_MUTED),
            text(body)
                .size(theme::TEXT_SM)
                .color(theme::TEXT_FAINT)
                .align_x(Alignment::Center),
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .padding([32, 16])
    .center_x(Length::Fill)
    .into()
}
