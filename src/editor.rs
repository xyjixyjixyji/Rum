use crate::{document::Document, Row, Terminal};
use std::env;
use std::time::Duration;
use std::time::Instant;
use termion::color;
use termion::event::Key;
use termion::cursor;

const STATUS_FG_COLOR: color::Rgb = color::Rgb(63, 63, 63);
const STATUS_BG_COLOR: color::Rgb = color::Rgb(239, 239, 239);
const TABSIZE: usize = 4;
const VERSION: &str = env!["CARGO_PKG_VERSION"];

#[derive(PartialEq)]
pub enum Mode {
    Normal,
    Visual,
    Insert,
}

#[derive(PartialEq, Clone, Copy)]
pub enum SearchDirection {
    Forward,
    Backward,
}

#[derive(Default, Copy, Clone)]
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
    highlighted_word: Option<String>, // used for searching
}

impl Editor {
    pub fn default() -> Self {
        let mut init_status = String::from("");
        let args: Vec<String> = env::args().collect();
        let document = if let Some(filename) = args.get(1) {
            let doc = Document::open(filename);
            if let Ok(doc) = doc {
                doc
            } else {
                init_status = format!("ERR: Failed to open file: {}", filename);
                Document::default()
            }
        } else {
            Document::default()
        };

        Self {
            mode: Mode::Normal,
            quit: false,
            #[allow(clippy::expect_used)]
            terminal: Terminal::default().expect("Failed to initialize terminal"),
            cursor_pos: Pos::default(),
            document,
            offset: Pos::default(),
            status_message: StatusMessage::from(init_status),
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
        match self.mode {
            Mode::Normal => self.normal_process_keypress()?,
            Mode::Insert => self.insert_process_keypress()?,
            Mode::Visual => (),
        };
        self.scroll();
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
        let old_pos = self.cursor_pos;
        let mut direction = SearchDirection::Forward;
        let query = self
            .prompt(
                "/",
            |editor, key, query| {
                let mut moved: bool = false;
                match key {
                    Key::Char('n') => {
                        direction = SearchDirection::Forward;
                        editor.move_cursor(Key::Right);
                        moved = true;
                    },
                    Key::Char('N') => {
                        direction = SearchDirection::Backward;
                    },
                    _ => direction = SearchDirection::Forward,
                }
                if let Some(pos) =
                    editor
                        .document
                        .find(query, &editor.cursor_pos, direction)
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

    fn draw_welcome_messages(&self) {
        self.process_and_print_welcome_msg(format!("Rum Editor -- version {}", VERSION));
        println!("~\r");
        self.process_and_print_welcome_msg(format!("A Vim-like Editor completely in Rust"));
        self.process_and_print_welcome_msg(format!("Rum is permanently free and open-source"));
        println!("~\r");
        self.process_and_print_welcome_msg(format!("Use: <:q> \t to <Exit>"));
        self.process_and_print_welcome_msg(format!("Use: <:wq> \t to <Save>"));
    }

    fn process_and_print_welcome_msg(&self, msg: String) {
        let width = self.terminal.size().width as usize;

        let len = msg.len();

        #[allow(clippy::integer_arithmetic, clippy::integer_division)]
        let padding = width.saturating_sub(len) / 2;
        let spaces = " ".repeat(padding.saturating_sub(1));

        let mut processed_msg = format!("~{}{}", spaces, msg);
        processed_msg.truncate(width);

        println!("{}\r", processed_msg);
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
            } else if (term_row == height / 3) && self.document.is_empty() {
                self.draw_welcome_messages();
            } else {
                println!("~\r");
            }
        }
    }

    fn draw_status_bar(&self) {
        let mut filename = "[No Name]".to_owned();
        let width = self.terminal.size().width as usize;

        if let Some(name) = &self.document.filename {
            filename = name.clone();
            #[allow(clippy::integer_arithmetic)]
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
        let mut status = format!("{} - line: {} {}", filename, line_status, dirty_status,);
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

    fn quit(&mut self, force: bool) {
        if self.document.is_dirty() && !force {
            self.set_status_message("File unsaved, use (:q! to force quit)");
            return;
        }
        self.quit = true;
    }

    fn save(&mut self) {
        if self.document.filename.is_none() {
            let new_name = self.prompt("Save as: ", |_, _, _| {}).unwrap_or(None);
            if new_name.is_none() {
                self.set_status_message("Save aborted");
                return;
            }
            self.document.filename = new_name;
        }

        if self.document.save().is_ok() {
            self.set_status_message("File saved successfully");
        } else {
            self.set_status_message("Failed to save file");
        }
    }

    fn change_mode(&mut self, mode: Mode) {
        self.mode = mode;
        match self.mode {
            Mode::Insert => {
                print!("{}", cursor::BlinkingBar);
            },
            Mode::Normal => {
                print!("{}", cursor::BlinkingBlock);
                self.normal_move_cursor('h');
            },
            Mode::Visual => {
                print!("{}", cursor::SteadyBlock);
            },
        }
    }

    // ========================================================
    // |                                                      |
    // |                     INSERT MODE                      |
    // |                                                      |
    // ========================================================
    fn insert_process_keypress(&mut self) -> Result<(), std::io::Error> {
        let pressed_key = Terminal::read_key()?;
        match pressed_key {
            Key::Char(c) => {
                match c {
                    '\t' => {
                        for _ in 0..TABSIZE {
                            self.document.insert(&self.cursor_pos, ' ');
                            self.move_cursor(Key::Right);
                        }
                    }
                    _ => {
                        self.document.insert(&self.cursor_pos, c);
                        self.move_cursor(Key::Right);
                    }
                }
            },
            Key::Delete => self.document.delete(&self.cursor_pos),
            Key::Backspace => {
                if self.cursor_pos.x > 0 || self.cursor_pos.y > 0 {
                    self.move_cursor(Key::Left);
                    self.document.delete(&self.cursor_pos);
                }
            },
            Key::Up | Key::Down | Key::Left | Key::Right => self.move_cursor(pressed_key),
            Key::Esc => self.change_mode(Mode::Normal),
            _ => ()
        }
        Ok(())
    }

    // ========================================================
    // |                                                      |
    // |                     NORMAL MODE                      |
    // |                                                      |
    // ========================================================
    fn normal_process_keypress(&mut self) -> Result<(), std::io::Error> {
        let pressed_key = Terminal::read_key()?;
        self._normal_process_keypress(pressed_key);
        Ok(())
    }

    // wrapped function, for recursive use
    fn _normal_process_keypress(&mut self, pressed_key: Key) {
        match pressed_key {
            Key::Char(c) => match c {
                'i' => self.change_mode(Mode::Insert),
                'a' => {
                    self.move_cursor(Key::Right);
                    self.change_mode(Mode::Insert);
                }
                'A' => {
                    self.move_cursor_thisline_end();
                    self._normal_process_keypress(Key::Char('i'));
                }
                'I' => {
                    self.move_cursor_thisline_front();
                    self._normal_process_keypress(Key::Char('i'));
                }
                'v' => self.change_mode(Mode::Visual),
                'h' | 'j' | 'k' | 'l' => self.normal_move_cursor(c),
                'x' => self.document.delete(&self.cursor_pos),
                's' => {
                    self._normal_process_keypress(Key::Char('x'));
                    self._normal_process_keypress(Key::Char('i'));
                }
                ':' => self.parse_command(),
                '/' => self.search(),
                'o' => {
                    if self.normal_insert_newline() {
                        self.move_cursor_nextline_front();
                        self.change_mode(Mode::Insert);
                    }
                }
                'O' => {
                    self.move_cursor(Key::Up);
                    self._normal_process_keypress(Key::Char('o'));
                }
                '^' => self.move_cursor_thisline_first_char(),
                '\n' => {
                }
                _ => (),
            }
            Key::Backspace => {
                if self.cursor_pos.x == 0 {  // skip the newline
                    self.move_cursor(Key::Left);
                    self.move_cursor(Key::Left);
                } else {
                    self.move_cursor(Key::Left);
                }
           }
            _ => (),
        }
    }

    fn normal_insert_newline(&mut self) -> bool {
        let mut cur_pos = self.cursor_pos;
        cur_pos.x = if let Some(row) = self.document.row(cur_pos.y) {
            row.len()
        } else {
            return false;
        };
        self.document.insert(&cur_pos, '\n');
        true
    }

    // wrapper for move_cursor<char>, and contrain the navigation
    // e.g. we do not allow navigate to \n (end of line)
    fn normal_move_cursor(&mut self, c: char) {
        match c {
            'h' => {
                // not allowing use h to move up a line
                if self.cursor_pos.x != 0 {
                    self.move_cursor(Key::Left);
                }
            }
            'j' => {
                self.move_cursor(Key::Down);
                self.fix_if_cursor_at_newline();  // not allowing to navigate to \n
            }
            'k' => {
                self.move_cursor(Key::Up);
                self.fix_if_cursor_at_newline();  // not allowing to navigate to \n
            }
            'l' => {
                let Pos {x, y} = self.cursor_pos;
                // we do not allow to navigate to \n
                if let Some(row) = self.document.row(y) {
                    if x < row.len().saturating_sub(1) {
                        self.move_cursor(Key::Right)
                    }
                }
            }
            _ => (),
        }
    }

    // if we are at the last char(newline) of a line, we move back
    fn fix_if_cursor_at_newline(&mut self) {
        let Pos {x, y} = self.cursor_pos;
        if let Some(row) = self.document.row(y) {
            if x == row.len() {
                self.normal_move_cursor('h');
            }
        }
    }

    fn move_cursor_thisline_end(&mut self) {
        let mut pos = &mut self.cursor_pos;
        if let Some(row) = self.document.row(pos.y) {
            pos.x = row.len(); // we are at \n
        }
    }

    fn move_cursor_thisline_front(&mut self) {
        self.move_cursor_nextline_front();
        self.move_cursor(Key::Up);
    }

    fn move_cursor_nextline_front(&mut self) {
        self.move_cursor_thisline_end();
        self.move_cursor(Key::Right); // to next line
    }
    
    fn move_cursor_thisline_first_char(&mut self) {
        let Pos {x, y} = self.cursor_pos;
        if let Some(row) = self.document.row(y) {
            let mut pos = &mut self.cursor_pos;
            pos.x = row.index_first_char();
        }
    }


    fn parse_command(&mut self) {
        let cmd = self
            .prompt(":", |_, _, _|{})
            .unwrap_or(None);
        if let Some(cmd) = cmd {
            match &cmd as &str {
                "w" => self.save(),
                "q" => self.quit(false),
                "q!" => self.quit(true),
                "wq" => {
                    self.save();
                    self.quit(false);
                }
                _ => self.set_status_message("Unknown command!")
            }
        }
    }

    fn set_status_message(&mut self, msg: &str) {
        self.status_message = StatusMessage::from(msg.to_string());
    }
}

fn die(e: &std::io::Error) {
    Terminal::clear_screen();
    panic!("{}", e);
}
