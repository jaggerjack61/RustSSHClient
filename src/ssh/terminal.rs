use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

const SCROLLBACK_LINES: usize = 10_000;

/// Default foreground and background of the terminal, also used to draw the
/// cursor on cells without explicit colors.
pub const DEFAULT_FOREGROUND: (u8, u8, u8) = (0xd4, 0xda, 0xe3);
pub const DEFAULT_BACKGROUND: (u8, u8, u8) = (0x0b, 0x0e, 0x14);

/// The 16 base ANSI colors, tuned for the dark UI.
const ANSI_PALETTE: [(u8, u8, u8); 16] = [
    (0x1f, 0x24, 0x2e), // black
    (0xf8, 0x71, 0x71), // red
    (0x4a, 0xde, 0x80), // green
    (0xfa, 0xcc, 0x15), // yellow
    (0x60, 0xa5, 0xfa), // blue
    (0xc0, 0x84, 0xfc), // magenta
    (0x22, 0xd3, 0xee), // cyan
    (0xcb, 0xd5, 0xe1), // white
    (0x64, 0x74, 0x8b), // bright black
    (0xfc, 0xa5, 0xa5), // bright red
    (0x86, 0xef, 0xac), // bright green
    (0xfd, 0xe0, 0x47), // bright yellow
    (0x93, 0xc5, 0xfd), // bright blue
    (0xd8, 0xb4, 0xfe), // bright magenta
    (0x67, 0xe8, 0xf9), // bright cyan
    (0xf8, 0xfa, 0xfc), // bright white
];

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
    /// Bumped on every visible change so views can cache rendered output.
    generation: u64,
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
        Self::new(36, 120)
    }
}

impl TerminalBuffer {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, SCROLLBACK_LINES),
            rows,
            cols,
            generation: 0,
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
        self.generation = self.generation.wrapping_add(1);
    }

    /// Resizes the grid. Returns `false` when the size did not change.
    pub fn resize(&mut self, rows: u16, cols: u16) -> bool {
        if (rows, cols) == (self.rows, self.cols) {
            return false;
        }

        self.rows = rows;
        self.cols = cols;
        self.parser.screen_mut().set_size(rows, cols);
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub fn clear(&mut self) {
        self.parser = vt100::Parser::new(self.rows, self.cols, SCROLLBACK_LINES);
        self.generation = self.generation.wrapping_add(1);
    }

    /// Lines scrolled back from the live screen (0 when following output).
    pub fn scroll_offset(&self) -> usize {
        self.parser.screen().scrollback()
    }

    /// Scrolls by `lines` (positive = towards older output). Returns whether
    /// the view moved.
    pub fn scroll_by(&mut self, lines: i32) -> bool {
        let current = self.scroll_offset();
        let target = if lines >= 0 {
            current.saturating_add(lines as usize)
        } else {
            current.saturating_sub(lines.unsigned_abs() as usize)
        };
        self.parser.screen_mut().set_scrollback(target);

        let moved = self.scroll_offset() != current;
        if moved {
            self.generation = self.generation.wrapping_add(1);
        }
        moved
    }

    pub fn scroll_to_bottom(&mut self) -> bool {
        if self.scroll_offset() == 0 {
            return false;
        }

        self.parser.screen_mut().set_scrollback(0);
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub fn application_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    pub fn alternate_screen(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    pub fn bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste()
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
        let cursor = if show_cursor && !screen.hide_cursor() && screen.scrollback() == 0 {
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
            if let Some(cell) = screen.cell(cursor_row, col) {
                line.push_str(cell.contents());
            }
        }
        line.trim_end().to_string()
    }
}

/// Encodes clipboard text for the PTY. Newlines become carriage returns (as
/// a real terminal sends them) and bracketed paste is honored so shells and
/// editors don't execute pasted text line by line.
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");

    if bracketed {
        // Strip any embedded end marker so pasted text can't escape the bracket.
        let sanitized = normalized.replace("\x1b[201~", "");
        let mut bytes = Vec::with_capacity(sanitized.len() + 12);
        bytes.extend_from_slice(b"\x1b[200~");
        bytes.extend_from_slice(sanitized.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        bytes
    } else {
        normalized.into_bytes()
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
    let mut foreground = vt100_color_to_rgb(cell.fgcolor(), cell.bold());
    let mut background = vt100_color_to_rgb(cell.bgcolor(), false);

    if cell.inverse() {
        std::mem::swap(&mut foreground, &mut background);
        foreground.get_or_insert(DEFAULT_BACKGROUND);
        background.get_or_insert(DEFAULT_FOREGROUND);
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
    let cursor_background = style.foreground.unwrap_or(DEFAULT_FOREGROUND);
    let cursor_foreground = style.background.unwrap_or(DEFAULT_BACKGROUND);

    style.foreground = Some(cursor_foreground);
    style.background = Some(cursor_background);
    style
}

fn vt100_color_to_rgb(color: vt100::Color, bold: bool) -> Option<(u8, u8, u8)> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Rgb(red, green, blue) => Some((red, green, blue)),
        // Classic terminals render bold text in the bright variant.
        vt100::Color::Idx(index) if bold && index < 8 => Some(xterm_index_to_rgb(index + 8)),
        vt100::Color::Idx(index) => Some(xterm_index_to_rgb(index)),
    }
}

fn xterm_index_to_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        0..=15 => ANSI_PALETTE[index as usize],
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
/// (modifier-only presses, OS shortcuts, unrecognised combinations).
///
/// `application_cursor` reflects DECCKM, which full-screen programs such as
/// vim and less enable to receive SS3-style cursor keys.
pub fn key_to_bytes(
    key: &Key,
    modifiers: Modifiers,
    text: Option<&str>,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    // Cmd (macOS) / Win (Windows) / Super (Linux) combinations are OS or app
    // shortcuts and must never type into the shell.
    if modifiers.logo() {
        return None;
    }

    // Ctrl+<key> → control codes. Ctrl+Shift combinations are app shortcuts
    // (copy/paste) and handled by the caller.
    if modifiers.control() && !modifiers.shift() && !modifiers.alt() {
        if let Key::Character(ch) = key {
            let ch = ch.chars().next()?;
            if ch.is_ascii_alphabetic() {
                let code = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
                return Some(vec![code]);
            }
            return match ch {
                '@' | '2' => Some(vec![0x00]),
                '[' | '3' => Some(vec![0x1B]),
                '\\' | '4' => Some(vec![0x1C]),
                ']' | '5' => Some(vec![0x1D]),
                '^' | '6' => Some(vec![0x1E]),
                '_' | '-' | '/' | '7' => Some(vec![0x1F]),
                '8' => Some(vec![0x7F]),
                _ => None,
            };
        }

        if let Key::Named(Named::Space) = key {
            return Some(vec![0x00]);
        }
    }

    // Named keys → escape sequences / control codes.
    if let Key::Named(named) = key {
        let cursor = |normal: &'static [u8], application: &'static [u8]| {
            Some(
                if application_cursor {
                    application
                } else {
                    normal
                }
                .to_vec(),
            )
        };

        let bytes = match named {
            Named::Enter => Some(vec![b'\r']),
            Named::Tab if modifiers.shift() => Some(b"\x1B[Z".to_vec()),
            Named::Tab => Some(vec![b'\t']),
            Named::Backspace if modifiers.control() => Some(vec![0x08]),
            Named::Backspace => Some(vec![0x7F]),
            Named::Delete => Some(b"\x1B[3~".to_vec()),
            Named::Escape => Some(vec![0x1B]),
            Named::ArrowUp => cursor(b"\x1B[A", b"\x1BOA"),
            Named::ArrowDown => cursor(b"\x1B[B", b"\x1BOB"),
            Named::ArrowRight => cursor(b"\x1B[C", b"\x1BOC"),
            Named::ArrowLeft => cursor(b"\x1B[D", b"\x1BOD"),
            Named::Home => cursor(b"\x1B[H", b"\x1BOH"),
            Named::End => cursor(b"\x1B[F", b"\x1BOF"),
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
            _ => None,
        };

        // Alt+<named key> → ESC prefix (e.g. Alt+Backspace deletes a word).
        return match bytes {
            Some(mut bytes) if modifiers.alt() && !modifiers.control() => {
                bytes.insert(0, 0x1B);
                Some(bytes)
            }
            other => other,
        };
    }

    // Alt+key → ESC prefix + character (Meta key convention).
    if modifiers.alt()
        && !modifiers.control()
        && let Some(t) = text.filter(|t| !t.is_empty())
    {
        let mut bytes = vec![0x1B];
        bytes.extend_from_slice(t.as_bytes());
        return Some(bytes);
    }

    // Regular character input — use the `text` field from Iced which
    // already accounts for Shift, keyboard layout, dead keys, etc.
    if !modifiers.control()
        && !modifiers.alt()
        && let Some(t) = text.filter(|t| !t.is_empty())
    {
        return Some(t.as_bytes().to_vec());
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
            false,
        );
        assert_eq!(bytes, Some(vec![0x03]));
    }

    #[test]
    fn enter_sends_carriage_return() {
        let bytes = key_to_bytes(&Key::Named(Named::Enter), Modifiers::empty(), None, false);
        assert_eq!(bytes, Some(vec![b'\r']));
    }

    #[test]
    fn arrow_up_sends_escape_sequence() {
        let bytes = key_to_bytes(&Key::Named(Named::ArrowUp), Modifiers::empty(), None, false);
        assert_eq!(bytes, Some(b"\x1B[A".to_vec()));
    }

    #[test]
    fn arrow_keys_follow_application_cursor_mode() {
        let bytes = key_to_bytes(&Key::Named(Named::ArrowUp), Modifiers::empty(), None, true);
        assert_eq!(bytes, Some(b"\x1BOA".to_vec()));
    }

    #[test]
    fn shift_tab_sends_back_tab() {
        let bytes = key_to_bytes(&Key::Named(Named::Tab), Modifiers::SHIFT, None, false);
        assert_eq!(bytes, Some(b"\x1B[Z".to_vec()));
    }

    #[test]
    fn logo_shortcuts_do_not_type_into_the_shell() {
        let bytes = key_to_bytes(
            &Key::Character("c".into()),
            Modifiers::LOGO,
            Some("c"),
            false,
        );
        assert_eq!(bytes, None);
    }

    #[test]
    fn regular_character() {
        let bytes = key_to_bytes(
            &Key::Character("a".into()),
            Modifiers::empty(),
            Some("a"),
            false,
        );
        assert_eq!(bytes, Some(vec![b'a']));
    }

    #[test]
    fn modifier_only_keys_produce_no_output() {
        let bytes = key_to_bytes(&Key::Named(Named::Shift), Modifiers::SHIFT, None, false);
        assert_eq!(bytes, None);
    }

    #[test]
    fn paste_uses_carriage_returns_and_brackets() {
        assert_eq!(paste_bytes("a\nb\r\nc", false), b"a\rb\rc".to_vec());
        assert_eq!(
            paste_bytes("ls\n", true),
            b"\x1b[200~ls\r\x1b[201~".to_vec()
        );
        assert_eq!(
            paste_bytes("x\x1b[201~rm", true),
            b"\x1b[200~xrm\x1b[201~".to_vec()
        );
    }

    #[test]
    fn extracts_command_from_prompt() {
        assert_eq!(
            extract_command_from_prompt_line("user@host:~$ cd /tmp"),
            "cd /tmp"
        );
        assert_eq!(extract_command_from_prompt_line("root# ls -la"), "ls -la");
        assert_eq!(
            extract_command_from_prompt_line("> echo hello"),
            "echo hello"
        );
        assert_eq!(extract_command_from_prompt_line("ls"), "ls");
    }

    #[test]
    fn preserves_ansi_colors_in_spans() {
        let mut buffer = TerminalBuffer::new(4, 20);
        buffer.feed(b"\x1b[31mred\x1b[0m plain");

        let spans = buffer.styled_spans();

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "red");
        assert_eq!(spans[0].foreground, Some(ANSI_PALETTE[1]));
        assert_eq!(spans[1].text, " plain");
        assert_eq!(spans[1].foreground, None);
    }

    #[test]
    fn renders_cursor_on_empty_terminal() {
        let buffer = TerminalBuffer::new(2, 4);

        let spans = buffer.styled_spans_with_cursor(true);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, " ");
        assert_eq!(spans[0].background, Some(DEFAULT_FOREGROUND));
        assert_eq!(spans[0].foreground, Some(DEFAULT_BACKGROUND));
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

    #[test]
    fn scrolls_back_through_history_and_hides_cursor() {
        let mut buffer = TerminalBuffer::new(2, 10);
        buffer.feed(b"one\r\ntwo\r\nthree\r\nfour");

        assert!(buffer.scroll_by(1));
        assert_eq!(buffer.scroll_offset(), 1);
        let text = buffer
            .styled_spans_with_cursor(true)
            .into_iter()
            .map(|span| span.text)
            .collect::<String>();
        assert_eq!(text, "two\nthree");

        assert!(!buffer.scroll_by(100) || buffer.scroll_offset() == 2);
        assert!(buffer.scroll_to_bottom());
        assert_eq!(buffer.scroll_offset(), 0);
    }

    #[test]
    fn resize_reports_changes_only() {
        let mut buffer = TerminalBuffer::new(10, 40);
        assert!(!buffer.resize(10, 40));
        assert!(buffer.resize(12, 40));
        assert_eq!(buffer.size(), (12, 40));
    }
}
