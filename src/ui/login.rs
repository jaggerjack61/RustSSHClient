use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input, toggler,
};
use iced::{Alignment, Element, Length};

use crate::app::messages::Message;
use crate::app::state::AppState;
use crate::models::{AuthType, SaveLifetime, SshKeyRecord};

use super::components::{self, icon, lucide};
use super::{host_list, theme};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let form = &state.login;
    let enabled = !form.connecting;
    let on_input = |message: fn(String) -> Message| move |value| message(value);

    let (title, subtitle) = if form.editing_host_id.is_some() {
        (
            "Connect to server",
            "Review the saved details, then connect.",
        )
    } else {
        (
            "New connection",
            "Enter the server details to open a secure shell.",
        )
    };

    let header = column![
        text(title)
            .size(theme::TEXT_2XL)
            .font(theme::SEMIBOLD)
            .color(theme::TEXT),
        text(subtitle).size(theme::TEXT_MD).color(theme::TEXT_MUTED),
    ]
    .spacing(6);

    let label_input = text_input("Production web server", &form.label)
        .on_input_maybe(enabled.then_some(on_input(Message::LoginLabelChanged)))
        .on_submit(Message::ConnectPressed)
        .padding([10, 12])
        .size(theme::TEXT_MD)
        .style(theme::input);

    let host_input = text_input("192.168.1.10 or server.example.com", &form.host)
        .on_input_maybe(enabled.then_some(on_input(Message::LoginHostChanged)))
        .on_submit(Message::ConnectPressed)
        .padding([10, 12])
        .size(theme::TEXT_MD)
        .style(theme::input);

    let port_input = text_input("22", &form.port)
        .on_input_maybe(enabled.then_some(on_input(Message::LoginPortChanged)))
        .on_submit(Message::ConnectPressed)
        .padding([10, 12])
        .size(theme::TEXT_MD)
        .style(theme::input);

    let username_input = text_input("root", &form.username)
        .on_input_maybe(enabled.then_some(on_input(Message::LoginUsernameChanged)))
        .on_submit(Message::ConnectPressed)
        .padding([10, 12])
        .size(theme::TEXT_MD)
        .style(theme::input);

    let address_row = row![
        components::field("Host", host_input),
        container(components::field("Port", port_input)).width(Length::Fixed(92.0)),
    ]
    .spacing(12);

    let auth_switch = container(
        row![
            auth_segment(
                "Password",
                lucide::lock(),
                form.auth_type == AuthType::Password,
                enabled.then_some(Message::UsePasswordAuthentication),
            ),
            auth_segment(
                "SSH key",
                lucide::key_round(),
                form.auth_type == AuthType::Key,
                enabled.then_some(Message::UseKeyAuthentication),
            ),
        ]
        .spacing(4),
    )
    .padding(3)
    .style(theme::inset);

    let auth_section: Element<'_, Message> = match form.auth_type {
        AuthType::Password => {
            components::field("Password", secret_input(state, "Password", enabled))
        }
        AuthType::Key => key_section(state, enabled),
    };

    let save_row = row![
        toggler(form.save_connection)
            .label("Save to vault")
            .on_toggle_maybe(enabled.then_some(Message::ToggleSaveConnection))
            .size(18)
            .text_size(theme::TEXT_MD)
            .spacing(10)
            .style(theme::toggle),
        Space::new().width(Length::Fill),
        text("Keep for")
            .size(theme::TEXT_SM)
            .color(if form.save_connection {
                theme::TEXT_FAINT
            } else {
                theme::TEXT_DISABLED
            }),
        pick_list(
            SaveLifetime::ALL,
            Some(form.save_lifetime),
            Message::SelectSaveLifetime,
        )
        .padding([5, 10])
        .text_size(theme::TEXT_SM)
        .style(theme::select)
        .menu_style(theme::select_menu),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let error_banner: Element<'_, Message> = match &form.error {
        Some(error) => container(
            row![
                icon(lucide::circle_alert(), 15.0, theme::DANGER),
                text(error).size(theme::TEXT_SM).color(theme::TEXT),
            ]
            .spacing(10)
            .align_y(Alignment::Start),
        )
        .padding([10, 12])
        .width(Length::Fill)
        .style(theme::banner(theme::DANGER))
        .into(),
        None => Space::new().into(),
    };

    let connect_label = if form.connecting {
        "Connecting\u{2026}"
    } else {
        "Connect"
    };
    let connect_icon = if form.connecting {
        lucide::loader_circle()
    } else {
        lucide::zap()
    };
    let connect = button(
        row![
            icon(connect_icon, 15.0, iced::Color::WHITE),
            text(connect_label)
                .size(theme::TEXT_LG)
                .font(theme::SEMIBOLD),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press_maybe(enabled.then_some(Message::ConnectPressed))
    .padding([11, 16])
    .width(Length::Fill)
    .style(theme::primary_button);

    let actions: Element<'_, Message> = if form.connecting {
        row![
            container(connect).width(Length::Fill),
            components::text_button(
                "Cancel",
                Some(Message::CancelConnect),
                theme::secondary_button
            )
            .padding([11, 16]),
        ]
        .spacing(10)
        .into()
    } else {
        connect.into()
    };

    let assurances = row![
        icon(lucide::shield_check(), 12.0, theme::SUCCESS),
        text("Vault encrypted with AES-256-GCM")
            .size(theme::TEXT_XS)
            .color(theme::TEXT_FAINT),
        Space::new().width(8),
        icon(lucide::fingerprint(), 12.0, theme::SUCCESS),
        text("Host keys verified")
            .size(theme::TEXT_XS)
            .color(theme::TEXT_FAINT),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let mut sections = column![
        header,
        column![
            components::field("Display name (optional)", label_input),
            address_row,
            components::field("Username", username_input),
        ]
        .spacing(14),
        column![
            components::field("Authentication", auth_switch),
            auth_section
        ]
        .spacing(14),
        components::horizontal_divider(),
        save_row,
    ]
    .spacing(20);
    if form.error.is_some() {
        sections = sections.push(error_banner);
    }
    sections = sections
        .push(actions)
        .push(container(assurances).center_x(Length::Fill));

    let card = container(sections)
        .padding(28)
        .max_width(460)
        .style(theme::card);

    let footer = row![
        footer_link("Documentation", Message::OpenProjectLink),
        text("\u{00b7}")
            .size(theme::TEXT_SM)
            .color(theme::TEXT_DISABLED),
        footer_link("Report an issue", Message::OpenIssuesLink),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let main = scrollable(
        container(
            column![card, footer]
                .spacing(18)
                .align_x(Alignment::Center)
                .width(Length::Fill),
        )
        .padding([40, 32])
        .center_x(Length::Fill),
    )
    .style(theme::scrollbar)
    .width(Length::Fill)
    .height(Length::Shrink);

    // A shrinking scrollable inside a centering container: centered when the
    // card fits, scrollable when the window is too short.
    row![
        host_list::view(state),
        container(main)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .style(theme::app_background),
    ]
    .height(Length::Fill)
    .into()
}

fn auth_segment<'a>(
    label: &'a str,
    glyph: text::Text<'a>,
    selected: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let color = if selected {
        theme::TEXT
    } else {
        theme::TEXT_MUTED
    };
    button(
        row![
            icon(glyph, 13.0, color),
            text(label).size(theme::TEXT_MD).font(theme::MEDIUM),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press_maybe(on_press)
    .padding([7, 12])
    .width(Length::Fill)
    .style(theme::segment(selected))
    .into()
}

/// A password field with a show/hide toggle.
fn secret_input<'a>(
    state: &'a AppState,
    placeholder: &'a str,
    enabled: bool,
) -> Element<'a, Message> {
    let form = &state.login;
    let visible = form.password_visible;

    let input = text_input(placeholder, &form.password)
        .secure(!visible)
        .on_input_maybe(enabled.then_some(Message::LoginPasswordChanged as fn(String) -> Message))
        .on_submit(Message::ConnectPressed)
        .padding([10, 12])
        .size(theme::TEXT_MD)
        .style(theme::bare_input);

    let reveal = components::with_tooltip(
        button(icon(
            if visible {
                lucide::eye_off()
            } else {
                lucide::eye()
            },
            15.0,
            theme::TEXT_FAINT,
        ))
        .padding(6)
        .on_press(Message::TogglePasswordVisibility)
        .style(theme::ghost_button),
        if visible { "Hide" } else { "Show" },
    );

    container(
        row![input, reveal]
            .align_y(Alignment::Center)
            .padding(iced::Padding {
                right: 4.0,
                ..iced::Padding::ZERO
            }),
    )
    .style(theme::input_frame(false))
    .into()
}

fn key_section(state: &AppState, enabled: bool) -> Element<'_, Message> {
    let keys: Element<'_, Message> = if state.keys.is_empty() {
        container(
            row![
                icon(lucide::key_round(), 15.0, theme::TEXT_FAINT),
                text("No keys imported yet. Import an OpenSSH or PEM private key.")
                    .size(theme::TEXT_SM)
                    .color(theme::TEXT_MUTED),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([12, 12])
        .width(Length::Fill)
        .style(theme::inset)
        .into()
    } else {
        let list = column(
            state
                .keys
                .iter()
                .map(|key| key_row(key, state.login.selected_key == Some(key.id), enabled)),
        )
        .spacing(4);

        container(
            scrollable(list)
                .style(theme::scrollbar)
                .height(Length::Shrink),
        )
        .max_height(148)
        .padding(4)
        .style(theme::inset)
        .into()
    };

    let import = components::labeled_button(
        lucide::upload(),
        "Import key\u{2026}",
        enabled.then_some(Message::ImportKeyPressed),
        theme::secondary_button,
    );

    column![
        components::field("Private key", column![keys, import].spacing(8)),
        components::field(
            "Key passphrase (if encrypted)",
            secret_input(state, "Leave empty for unencrypted keys", enabled),
        ),
    ]
    .spacing(14)
    .into()
}

fn key_row(key: &SshKeyRecord, selected: bool, enabled: bool) -> Element<'_, Message> {
    let body = row![
        icon(
            lucide::key_round(),
            14.0,
            if selected {
                theme::ACCENT_TEXT
            } else {
                theme::TEXT_FAINT
            }
        ),
        column![
            text(&key.label)
                .size(theme::TEXT_MD)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
            text(format!("Imported {}", key.created_at.format("%b %-d, %Y")))
                .size(theme::TEXT_XS)
                .color(theme::TEXT_FAINT),
        ]
        .spacing(2)
        .width(Length::Fill)
        .clip(true),
        components::with_tooltip(
            button(icon(lucide::trash_two(), 13.0, theme::TEXT_FAINT))
                .padding(5)
                .on_press_maybe(enabled.then_some(Message::DeleteKey(key.id)))
                .style(theme::ghost_danger_button),
            "Remove key",
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    button(body)
        .padding([6, 8])
        .width(Length::Fill)
        .on_press_maybe(enabled.then_some(Message::SelectKey(key.id)))
        .style(theme::list_row(selected))
        .into()
}

fn footer_link(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(theme::TEXT_SM))
        .padding([2, 2])
        .on_press(message)
        .style(theme::link_button)
        .into()
}
