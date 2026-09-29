use iced::widget::{
    Space, button, column, container, markdown, row, scrollable, text, text_editor,
};
use iced::{Alignment, Element, Length};

use crate::app::messages::Message;
use crate::app::state::AppState;
use crate::models::{EditorDocument, EditorLanguage};

use super::components::{self, icon, lucide};
use super::theme;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let Some(document) = state.active_editor() else {
        return super::terminal::empty();
    };

    let body: Element<'_, Message> = if document.is_loading {
        placeholder(
            icon(lucide::loader_circle(), 20.0, theme::TEXT_FAINT),
            "Loading remote file\u{2026}",
            None,
        )
    } else if let Some(error) = &document.load_error {
        placeholder(
            icon(lucide::file_text(), 22.0, theme::DANGER),
            "This file can\u{2019}t be opened",
            Some(error),
        )
    } else if document.markdown_preview {
        let preview = markdown::view(
            &document.markdown_items,
            markdown::Settings::with_text_size(14, theme::app_theme()),
        )
        .map(|url| Message::MarkdownLinkClicked(url.to_string()));

        scrollable(
            container(container(preview).max_width(820))
                .padding([24, 32])
                .center_x(Length::Fill),
        )
        .style(theme::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        let path = document.path.clone();
        let editor = text_editor(&document.buffer)
            .on_action(move |action| Message::EditorAction(path.clone(), action))
            .font(theme::MONO_FONT)
            .size(13)
            .padding([12, 16])
            .height(Length::Fill)
            .style(theme::code_editor);

        match document.language.syntax_token() {
            Some(token) => editor
                .highlight(token, iced::highlighter::Theme::Base16Ocean)
                .into(),
            None => editor.into(),
        }
    };

    column![
        header(document),
        components::horizontal_divider(),
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::terminal),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn header(document: &EditorDocument) -> Element<'_, Message> {
    let (status, status_color) = if document.is_saving {
        ("Saving\u{2026}", theme::TEXT_FAINT)
    } else if document.is_dirty {
        ("Unsaved changes", theme::WARNING)
    } else if document.is_loading || document.load_error.is_some() {
        ("", theme::TEXT_FAINT)
    } else {
        ("Saved", theme::SUCCESS)
    };

    let can_save = !document.is_loading
        && document.load_error.is_none()
        && !document.is_saving
        && document.is_dirty;

    let mut actions = row![].spacing(8).align_y(Alignment::Center);
    if document.language == EditorLanguage::Markdown && document.load_error.is_none() {
        actions = actions.push(components::labeled_button(
            if document.markdown_preview {
                lucide::square_pen()
            } else {
                lucide::book_open()
            },
            if document.markdown_preview {
                "Edit"
            } else {
                "Preview"
            },
            (!document.is_loading).then_some(Message::ToggleMarkdownPreview),
            theme::secondary_button,
        ));
    }
    actions = actions.push(
        button(
            row![
                icon(
                    lucide::save(),
                    13.0,
                    if can_save {
                        iced::Color::WHITE
                    } else {
                        theme::TEXT_DISABLED
                    }
                ),
                text("Save").size(theme::TEXT_MD).font(theme::MEDIUM),
                components::kbd(components::command_key("S")),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([5, 10])
        .on_press_maybe(can_save.then_some(Message::SaveActiveEditor))
        .style(if can_save {
            theme::primary_button
        } else {
            theme::secondary_button
        }),
    );

    let title = row![
        text(&document.path)
            .size(theme::TEXT_SM)
            .font(theme::MONO_FONT)
            .color(theme::TEXT_MUTED)
            .wrapping(text::Wrapping::None),
    ];

    container(
        row![
            container(title).width(Length::Fill).clip(true),
            text(status).size(theme::TEXT_SM).color(status_color),
            actions,
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .padding([0, 14])
    .height(theme::HEADER_HEIGHT)
    .center_y(theme::HEADER_HEIGHT)
    .width(Length::Fill)
    .style(theme::bar)
    .into()
}

/// Status bar content shown while an editor tab is active.
pub fn status_items(state: &AppState) -> Element<'_, Message> {
    let Some(document) = state.active_editor() else {
        return Space::new().into();
    };
    if document.is_loading || document.load_error.is_some() {
        return Space::new().into();
    }

    let (line, column) = document.cursor_position();
    row![
        text(format!("Ln {line}, Col {column}"))
            .size(theme::TEXT_XS)
            .color(theme::TEXT_FAINT),
        text(format!("{} lines", document.buffer.line_count().max(1)))
            .size(theme::TEXT_XS)
            .color(theme::TEXT_FAINT),
        text(components::format_bytes(document.byte_len as u64))
            .size(theme::TEXT_XS)
            .color(theme::TEXT_FAINT),
        Space::new().width(Length::Fill),
        text(document.language.label())
            .size(theme::TEXT_XS)
            .color(theme::TEXT_MUTED),
        text("UTF-8").size(theme::TEXT_XS).color(theme::TEXT_FAINT),
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .into()
}

fn placeholder<'a>(
    glyph: text::Text<'a>,
    title: &'a str,
    detail: Option<&'a str>,
) -> Element<'a, Message> {
    let mut content = column![
        glyph,
        text(title)
            .size(theme::TEXT_LG)
            .font(theme::MEDIUM)
            .color(theme::TEXT),
    ]
    .spacing(10)
    .align_x(Alignment::Center)
    .max_width(420);

    if let Some(detail) = detail {
        content = content.push(
            text(detail)
                .size(theme::TEXT_MD)
                .color(theme::TEXT_MUTED)
                .align_x(Alignment::Center),
        );
    }

    container(content).center(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use iced::widget::text_editor;

    use crate::models::{EditorDocument, EditorLanguage};

    #[test]
    fn editor_document_becomes_dirty_after_text_action() {
        let mut document = EditorDocument::new_loading("/srv/app/src/main.rs");
        document.apply_content("fn main() {}\n".into());
        document.apply_action(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        document.apply_action(text_editor::Action::Edit(text_editor::Edit::Insert('/')));

        assert!(document.is_dirty);
    }

    #[test]
    fn editor_language_exposes_syntax_token() {
        assert_eq!(EditorLanguage::Rust.syntax_token(), Some("rust"));
        assert_eq!(EditorLanguage::PlainText.syntax_token(), None);
    }

    #[test]
    fn reports_line_count_from_buffer() {
        let mut document = EditorDocument::new_loading("/srv/app/src/main.rs");
        document.apply_content("fn main() {}\n".into());
        document.apply_action(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        document.apply_action(text_editor::Action::Edit(text_editor::Edit::Enter));

        assert_eq!(document.buffer.line_count(), 3);
    }
}
