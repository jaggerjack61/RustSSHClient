use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Color, Element, Length};

use crate::app::messages::Message;
use crate::app::state::AppState;

use super::styles;

pub fn view(state: &AppState) -> Element<'_, Message> {
    // --- Header: SSH Client branding ---
    let header = container(
        row![
            container(text(">_").size(14).color(styles::text_white()),)
                .padding([8, 10])
                .style(|_theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(styles::primary())),
                    border: iced::Border {
                        radius: 8.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            column![
                text("SSH Client")
                    .size(16)
                    .color(styles::text_white()),
                text("v0.1.0")
                    .size(11)
                    .color(Color::from_rgba8(0x3b, 0x82, 0xf6, 0.7)),
            ]
            .spacing(2)
            .width(Length::Fill),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
    )
    .padding([20, 24])
    .width(Length::Fill)
    .style(styles::sidebar_header);

    // --- Saved Connections label ---
    let section_label = container(
        text("SAVED CONNECTIONS")
            .size(10)
            .color(styles::text_slate_500()),
    )
    .padding([20, 14]);

    // --- Host list ---
    let hosts = state.sorted_hosts();

    let host_entries = hosts.into_iter().fold(column![].spacing(6), |col, host| {
        let is_editing = state.login.editing_host_id == Some(host.id);

        let label_lower = host.label.to_lowercase();
        let icon = if label_lower.contains("db") {
            "\u{1F5C4}"
        } else if label_lower.contains("web") || label_lower.contains("staging") {
            "\u{2601}"
        } else if label_lower.contains("backup") {
            "\u{1F5A5}"
        } else {
            "\u{1F5A7}"
        };
        let host_port = format!("{}:{}", host.host, host.port);

        let card = button(
            row![
                text(icon).size(18),
                column![
                    text(host.label.clone())
                        .size(14)
                        .color(if is_editing {
                            styles::text_white()
                        } else {
                            styles::text_slate_400()
                        }),
                    text(host_port)
                        .size(10)
                        .color(if is_editing {
                            styles::text_slate_400()
                        } else {
                            styles::text_slate_500()
                        }),
                ]
                .spacing(2)
                .width(Length::Fill),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::HostCardPressed(host.id))
        .padding([12, 14])
        .width(Length::Fill);

        let card = if is_editing {
            card.style(styles::host_card_active)
        } else {
            card.style(styles::host_card_button)
        };

        let delete_button = button(
            container(
                text("\u{1F5D1}")
                    .size(14)
                    .color(styles::text_slate_500()),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill),
        )
        .on_press(Message::DeleteHost(host.id))
        .padding([10, 0])
        .width(Length::Fixed(40.0))
        .style(styles::ghost_button);

        col.push(
            row![card, delete_button]
                .spacing(6)
                .width(Length::Fill)
                .align_y(iced::Alignment::Center),
        )
    });

    let host_list = container(
        scrollable(host_entries)
            .style(styles::dark_scrollable)
            .height(Length::Fill),
    )
    .padding([12, 12]);

    // --- Assemble sidebar ---
    container(
        column![header, section_label, host_list].height(Length::Fill),
    )
    .width(300)
    .height(Length::Fill)
    .style(styles::sidebar_container)
    .into()
}
