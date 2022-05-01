use crate::{document::Document, Row, Terminal};
use crate::modes::mode::Mode;
use std::env;
use std::time::Duration;
use std::time::Instant;
use termion::color;
use termion::event::Key;

const STATUS_FG_COLOR: color::Rgb = color::Rgb(63, 63, 63);
const STATUS_BG_COLOR: color::Rgb = color::Rgb(239, 239, 239);
const VERSION: &str = env!["CARGO_PKG_VERSION"];

const QUIT_TIMES_WITHOUT_SAVING: u8 = 3;

#[derive(PartialEq, Clone, Copy)]
pub enum SearchDirection {
    Forward,
    Backward,
}

#[derive(Default, Clone)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}

pub struct StatusMessage {
    text: String,
    time: Instant,
}

impl From<String> for StatusMessage {
    fn from(message: String) -> Self {
        Self {
            time: Instant::now(),
            text: message,
        }
    }
}

pub struct Editor {
    mode: Mode,
    quit: bool,
    terminal: Terminal,
    cursor_pos: Pos,
    offset: Pos,
    document: Document,
    status_message: StatusMessage,
    quit_times: u8,
    highlighted_word: Option<String>, // used for searching
}

impl Editor {
    pub fn default() -> Self {
        let mut init_status = String::from("Help: | Ctrl-Q - quit | Ctrl-S - save |");
        let args: Vec<String> = env::args().collect();
        let document = if let Some(filename) = args.get(1) {
            let doc = Document::open(filename);
            if let Ok(doc) = doc {
                doc
            } else {
                init_status = format!("ERR: Failed to open file: {}", args[1]);
                Document::default()
            }
        } else {
            Document::default()
        };

        Self {
            mode: Mode::Normal,
            quit: false,
            terminal: Terminal::default().expect("Failed to initialize terminal"),
            cursor_pos: Pos::default(),
            document,
            offset: Pos::default(),
            status_message: StatusMessage::from(init_status),
            quit_times: QUIT_TIMES_WITHOUT_SAVING,
            highlighted_word: None,
        }
    }

    pub fn run(&mut self) {
        loop {
            if let Err(error) = self.refresh_screen() {
                die(&error);
            }

            if self.quit {
                break;
            }

            if let Err(error) = self.process_keypress() {
                die(&error);
            }
        }
    }

    fn process_keypress(&mut self) -> Result<(), std::io::Error> {
        let pressed_key = Terminal::read_key()?;
        match pressed_key {
            Key::Ctrl('q') => {
                if self.quit_times > 0 && self.document.is_dirty() {
                    self.status_message = StatusMessage::from(format!(
                        "WARN: Quitting unsaved, {} more times for quitting",
                        self.quit_times
                    ));
                    self.quit_times -= 1;
                    return Ok(());
                }
                self.quit = true;
            }
            Key::Ctrl('s') => self.save(),
            Key::Ctrl('f') => self.search(),
            Key::Char(c) => {
                self.document.insert(&self.cursor_pos, c);
                self.move_cursor(Key::Right);
            }
            Key::Delete => self.document.delete(&self.cursor_pos),
            Key::Backspace => {
                if self.cursor_pos.x > 0 || self.cursor_pos.y > 0 {
                    self.move_cursor(Key::Left);
                    self.document.delete(&self.cursor_pos);
                }
            }
            Key::Up | Key::Down | Key::Left | Key::Right => self.move_cursor(pressed_key),
            _ => (),
        }
        self.scroll();
        if self.quit_times < QUIT_TIMES_WITHOUT_SAVING {
            self.quit_times = QUIT_TIMES_WITHOUT_SAVING;
            self.status_message = StatusMessage::from(String::new());
        }
        Ok(())
    }

    fn prompt<C>(&mut self, prompt: &str, mut callback: C) -> Result<Option<String>, std::io::Error>
    where
        C: FnMut(&mut Self, Key, &String)
    {
        let mut result = String::new();
        loop {
            self.status_message = StatusMessage::from(format!("{}{}", prompt, result));
            self.refresh_screen()?;
            let key = Terminal::read_key()?;
            match key {
                Key::Backspace => result.truncate(result.len().saturating_sub(1)),
                Key::Char('\n') => break,
                Key::Char(c) => {
                    if !c.is_control() {
                        result.push(c);
                    }
                }
                Key::Esc => {
                    result.truncate(0);
                    break;
                }
                _ => (),
            }
            callback(self, key, &result);
        }
        self.status_message = StatusMessage::from(String::new());
        if result.is_empty() {
            return Ok(None);
        }
        Ok(Some(result))
    }

    fn refresh_screen(&mut self) -> Result<(), std::io::Error> {
        Terminal::cursor_hide();
        Terminal::cursor_pos(&Pos::default());
        if self.quit {
            Terminal::clear_screen();
            println!("See ya");
        } else {
            self.document.highlight(
                &self.highlighted_word,
                Some(
                    self.offset
                        .y
                        .saturating_add(self.terminal.size().height as usize),
                ),
            );
            self.draw_rows();
            self.draw_status_bar();
            self.draw_message_bar();
            Terminal::cursor_pos(&Pos {
                x: self.cursor_pos.x.saturating_sub(self.offset.x),
                y: self.cursor_pos.y.saturating_sub(self.offset.y),
            });
        }
        Terminal::cursor_show();
        Terminal::flush()
    }

    fn search(&mut self) {
        let old_pos = self.cursor_pos.clone();
        let mut direction = SearchDirection::Forward;
        let query = self
            .prompt(
                "Search (ESC-cancel, Arrows-navigate): ",
            |editor, key, query| {
                let mut moved: bool = false;
                match key {
                    Key::Right | Key::Down => {
                        direction = SearchDirection::Forward;
                        editor.move_cursor(Key::Right);
                        moved = true;
                    },
                    Key::Left | Key::Up => {
                        direction = SearchDirection::Backward;
                    },
                    _ => direction = SearchDirection::Forward,
                }
                if let Some(pos) =
                    editor
                        .document
                        .find(&query, &editor.cursor_pos, direction)
                        {
                            editor.cursor_pos = pos;
                            editor.scroll();
                        } else if moved {
                            editor.move_cursor(Key::Left);
                        }
                        editor.highlighted_word = Some(query.to_string());
            }).unwrap_or(None);

            if query.is_none() {
                self.cursor_pos = old_pos;
                self.scroll();
            }
            self.highlighted_word = None;
    }

    fn draw_welcome_message(&self) {
        let mut msg = format!("Jim Editor -- version {}", VERSION);
        let width = self.terminal.size().width as usize;
        let len = msg.len();
        #[allow(clippy::integer_arithmetic, clippy::integer_division)]
        let padding = width.saturating_sub(len) / 2;
        let spaces = " ".repeat(padding.saturating_sub(1));
        msg = format!("~{}{}", spaces, msg);
        msg.truncate(width);
        println!("{}\r", msg);
    }

    fn draw_row(&self, row: &Row) {
        let width = self.terminal.size().width as usize;
        let start = self.offset.x;
        let end = start.saturating_add(width);
        let row = row.render(start, end);
        println!("{}\r", row);
    }

    #[allow(clippy::integer_arithmetic, clippy::integer_division)]
    fn draw_rows(&self) {
        let height = self.terminal.size().height;
        for term_row in 0..height {
            Terminal::clear_current_line();
            if let Some(row) = self
                .document
                .row(self.offset.y.saturating_add(term_row as usize))
            {
                self.draw_row(row);
            } else if self.document.is_empty() && term_row == height / 3 {
                self.draw_welcome_message();
            } else {
                println!("~\r");
            }
        }
    }

    fn draw_status_bar(&self) {
        let mut status;
        let mut filename = "[No Name]".to_string();
        let width = self.terminal.size().width as usize;

        if let Some(name) = &self.document.filename {
            filename = name.clone();
            filename.truncate(width / 4);
        }
        let dirty_status = if self.document.is_dirty() {
            "(modified)"
        } else {
            ""
        };
        let line_status = format!(
            "{} | {}/{}",
            self.document.file_type(),
            self.cursor_pos.y.saturating_add(1),
            self.document.len()
        );
        status = format!("{} - line: {} {}", filename, line_status, dirty_status,);
        status.push_str(&" ".repeat(width.saturating_sub(status.len())));
        status.truncate(width);

        Terminal::set_fg_color(STATUS_FG_COLOR);
        Terminal::set_bg_color(STATUS_BG_COLOR);
        println!("{}\r", status);
        Terminal::reset_bg_color();
        Terminal::reset_fg_color();
    }

    fn draw_message_bar(&self) {
        Terminal::clear_current_line();
        let msg = &self.status_message;
        // only print status message within 5 sec
        if Instant::now() - msg.time < Duration::new(5, 0) {
            let mut text = msg.text.clone();
            text.truncate(self.terminal.size().width as usize);
            print!("{}", text);
        }
    }

    fn move_cursor(&mut self, key: Key) {
        let Pos { mut x, mut y } = self.cursor_pos;
        let height = self.document.len();
        let mut width = if let Some(row) = self.document.row(y) {
            row.len()
        } else {
            0
        };

        match key {
            Key::Up => y = y.saturating_sub(1),
            Key::Down => {
                if y < height {
                    y = y.saturating_add(1);
                }
            }
            Key::Left => {
                if x > 0 {
                    x -= 1;
                } else if y > 0 {
                    y -= 1;
                    if let Some(row) = self.document.row(y) {
                        x = row.len();
                    } else {
                        x = 0;
                    }
                }
            }
            Key::Right => {
                if x < width {
                    x += 1;
                } else if y < height {
                    y += 1;
                    x = 0;
                }
            }
            _ => (),
        }

        // prevent pos.x exceeds the length of row
        width = if let Some(row) = self.document.row(y) {
            row.len()
        } else {
            0
        };

        if x > width {
            x = width;
        }

        self.cursor_pos = Pos { x, y };
    }

    fn scroll(&mut self) {
        let Pos { x, y } = self.cursor_pos;
        let width = self.terminal.size().width as usize;
        let height = self.terminal.size().height as usize;
        let mut offset = &mut self.offset;

        if y < offset.y {
            offset.y = y;
        } else if y >= offset.y.saturating_add(height) {
            offset.y = y.saturating_sub(height).saturating_add(1);
        }
        if x < offset.x {
            offset.x = x;
        } else if x >= offset.x.saturating_add(width) {
            offset.x = x.saturating_sub(width).saturating_add(1);
        }
    }

    fn save(&mut self) {
        if self.document.filename.is_none() {
            let new_name = self.prompt("Save as: ", |_, _, _| {}).unwrap_or(None);
            if new_name.is_none() {
                self.status_message = StatusMessage::from("Save aborted".to_string());
                return;
            }
            self.document.filename = new_name;
        }

        if self.document.save().is_ok() {
            self.status_message = StatusMessage::from("File saved successfully".to_string());
        } else {
            self.status_message = StatusMessage::from("Failed to save file".to_string());
        }
    }
}

fn die(e: &std::io::Error) {
    Terminal::clear_screen();
    panic!("{}", e);
}
