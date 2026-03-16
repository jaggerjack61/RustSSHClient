use iced::widget::{button, column, container, markdown, row, scrollable, text, text_editor, Space};
use iced::{Color, Element, Length, Theme};

use crate::app::messages::Message;
use crate::app::state::AppState;
use crate::models::{EditorDocument, EditorLanguage};

use super::styles;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let Some(document) = state.active_editor() else {
        return container(text("No editor tab is active.").color(styles::text_slate_500()))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(styles::terminal_area)
            .into();
    };

    let can_save = !document.is_loading && !document.is_saving && document.is_dirty;
    let status_text = if document.is_dirty {
        "Unsaved changes"
    } else if document.is_saving {
        "Saving..."
    } else {
        "Saved"
    };
    let status_color = if document.is_dirty {
        styles::orange_400()
    } else if document.is_saving {
        styles::text_slate_500()
    } else {
        styles::emerald_400()
    };

    let is_markdown = document.language == EditorLanguage::Markdown;

    let save_button = button(
        text(if document.is_saving { "Saving" } else { "Save" })
            .size(12)
            .color(if document.is_saving {
                styles::text_slate_500()
            } else {
                Color::WHITE
            }),
    )
    .on_press_maybe(can_save.then_some(Message::SaveActiveEditor))
    .padding([6, 14])
    .style(if can_save {
        styles::primary_button
    } else {
        styles::ghost_button
    });

    let header_row: Element<'_, Message> = if is_markdown {
        let preview_label = if document.markdown_preview { "Edit" } else { "Preview" };
        let preview_button = button(
            text(preview_label)
                .size(12)
                .color(Color::WHITE),
        )
        .on_press(Message::ToggleMarkdownPreview)
        .padding([6, 14])
        .style(if document.markdown_preview {
            styles::primary_button
        } else {
            styles::ghost_button
        });

        row![
            column![
                text(&document.title).size(14).color(Color::WHITE),
                text(&document.path)
                    .size(11)
                    .color(styles::text_slate_500())
                    .width(Length::Fill),
            ]
            .spacing(3)
            .width(Length::FillPortion(3)),
            Space::new().width(Length::Fill),
            column![
                text(status_text)
                    .size(11)
                    .color(status_color),
                text(document.language.label())
                    .size(11)
                    .color(styles::blue_400()),
            ]
            .spacing(2)
            .align_x(iced::Alignment::End),
            preview_button,
            save_button,
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        row![
            column![
                text(&document.title).size(14).color(Color::WHITE),
                text(&document.path)
                    .size(11)
                    .color(styles::text_slate_500())
                    .width(Length::Fill),
            ]
            .spacing(3)
            .width(Length::FillPortion(3)),
            Space::new().width(Length::Fill),
            column![
                text(status_text)
                    .size(11)
                    .color(status_color),
                text(document.language.label())
                    .size(11)
                    .color(styles::blue_400()),
            ]
            .spacing(2)
            .align_x(iced::Alignment::End),
            save_button,
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .into()
    };

    let header = container(header_row)
        .padding([10, 16])
        .width(Length::Fill)
        .style(styles::editor_header);

    let body: Element<'_, Message> = if document.is_loading {
        container(text("Loading remote file...").color(styles::text_slate_400()))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(styles::terminal_area)
            .into()
    } else if let Some(error) = &document.load_error {
        container(
            column![
                text("Unable to open file")
                    .size(16)
                    .color(styles::red_400()),
                text(error).size(13).color(styles::text_slate_400()),
            ]
            .spacing(8),
        )
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .style(styles::terminal_area)
        .into()
    } else if document.markdown_preview {
        let preview = markdown::view(
            &document.markdown_items,
            markdown::Settings::with_style(Theme::Dark),
        )
        .map(|url| Message::MarkdownLinkClicked(url.to_string()));

        container(
            scrollable(
                container(preview)
                    .padding([16, 24])
                    .width(Length::Fill),
            )
            .style(styles::dark_scrollable)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::terminal_area)
        .into()
    } else {
        let path = document.path.clone();
        let editor_content: Element<'_, Message> = if let Some(token) = document.language.syntax_token() {
            text_editor(&document.buffer)
                .on_action(move |action| Message::EditorAction(path.clone(), action))
                .style(styles::dark_text_editor)
                .padding([12, 14])
                .size(14)
                .height(Length::Fill)
                .highlight(token, iced::highlighter::Theme::Base16Ocean)
                .into()
        } else {
            text_editor(&document.buffer)
                .on_action(move |action| Message::EditorAction(path.clone(), action))
                .style(styles::dark_text_editor)
                .padding([12, 14])
                .size(14)
                .height(Length::Fill)
                .into()
        };

        container(
            editor_content,
        )
        .padding([16, 18])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::terminal_area)
        .into()
    };

    let footer = container(
        row![
            text(format!("{} lines", line_count(document)))
                .size(11)
                .color(styles::text_slate_500()),
            text(format!("{} bytes", document.current_text().len()))
                .size(11)
                .color(styles::text_slate_500()),
            Space::new().width(Length::Fill),
            text("Ctrl+S to save")
                .size(11)
                .color(styles::text_slate_500()),
        ]
        .spacing(14)
        .align_y(iced::Alignment::Center),
    )
    .padding([8, 16])
    .width(Length::Fill)
    .style(styles::status_bar);

    column![header, body, footer]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn line_count(document: &EditorDocument) -> usize {
    document.buffer.line_count().max(1)
}

#[cfg(test)]
mod tests {
    use iced::widget::text_editor;

    use crate::models::{EditorDocument, EditorLanguage};

    use super::line_count;

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

        assert_eq!(line_count(&document), 3);
    }
}
