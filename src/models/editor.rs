use std::path::Path;

use iced::widget::text_editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorLanguage {
    PlainText,
    Rust,
    Shell,
    Json,
    Markdown,
    Toml,
    Yaml,
    JavaScript,
    TypeScript,
    Python,
    Html,
    Css,
    Xml,
    Ini,
    Sql,
}

impl EditorLanguage {
    pub fn from_path(path: &str) -> Self {
        let file_name = Path::new(path)
            .file_name()
            .map(|value| value.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_else(|| path.to_ascii_lowercase());

        match file_name.as_str() {
            "dockerfile" | "makefile" | ".gitignore" | ".dockerignore" | ".env" => {
                Self::Shell
            }
            "cargo.toml" => Self::Toml,
            _ => match Path::new(&file_name)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase())
                .as_deref()
            {
                Some("rs") => Self::Rust,
                Some("sh") | Some("bash") | Some("zsh") | Some("fish") => Self::Shell,
                Some("json") => Self::Json,
                Some("md") | Some("markdown") | Some("rst") => Self::Markdown,
                Some("toml") | Some("lock") => Self::Toml,
                Some("yaml") | Some("yml") => Self::Yaml,
                Some("js") | Some("mjs") | Some("cjs") => Self::JavaScript,
                Some("ts") | Some("tsx") => Self::TypeScript,
                Some("py") => Self::Python,
                Some("html") | Some("htm") => Self::Html,
                Some("css") | Some("scss") | Some("less") => Self::Css,
                Some("xml") | Some("svg") => Self::Xml,
                Some("ini") | Some("cfg") | Some("conf") | Some("properties") => Self::Ini,
                Some("sql") => Self::Sql,
                _ => Self::PlainText,
            },
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PlainText => "Plain Text",
            Self::Rust => "Rust",
            Self::Shell => "Shell",
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Python => "Python",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Xml => "XML",
            Self::Ini => "Config",
            Self::Sql => "SQL",
        }
    }

    pub fn syntax_token(self) -> Option<&'static str> {
        match self {
            Self::PlainText => None,
            Self::Rust => Some("rust"),
            Self::Shell => Some("shell"),
            Self::Json => Some("json"),
            Self::Markdown => Some("markdown"),
            Self::Toml => Some("toml"),
            Self::Yaml => Some("yaml"),
            Self::JavaScript => Some("javascript"),
            Self::TypeScript => Some("typescript"),
            Self::Python => Some("python"),
            Self::Html => Some("html"),
            Self::Css => Some("css"),
            Self::Xml => Some("xml"),
            Self::Ini => Some("ini"),
            Self::Sql => Some("sql"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorDocument {
    pub path: String,
    pub title: String,
    pub language: EditorLanguage,
    pub buffer: text_editor::Content,
    pub saved_content: String,
    pub is_loading: bool,
    pub is_saving: bool,
    pub is_dirty: bool,
    pub load_error: Option<String>,
}

impl EditorDocument {
    pub fn new_loading(path: impl Into<String>) -> Self {
        let path = path.into();
        let title = editor_title(&path);

        Self {
            language: EditorLanguage::from_path(&path),
            path,
            title,
            buffer: text_editor::Content::new(),
            saved_content: String::new(),
            is_loading: true,
            is_saving: false,
            is_dirty: false,
            load_error: None,
        }
    }

    pub fn apply_content(&mut self, content: String) {
        self.buffer = text_editor::Content::with_text(&content);
        self.saved_content = content;
        self.is_loading = false;
        self.is_saving = false;
        self.is_dirty = false;
        self.load_error = None;
    }

    pub fn set_error(&mut self, error: String) {
        self.buffer = text_editor::Content::new();
        self.saved_content.clear();
        self.is_loading = false;
        self.is_saving = false;
        self.is_dirty = false;
        self.load_error = Some(error);
    }

    pub fn apply_action(&mut self, action: text_editor::Action) {
        self.buffer.perform(action);
        self.is_dirty = self.buffer.text() != self.saved_content;
    }

    pub fn mark_saving(&mut self) {
        self.is_saving = true;
    }

    pub fn mark_saved(&mut self) {
        self.saved_content = self.buffer.text();
        self.is_saving = false;
        self.is_dirty = false;
    }

    pub fn mark_save_failed(&mut self) {
        self.is_saving = false;
        self.is_dirty = self.buffer.text() != self.saved_content;
    }

    pub fn current_text(&self) -> String {
        self.buffer.text()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceTab {
    Terminal,
    Editor(String),
}

pub fn editor_title(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use iced::widget::text_editor;

    use super::{EditorDocument, EditorLanguage, editor_title};

    #[test]
    fn infers_editor_language_from_path() {
        assert_eq!(EditorLanguage::from_path("/srv/app/main.rs"), EditorLanguage::Rust);
        assert_eq!(EditorLanguage::from_path("/srv/app/package.json"), EditorLanguage::Json);
        assert_eq!(EditorLanguage::from_path("/srv/app/Dockerfile"), EditorLanguage::Shell);
    }

    #[test]
    fn derives_editor_title_from_file_name() {
        assert_eq!(editor_title("/srv/app/settings.toml"), "settings.toml");
    }

    #[test]
    fn marks_editor_dirty_after_edits() {
        let mut document = EditorDocument::new_loading("/srv/app/Procfile");
        document.apply_content("web: serve\n".into());
        document.apply_action(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        document.apply_action(text_editor::Action::Edit(text_editor::Edit::Insert('!')));

        assert!(document.is_dirty);
        assert_eq!(document.current_text(), "web: serve\n!");
    }
}