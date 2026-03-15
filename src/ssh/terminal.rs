use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalStyleSpan {
    pub text: String,
    pub foreground: Option<(u8, u8, u8)>,
    pub background: Option<(u8, u8, u8)>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

pub struct TerminalBuffer {
    parser: vt100::Parser,
    rows: u16,
    cols: u16,
}

impl std::fmt::Debug for TerminalBuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalBuffer")
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .finish()
    }
}

impl Default for TerminalBuffer {
    fn default() -> Self {
        Self::new(48, 140)
    }
}

impl TerminalBuffer {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 10_000),
            rows,
            cols,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        self.parser.screen_mut().set_size(rows, cols);
    }

    pub fn clear(&mut self) {
        self.parser = vt100::Parser::new(self.rows, self.cols, 10_000);
    }

    pub fn display_text(&self) -> String {
        self.parser.screen().contents().to_string()
    }

    pub fn styled_spans(&self) -> Vec<TerminalStyleSpan> {
        self.styled_spans_internal(false)
    }

    pub fn styled_spans_with_cursor(&self, show_cursor: bool) -> Vec<TerminalStyleSpan> {
        self.styled_spans_internal(show_cursor)
    }

    fn styled_spans_internal(&self, show_cursor: bool) -> Vec<TerminalStyleSpan> {
        let screen = self.parser.screen();
        let cursor = if show_cursor && !screen.hide_cursor() {
            Some(screen.cursor_position())
        } else {
            None
        };

        let last_rendered_row = (0..self.rows)
            .rev()
            .find(|row| row_has_renderable_cells(screen, *row, self.cols));
        let last_row = match (last_rendered_row, cursor) {
            (Some(row), Some((cursor_row, _))) => row.max(cursor_row),
            (Some(row), None) => row,
            (None, Some((cursor_row, _))) => cursor_row,
            (None, None) => return Vec::new(),
        };

        let mut spans = Vec::new();

        for row in 0..=last_row {
            let last_rendered_col = (0..self.cols)
                .rev()
                .find(|col| screen.cell(row, *col).is_some_and(cell_is_renderable));
            let last_col = match (last_rendered_col, cursor) {
                (Some(col), Some((cursor_row, cursor_col))) if cursor_row == row => {
                    col.max(cursor_col)
                }
                (None, Some((cursor_row, cursor_col))) if cursor_row == row => cursor_col,
                (Some(col), _) => col,
                (None, _) => {
                    if row < last_row {
                        spans.push(TerminalStyleSpan {
                            text: "\n".into(),
                            ..Default::default()
                        });
                    }
                    continue;
                }
            };

            let mut current_span: Option<TerminalStyleSpan> = None;

            for col in 0..=last_col {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };

                if cell.is_wide_continuation() {
                    continue;
                }

                let mut style = style_for_cell(cell);
                if cursor == Some((row, col)) {
                    style = style_with_cursor(style);
                }
                let text = if cell.has_contents() {
                    cell.contents()
                } else {
                    " "
                };

                match &mut current_span {
                    Some(span) if same_style(span, &style) => span.text.push_str(text),
                    Some(span) => {
                        spans.push(std::mem::take(span));
                        *span = style;
                        span.text.push_str(text);
                    }
                    None => {
                        let mut span = style;
                        span.text.push_str(text);
                        current_span = Some(span);
                    }
                }
            }

            if let Some(span) = current_span.take() {
                spans.push(span);
            }

            if row < last_row {
                spans.push(TerminalStyleSpan {
                    text: "\n".into(),
                    ..Default::default()
                });
            }
        }

        spans
    }

    /// Returns the text on the row where the cursor currently sits.
    /// Used to detect `cd` commands for SFTP explorer sync after Enter is pressed.
    pub fn current_cursor_line(&self) -> String {
        let screen = self.parser.screen();
        let (cursor_row, _) = screen.cursor_position();
        let mut line = String::new();
        for col in 0..self.cols {
            let cell = screen.cell(cursor_row, col);
            if let Some(cell) = cell {
                line.push_str(&cell.contents());
            }
        }
        line.trim_end().to_string()
    }
}

/// Strips common shell prompt patterns to isolate the typed command.
/// Handles prompts like `user@host:~$ cmd`, `root# cmd`, `> cmd`.
pub fn extract_command_from_prompt_line(line: &str) -> &str {
    if let Some(pos) = line.rfind("$ ") {
        return line[pos + 2..].trim();
    }
    if let Some(pos) = line.rfind("# ") {
        return line[pos + 2..].trim();
    }
    if let Some(pos) = line.rfind("> ") {
        return line[pos + 2..].trim();
    }
    line.trim()
}

fn row_has_renderable_cells(screen: &vt100::Screen, row: u16, cols: u16) -> bool {
    (0..cols).any(|col| screen.cell(row, col).is_some_and(cell_is_renderable))
}

fn cell_is_renderable(cell: &vt100::Cell) -> bool {
    cell.has_contents()
        || cell.bgcolor() != vt100::Color::Default
        || cell.fgcolor() != vt100::Color::Default
        || cell.bold()
        || cell.dim()
        || cell.italic()
        || cell.underline()
        || cell.inverse()
}

fn style_for_cell(cell: &vt100::Cell) -> TerminalStyleSpan {
    let mut foreground = vt100_color_to_rgb(cell.fgcolor());
    let mut background = vt100_color_to_rgb(cell.bgcolor());

    if cell.inverse() {
        std::mem::swap(&mut foreground, &mut background);
    }

    TerminalStyleSpan {
        text: String::new(),
        foreground,
        background,
        bold: cell.bold(),
        dim: cell.dim(),
        italic: cell.italic(),
        underline: cell.underline(),
    }
}

fn same_style(span: &TerminalStyleSpan, style: &TerminalStyleSpan) -> bool {
    span.foreground == style.foreground
        && span.background == style.background
        && span.bold == style.bold
        && span.dim == style.dim
        && span.italic == style.italic
        && span.underline == style.underline
}

fn style_with_cursor(mut style: TerminalStyleSpan) -> TerminalStyleSpan {
    let cursor_background = style.foreground.unwrap_or((0xcb, 0xd5, 0xe1));
    let cursor_foreground = style.background.unwrap_or((0x0f, 0x17, 0x2a));

    style.foreground = Some(cursor_foreground);
    style.background = Some(cursor_background);
    style
}

fn vt100_color_to_rgb(color: vt100::Color) -> Option<(u8, u8, u8)> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Rgb(red, green, blue) => Some((red, green, blue)),
        vt100::Color::Idx(index) => Some(xterm_index_to_rgb(index)),
    }
}

fn xterm_index_to_rgb(index: u8) -> (u8, u8, u8) {
    const ANSI: [(u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00),
        (0xcd, 0x00, 0x00),
        (0x00, 0xcd, 0x00),
        (0xcd, 0xcd, 0x00),
        (0x00, 0x00, 0xee),
        (0xcd, 0x00, 0xcd),
        (0x00, 0xcd, 0xcd),
        (0xe5, 0xe5, 0xe5),
        (0x7f, 0x7f, 0x7f),
        (0xff, 0x00, 0x00),
        (0x00, 0xff, 0x00),
        (0xff, 0xff, 0x00),
        (0x5c, 0x5c, 0xff),
        (0xff, 0x00, 0xff),
        (0x00, 0xff, 0xff),
        (0xff, 0xff, 0xff),
    ];

    match index {
        0..=15 => ANSI[index as usize],
        16..=231 => {
            let index = index - 16;
            let red = index / 36;
            let green = (index % 36) / 6;
            let blue = index % 6;
            (
                color_cube_component(red),
                color_cube_component(green),
                color_cube_component(blue),
            )
        }
        232..=255 => {
            let value = 8 + (index - 232) * 10;
            (value, value, value)
        }
    }
}

fn color_cube_component(value: u8) -> u8 {
    match value {
        0 => 0,
        _ => 55 + value * 40,
    }
}

/// Translates an Iced keyboard event into the byte sequence expected by an
/// xterm PTY. Returns `None` for keys that should not produce output
/// (modifier-only presses, unrecognised combinations).
pub fn key_to_bytes(key: &Key, modifiers: Modifiers, text: Option<&str>) -> Option<Vec<u8>> {
    // Ctrl+Shift combinations are handled by the caller (copy, paste).
    // Here we only handle Ctrl+<letter> → control codes.
    if modifiers.control() && !modifiers.shift() && !modifiers.alt() {
        if let Key::Character(ch) = key {
            let ch = ch.chars().next()?;
            if ch.is_ascii_alphabetic() {
                let code = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
                return Some(vec![code]);
            }
            return match ch {
                '[' => Some(vec![0x1B]),
                '\\' => Some(vec![0x1C]),
                ']' => Some(vec![0x1D]),
                _ => None,
            };
        }
    }

    // Named keys → escape sequences / control codes.
    if let Key::Named(named) = key {
        return match named {
            Named::Enter => Some(vec![b'\r']),
            Named::Tab => Some(vec![b'\t']),
            Named::Backspace => Some(vec![0x7F]),
            Named::Delete => Some(b"\x1B[3~".to_vec()),
            Named::Escape => Some(vec![0x1B]),
            Named::ArrowUp => Some(b"\x1B[A".to_vec()),
            Named::ArrowDown => Some(b"\x1B[B".to_vec()),
            Named::ArrowRight => Some(b"\x1B[C".to_vec()),
            Named::ArrowLeft => Some(b"\x1B[D".to_vec()),
            Named::Home => Some(b"\x1B[H".to_vec()),
            Named::End => Some(b"\x1B[F".to_vec()),
            Named::PageUp => Some(b"\x1B[5~".to_vec()),
            Named::PageDown => Some(b"\x1B[6~".to_vec()),
            Named::Insert => Some(b"\x1B[2~".to_vec()),
            Named::Space => Some(vec![b' ']),
            Named::F1 => Some(b"\x1BOP".to_vec()),
            Named::F2 => Some(b"\x1BOQ".to_vec()),
            Named::F3 => Some(b"\x1BOR".to_vec()),
            Named::F4 => Some(b"\x1BOS".to_vec()),
            Named::F5 => Some(b"\x1B[15~".to_vec()),
            Named::F6 => Some(b"\x1B[17~".to_vec()),
            Named::F7 => Some(b"\x1B[18~".to_vec()),
            Named::F8 => Some(b"\x1B[19~".to_vec()),
            Named::F9 => Some(b"\x1B[20~".to_vec()),
            Named::F10 => Some(b"\x1B[21~".to_vec()),
            Named::F11 => Some(b"\x1B[23~".to_vec()),
            Named::F12 => Some(b"\x1B[24~".to_vec()),
            // Modifier-only keys produce no output.
            Named::Shift | Named::Control | Named::Alt | Named::Super
            | Named::CapsLock | Named::NumLock | Named::ScrollLock => None,
            _ => None,
        };
    }

    // Alt+key → ESC prefix + character (Meta key convention).
    if modifiers.alt() && !modifiers.control() {
        if let Some(t) = text {
            if !t.is_empty() {
                let mut bytes = vec![0x1B];
                bytes.extend_from_slice(t.as_bytes());
                return Some(bytes);
            }
        }
    }

    // Regular character input — use the `text` field from Iced which
    // already accounts for Shift, keyboard layout, dead keys, etc.
    if !modifiers.control() && !modifiers.alt() {
        if let Some(t) = text {
            if !t.is_empty() {
                return Some(t.as_bytes().to_vec());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_sends_etx() {
        let bytes = key_to_bytes(
            &Key::Character("c".into()),
            Modifiers::CTRL,
            Some("c"),
        );
        assert_eq!(bytes, Some(vec![0x03]));
    }

    #[test]
    fn enter_sends_carriage_return() {
        let bytes = key_to_bytes(&Key::Named(Named::Enter), Modifiers::empty(), None);
        assert_eq!(bytes, Some(vec![b'\r']));
    }

    #[test]
    fn arrow_up_sends_escape_sequence() {
        let bytes = key_to_bytes(&Key::Named(Named::ArrowUp), Modifiers::empty(), None);
        assert_eq!(bytes, Some(b"\x1B[A".to_vec()));
    }

    #[test]
    fn regular_character() {
        let bytes = key_to_bytes(
            &Key::Character("a".into()),
            Modifiers::empty(),
            Some("a"),
        );
        assert_eq!(bytes, Some(vec![b'a']));
    }

    #[test]
    fn modifier_only_keys_produce_no_output() {
        let bytes = key_to_bytes(&Key::Named(Named::Shift), Modifiers::SHIFT, None);
        assert_eq!(bytes, None);
    }

    #[test]
    fn extracts_command_from_prompt() {
        assert_eq!(extract_command_from_prompt_line("user@host:~$ cd /tmp"), "cd /tmp");
        assert_eq!(extract_command_from_prompt_line("root# ls -la"), "ls -la");
        assert_eq!(extract_command_from_prompt_line("> echo hello"), "echo hello");
        assert_eq!(extract_command_from_prompt_line("ls"), "ls");
    }

    #[test]
    fn preserves_ansi_colors_in_spans() {
        let mut buffer = TerminalBuffer::new(4, 20);
        buffer.feed(b"\x1b[31mred\x1b[0m plain");

        let spans = buffer.styled_spans();

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "red");
        assert_eq!(spans[0].foreground, Some((0xcd, 0x00, 0x00)));
        assert_eq!(spans[1].text, " plain");
        assert_eq!(spans[1].foreground, None);
    }

    #[test]
    fn renders_cursor_on_empty_terminal() {
        let buffer = TerminalBuffer::new(2, 4);

        let spans = buffer.styled_spans_with_cursor(true);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, " ");
        assert_eq!(spans[0].background, Some((0xcb, 0xd5, 0xe1)));
        assert_eq!(spans[0].foreground, Some((0x0f, 0x17, 0x2a)));
    }

    #[test]
    fn styled_spans_match_plain_text_output() {
        let mut buffer = TerminalBuffer::new(4, 20);
        buffer.feed(b"first\n\nthird");

        let text = buffer
            .styled_spans()
            .into_iter()
            .map(|span| span.text)
            .collect::<String>();

        assert_eq!(text, buffer.display_text());
    }
}
