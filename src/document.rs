use crate::FileType;
use crate::Pos;
use crate::Row;
use crate::SearchDirection;
use std::fs;
use std::io::{Error, Write};

#[derive(Default)]
pub struct Document {
    rows: Vec<Row>,
    dirty: bool,
    pub filename: Option<String>,
    filetype: FileType,
}

impl Document {
    pub fn open(filename: &str) -> Result<Self, std::io::Error> {
        let contents = fs::read_to_string(filename)?;
        let mut rows = Vec::new();
        let filetype = FileType::from(filename);

        for value in contents.lines() {
            rows.push(Row::from(value));
        }
        Ok(Self {
            rows,
            dirty: false,
            filename: Some(filename.to_string()),
            filetype: filetype,
        })
    }

    pub fn insert(&mut self, at: &Pos, c: char) {
        if at.y > self.rows.len() {
            return;
        }
        self.dirty = true;
        if c == '\n' {
            if at.x == self.rows[at.y].len() {
                self.insert_newline_at_end(at.y);
            } else {
                self.insert_newline(at);
            }
        } else if at.y == self.rows.len() {
            let mut row = Row::default();
            row.insert(0, c);
            self.rows.push(row);
        } else {
            #[allow(clippy::indexing_slicing)]
            let row = &mut self.rows[at.y];
            row.insert(at.x, c);
        }
        self.unhighlight_rows(at.y);
    }

    // more efficient (w/o split)
    pub fn insert_newline_at_end(&mut self, y_at: usize) {
        if y_at > self.rows.len() {
            return;
        }
        self.rows.insert(y_at.saturating_add(1), Row::default());
    }

    pub fn insert_newline(&mut self, at: &Pos) {
        if at.y > self.rows.len() {
            return;
        }
        if at.y == self.rows.len() {
            self.rows.push(Row::default());
            return;
        }
        #[allow(clippy::indexing_slicing)]
        let current_row = &mut self.rows[at.y];
        let new_row = current_row.split(at.x);
        #[allow(clippy::integer_arithmetic)]
        self.rows.insert(at.y + 1, new_row);
    }

    #[allow(clippy::integer_arithmetic, clippy::indexing_slicing)]
    pub fn delete(&mut self, at: &Pos) {
        let len = self.rows.len();
        if at.y >= len {
            return;
        }
        self.dirty = true;
        if at.x == self.rows[at.y].len() && at.y + 1 < len {
            let next_row = self.rows.remove(at.y + 1);
            let row = &mut self.rows[at.y];
            row.append(&next_row);
        } else {
            let row = &mut self.rows[at.y];
            row.delete(at.x);
        }
        self.unhighlight_rows(at.y);
    }

    pub fn save(&mut self) -> Result<(), Error> {
        if let Some(filename) = &self.filename {
            let mut file = fs::File::create(filename)?;
            self.filetype = FileType::from(&filename[..]);
            for row in &mut self.rows {
                file.write_all(row.as_bytes())?;
                file.write_all(b"\n")?;
            }
            self.dirty = false;
        }
        Ok(())
    }

    pub fn find(&self, query: &str, at: &Pos, direction: SearchDirection) -> Option<Pos> {
        if at.y > self.rows.len() {
            return None;
        }
        let mut pos = Pos { x: at.x, y: at.y };
        let start = if direction == SearchDirection::Forward {
            at.y
        } else {
            0
        };
        let end = if direction == SearchDirection::Forward {
            self.rows.len()
        } else {
            at.y.saturating_add(1) // exclusive
        };

        for _ in start..end {
            if let Some(row) = self.rows.get(pos.y) {
                if let Some(x) = row.find(&query, pos.x, direction) {
                    pos.x = x;
                    return Some(pos);
                }
                if direction == SearchDirection::Forward {
                    pos.x = 0;
                    pos.y = pos.y.saturating_add(1);
                } else {
                    pos.y = pos.y.saturating_sub(1);
                    pos.x = self.rows[pos.y].len();
                }
            } else {
                return None;
            }
        }
        None
    }

    pub fn highlight(&mut self, word: &Option<String>, until: Option<usize>) {
        let mut start_with_comment = false;
        let until = if let Some(until) = until {
            if until.saturating_add(1) < self.rows.len() {
                until.saturating_add(1)
            } else {
                self.rows.len()
            }
        } else {
            self.rows.len()
        };
        for row in &mut self.rows[..until] {
            start_with_comment = row.highlight(
                &self.filetype.options(),
                word,
                start_with_comment);
        }
    }

    fn unhighlight_rows(&mut self, start: usize) {
        for row in self.rows.iter_mut().skip(start.saturating_sub(1)) {
            row.is_highlighted = false;
        }
    }

    pub fn row(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn file_type(&self) -> String {
        self.filetype.name()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
