use iced::widget::{button, checkbox, column, container, row, scrollable, stack, text, text_input, Space};
use iced::{Color, Element, Length};

use crate::app::messages::Message;
use crate::app::state::AppState;
use crate::models::{AuthType, SaveLifetime};

use super::{host_list, styles};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FooterLinkKind {
    Project,
    KeyManager,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FooterLinkSpec {
    label: &'static str,
    kind: FooterLinkKind,
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    // --- Form fields matching the HTML mockup ---

    // Host Address
    let host_field = column![
        text("HOST ADDRESS")
            .size(10)
            .color(styles::text_slate_500()),
        text_input("e.g. 192.168.1.1 or server.com", &state.login.host)
            .on_input(Message::LoginHostChanged)
            .padding([14, 16])
            .width(Length::Fill)
            .style(styles::dark_input),
    ]
    .spacing(8)
    .width(Length::Fill);

    // Username + Port row
    let user_port_row = row![
        column![
            text("USERNAME")
                .size(10)
                .color(styles::text_slate_500()),
            text_input("root", &state.login.username)
                .on_input(Message::LoginUsernameChanged)
                .padding([14, 16])
                .width(Length::Fill)
                .style(styles::dark_input),
        ]
        .spacing(8)
        .width(Length::FillPortion(2)),
        column![
            text("PORT")
                .size(10)
                .color(styles::text_slate_500()),
            text_input("22", &state.login.port)
                .on_input(Message::LoginPortChanged)
                .padding([14, 16])
                .width(Length::Fill)
                .style(styles::dark_input),
        ]
        .spacing(8)
        .width(Length::FillPortion(1)),
    ]
    .spacing(16)
    .width(Length::Fill);

    // Auth type section
    let auth_buttons = row![
        styles::auth_badge(
            state.login.auth_type == AuthType::Password,
            "Password",
            Message::UsePasswordAuthentication,
        ),
        styles::auth_badge(
            state.login.auth_type == AuthType::Key,
            "SSH Key",
            Message::UseKeyAuthentication,
        ),
    ]
    .spacing(12)
    .width(Length::Fill);

    // Password / Key field
    let mut auth_section = column![].spacing(8);

    match state.login.auth_type {
        AuthType::Password => {
            auth_section = auth_section.push(
                column![
                    text("PASSWORD / SSH KEY")
                        .size(10)
                        .color(styles::text_slate_500()),
                    text_input("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}", &state.login.password)
                        .secure(true)
                        .on_input(Message::LoginPasswordChanged)
                        .padding([14, 16])
                        .width(Length::Fill)
                        .style(styles::dark_input),
                ]
                .spacing(8)
                .width(Length::Fill),
            );
        }
        AuthType::Key => {
            let mut key_list: iced::widget::Column<'_, Message> = column![].spacing(6);
            for key in &state.keys {
                let is_selected = state.login.selected_key == Some(key.id);
                key_list = key_list.push(
                    row![
                        button(
                            text(&key.label)
                                .size(13)
                                .color(if is_selected {
                                    Color::WHITE
                                } else {
                                    styles::text_slate_400()
                                }),
                        )
                        .on_press(Message::SelectKey(key.id))
                        .padding([8, 12])
                        .width(Length::Fill)
                        .style(if is_selected {
                            styles::host_card_active as fn(&iced::Theme, button::Status) -> button::Style
                        } else {
                            styles::ghost_button
                        }),
                        button(
                            text("\u{2715}").size(12).color(styles::text_slate_500()),
                        )
                        .on_press(Message::DeleteKey(key.id))
                        .padding([8, 0])
                        .width(Length::Fixed(36.0))
                        .style(styles::ghost_button),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
            }

            auth_section = auth_section.push(
                text("SSH KEYS")
                    .size(10)
                    .color(styles::text_slate_500()),
            );

            if state.keys.is_empty() {
                auth_section = auth_section.push(
                    text("Import a PEM key to enable key-based authentication.")
                        .size(13)
                        .color(styles::text_slate_600()),
                );
            } else {
                auth_section = auth_section.push(
                    scrollable(key_list)
                        .width(Length::Fill)
                        .height(100)
                        .style(styles::dark_scrollable),
                );
            }

            auth_section = auth_section.push(
                button(
                    text("Import PEM Key")
                        .size(13)
                        .color(styles::primary()),
                )
                .on_press(Message::ImportKeyPressed)
                .padding([8, 16])
                .style(styles::ghost_button),
            );
        }
    }

    auth_section = auth_section.width(Length::Fill);

    // Save connection + Advanced Settings row
    let options_row = row![
        checkbox(state.login.save_connection)
            .label("Save connection")
            .on_toggle(Message::ToggleSaveConnection)
            .style(styles::dark_checkbox)
            .text_size(14),
        Space::new().width(Length::Fill),
        button(
            text(format!("Advanced Settings · {}", state.login.save_lifetime.label()))
                .size(12)
                .color(styles::primary()),
        )
        .on_press(Message::OpenAdvancedSettings)
        .style(styles::accent_link_button),
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    // Connect button
    let connect_label = if state.login.connecting {
        "Connecting..."
    } else {
        "\u{26A1} Connect Now"
    };

    let connect_button = button(
        container(
            text(connect_label)
                .size(16)
                .color(Color::WHITE),
        )
        .center_x(Length::Fill)
        .center_y(Length::Shrink),
    )
    .on_press_maybe((!state.login.connecting).then_some(Message::ConnectPressed))
    .padding([16, 24])
    .width(Length::Fill)
    .style(styles::primary_button);

    // Security badges
    let security_badges = row![
        text("\u{1F6E1}").size(12).color(styles::accent()),
        text("AES-256")
            .size(11)
            .color(styles::text_slate_500()),
        text("\u{00B7}").size(14).color(styles::text_slate_600()),
        text("\u{1F511}").size(12).color(styles::accent()),
        text("SSH KEY")
            .size(11)
            .color(styles::text_slate_500()),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let footer_links = row(footer_link_specs().into_iter().map(footer_link_button))
        .spacing(18)
        .align_y(iced::Alignment::Center);

    let card_meta = container(
        column![
            security_badges,
            footer_links,
        ]
        .spacing(10)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .center_x(Length::Fill)
    .padding([6, 0]);

    // --- Assemble form card ---
    let form = column![
        // Title
        text("New Connection")
            .size(30)
            .color(Color::WHITE),
        text("Enter server credentials to begin session")
            .size(14)
            .color(styles::text_slate_400()),
        Space::new().height(8),
        host_field,
        user_port_row,
        auth_buttons,
        auth_section,
        options_row,
        connect_button,
        card_meta,
    ]
    .spacing(16)
    .width(Length::Fill)
    .align_x(iced::Alignment::Start);

    let card = container(form)
    .width(Length::Fill)
    .max_width(460)
    .padding([26, 30])
    .style(styles::glass_card);

    // --- Main content area ---
    let main_content = container(
        column![
            Space::new().height(Length::Fill),
            container(card)
                .width(Length::Fill)
                .center_x(Length::Fill),
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .padding([24, 32])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(styles::main_area);

    let layout: Element<'_, Message> = row![host_list::view(state), main_content]
        .height(Length::Fill)
        .into();

    if state.advanced_settings_open {
        stack([layout, advanced_settings_modal(state)]).into()
    } else {
        layout
    }
}

fn footer_link_specs() -> [FooterLinkSpec; 3] {
    [
        FooterLinkSpec {
            label: "Documentation",
            kind: FooterLinkKind::Project,
        },
        FooterLinkSpec {
            label: "Support",
            kind: FooterLinkKind::Project,
        },
        FooterLinkSpec {
            label: "Key Manager",
            kind: FooterLinkKind::KeyManager,
        },
    ]
}

fn footer_link_button<'a>(spec: FooterLinkSpec) -> Element<'a, Message> {
    let message = match spec.kind {
        FooterLinkKind::Project => Message::OpenProjectLink,
        FooterLinkKind::KeyManager => Message::ToggleKeyManager,
    };

    button(text(spec.label).size(14).color(styles::text_slate_500()))
        .on_press(message)
        .style(styles::link_button)
        .into()
}

fn advanced_settings_modal(state: &AppState) -> Element<'_, Message> {
    let lifetime_options = SaveLifetime::ALL.into_iter().fold(
        column![].spacing(10),
        |column, lifetime| {
            let is_selected = state.login.save_lifetime == lifetime;
            let label = if is_selected {
                format!("{}  •  Active", lifetime.label())
            } else {
                lifetime.label().to_string()
            };

            column.push(
                button(
                    row![
                        column![
                            text(label)
                                .size(15)
                                .color(Color::WHITE),
                            text(lifetime.detail())
                                .size(12)
                                .color(styles::text_slate_400()),
                        ]
                        .spacing(4)
                        .width(Length::Fill),
                        text(if is_selected { "KEEP" } else { "SET" })
                            .size(10)
                            .color(if is_selected {
                                styles::blue_400()
                            } else {
                                styles::text_slate_500()
                            }),
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::SelectSaveLifetime(lifetime))
                .padding([14, 16])
                .width(Length::Fill)
                .style(if is_selected {
                    styles::settings_option_active_button
                } else {
                    styles::settings_option_button
                }),
            )
        },
    );

    let panel = container(
        column![
            row![
                column![
                    text("Credential Retention")
                        .size(26)
                        .color(Color::WHITE),
                    text("Decide how long saved logins stay in the local encrypted vault.")
                        .size(13)
                        .color(styles::text_slate_400()),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                button(text("x").size(14).color(styles::text_slate_500()))
                    .on_press(Message::CloseAdvancedSettings)
                    .padding([8, 10])
                    .style(styles::ghost_button),
            ]
            .align_y(iced::Alignment::Start),
            container(
                row![
                    column![
                        text("VAULT POLICY")
                            .size(10)
                            .color(styles::text_slate_500()),
                        text(state.login.save_lifetime.label())
                            .size(20)
                            .color(styles::blue_400()),
                    ]
                    .spacing(6)
                    .width(Length::FillPortion(2)),
                    column![
                        text("BEHAVIOR")
                            .size(10)
                            .color(styles::text_slate_500()),
                        text(if state.login.save_connection {
                            "Applied to newly saved or updated credentials"
                        } else {
                            "Enable Save connection to persist credentials at all"
                        })
                        .size(12)
                        .color(styles::text_slate_400()),
                    ]
                    .spacing(6)
                    .width(Length::FillPortion(3)),
                ]
                .spacing(18),
            )
            .padding(18)
            .style(styles::settings_modal_section),
            lifetime_options,
            container(
                column![
                    text("Forever leaves the credential in storage until you remove it manually.")
                        .size(12)
                        .color(styles::text_slate_500())
                        .width(Length::Fill),
                    row![
                        Space::new().width(Length::Fill),
                        button(
                            text("Done")
                                .size(13)
                                .color(Color::WHITE),
                        )
                        .on_press(Message::CloseAdvancedSettings)
                        .padding([10, 18])
                        .style(styles::primary_button),
                    ]
                    .align_y(iced::Alignment::Center),
                ]
                .spacing(12),
            )
            .padding([8, 0]),
        ]
        .spacing(18),
    )
    .max_width(620)
    .padding(28)
    .style(styles::settings_modal_panel);

    container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(24)
        .style(styles::modal_backdrop)
        .into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn footer_link_specs_include_documentation_support_and_key_manager() {
        let specs = super::footer_link_specs();

        assert_eq!(specs[0].label, "Documentation");
        assert_eq!(specs[1].label, "Support");
        assert_eq!(specs[2].label, "Key Manager");
    }

    #[test]
    fn footer_link_specs_keep_project_actions_grouped() {
        let specs = super::footer_link_specs();

        assert_eq!(specs[0].kind, super::FooterLinkKind::Project);
        assert_eq!(specs[1].kind, super::FooterLinkKind::Project);
        assert_eq!(specs[2].kind, super::FooterLinkKind::KeyManager);
    }

    #[test]
    fn footer_link_specs_remain_three_items_long() {
        assert_eq!(super::footer_link_specs().len(), 3);
    }
}
