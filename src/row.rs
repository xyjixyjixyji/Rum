use crate::highlighting;
use crate::HighlightingOptions;
use crate::SearchDirection;
use std::cmp;
use termion::color;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct Row {
    string: String,
    highlighting: Vec<highlighting::Type>,
    pub is_highlighted: bool,
    len: usize,
}

impl From<&str> for Row {
    fn from(slice: &str) -> Self {
        Self {
            string: String::from(slice),
            highlighting: Vec::new(),
            is_highlighted: false,
            len: slice.graphemes(true).count(),
        }
    }
}

impl Row {
    #[must_use]
    pub fn render(&self, start: usize, end: usize) -> String {
        let end = cmp::min(end, self.string.len());
        let start = cmp::min(start, end);
        let mut parsed = String::new();
        let mut cur_highlighting = &highlighting::Type::None;
        #[allow(clippy::integer_arithmetic)]
        for (index, grapheme) in self.string[..]
            .graphemes(true)
            .enumerate()
            .skip(start)
            .take(end - start)
        {
            if let Some(c) = grapheme.chars().next() {
                let highlighting_type = self
                    .highlighting
                    .get(index)
                    .unwrap_or(&highlighting::Type::None);

                if highlighting_type != cur_highlighting {
                    cur_highlighting = highlighting_type;
                    let start_highlighting =
                        format!("{}", termion::color::Fg(highlighting_type.to_color()));
                    parsed.push_str(&start_highlighting[..]);
                }

                parsed.push(c);
            }
        }
        let end_highlight = format!("{}", termion::color::Fg(color::Reset));
        parsed.push_str(&end_highlight[..]);
        parsed
    }

    pub fn insert(&mut self, at: usize, c: char) {
        if at >= self.len() {
            self.string.push(c);
            self.len += 1;
            return;
        } else {
            let mut result: String = String::new();
            let mut length = 0;
            for (index, grapheme) in self.string[..].graphemes(true).enumerate() {
                length += 1;
                if index == at {
                    length += 1;
                    result.push(c);
                }
                result.push_str(grapheme);
            }
            self.len = length;
            self.string = result;
        }
    }

    pub fn delete(&mut self, at: usize) {
        if at >= self.len() {
            return;
        } else {
            let mut result: String = String::new();
            let mut length = 0;
            for (index, grapheme) in self.string[..].graphemes(true).enumerate() {
                if index != at {
                    length += 1;
                    result.push_str(grapheme);
                }
            }
            self.len = length;
            self.string = result;
        }
    }

    pub fn append(&mut self, new: &Self) {
        self.string = format!("{}{}", self.string, new.string);
        self.len += new.len;
    }

    pub fn split(&mut self, at: usize) -> Self {
        let mut row = String::new();
        let mut length = 0;
        let mut splitted_row = String::new();
        let mut splitted_length = 0;
        for (index, grapheme) in self.string[..].graphemes(true).enumerate() {
            if index < at {
                length += 1;
                row.push_str(grapheme);
            } else {
                splitted_length += 1;
                splitted_row.push_str(grapheme);
            }
        }
        self.string = row;
        self.len = length;
        self.is_highlighted = false;
        Self {
            string: splitted_row,
            highlighting: Vec::new(),
            is_highlighted: false,
            len: splitted_length,
        }
    }

    pub fn find(&self, query: &str, at: usize, direction: SearchDirection) -> Option<usize> {
        if query.is_empty() || at > self.len() {
            return None;
        }
        // [start, end)
        let start = if direction == SearchDirection::Forward {
            at
        } else {
            0
        };
        let end = if direction == SearchDirection::Forward {
            self.len
        } else {
            at
        };

        #[allow(clippy::integer_arithmetic)]
        let substring: String = self.string[..]
            .graphemes(true)
            .skip(start)
            .take(end - start)
            .collect();
        let matching_byte_index = if direction == SearchDirection::Forward {
            substring.find(query)
        } else {
            substring.rfind(query)
        };

        if let Some(matching_byte_index) = matching_byte_index {
            for (grapheme_index, (byte_index, _)) in
                substring[..].grapheme_indices(true).enumerate()
            {
                if byte_index == matching_byte_index {
                    return Some(start + grapheme_index);
                }
            }
        }
        None
    }

    fn highlight_match(&mut self, word: &Option<String>) {
        if let Some(word) = word {
            if word.is_empty() {
                return;
            }
            let mut index = 0;
            while let Some(search_match) = self.find(word, index, SearchDirection::Forward) {
                if let Some(next_index) = search_match.checked_add(word[..].graphemes(true).count())
                {
                    for i in index.saturating_add(search_match)..next_index {
                        self.highlighting[i] = highlighting::Type::Match;
                    }
                    index = next_index;
                } else {
                    break;
                }
            }
        }
    }

    fn highlight_char(
        &mut self,
        index: &mut usize,
        opts: &HighlightingOptions,
        c: char,
        chars: &[char],
    ) -> bool {
        if opts.characters() && c == '\'' {
            if let Some(next_char) = chars.get(index.saturating_add(1)) {
                let closing_index = if *next_char == '\\' {
                    index.saturating_add(3)
                } else {
                    index.saturating_add(2)
                };
                if let Some(closing_char) = chars.get(closing_index) {
                    if *closing_char == '\'' {
                        for _ in 0..=closing_index.saturating_sub(*index) {
                            self.highlighting.push(highlighting::Type::Character);
                            *index += 1;
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    fn highlight_comment(
        &mut self,
        index: &mut usize,
        opts: &HighlightingOptions,
        c: char,
        chars: &[char],
    ) -> bool {
        if opts.comments() && c == '/' && *index < chars.len() {
            if let Some(next_char) = chars.get(index.saturating_add(1)) {
                if *next_char == '/' {
                    for _ in *index..chars.len() {
                        self.highlighting.push(highlighting::Type::Comment);
                        *index += 1;
                    }
                    return true;
                }
            }
        }
        false
    }

    fn highlight_multiline_comment(
        &mut self,
        index: &mut usize,
        opts: &HighlightingOptions,
        c: char,
        chars: &[char],
    ) -> bool {
        if opts.multiline_comments() && c == '/' && *index < chars.len() {
            if let Some(next_char) = chars.get(index.saturating_add(1)) {
                if *next_char == '*' {
                    let closing_index =
                        if let Some(closing_index) = self.string[*index + 2..].find("*/") {
                            *index + closing_index + 4 // 4 = len(/**/)
                        } else {
                            chars.len()
                        };
                    for _ in *index..closing_index {
                        self.highlighting.push(highlighting::Type::MultilineComment);
                        *index += 1;
                    }
                    return true;
                }
            };
        }
        false
    }

    fn highlight_string(
        &mut self,
        index: &mut usize,
        opts: &HighlightingOptions,
        c: char,
        chars: &[char],
    ) -> bool {
        if opts.strings() && c == '"' {
            loop {
                self.highlighting.push(highlighting::Type::String);
                *index += 1;
                if let Some(next_char) = chars.get(*index) {
                    if *next_char == '"' {
                        break;
                    }
                } else {
                    break;
                }
            }
            self.highlighting.push(highlighting::Type::String);
            *index += 1;
            return true;
        }
        false
    }

    fn highlight_numbers(
        &mut self,
        index: &mut usize,
        opts: &HighlightingOptions,
        c: char,
        chars: &[char],
    ) -> bool {
        if opts.numbers() && c.is_ascii_digit() {
            if *index > 0 {
                let prev_char = chars[*index - 1];
                if !(is_sep(prev_char)) {
                    return false;
                }
            }
            loop {
                self.highlighting.push(highlighting::Type::Number);
                *index += 1;
                if let Some(next_char) = chars.get(*index) {
                    if *next_char != '.' && !next_char.is_ascii_digit() {
                        break;
                    }
                }
            }
            return true;
        }
        false
    }

    fn highlight_str(
        &mut self,
        index: &mut usize,
        substring: &str,
        chars: &[char],
        hl_type: highlighting::Type,
    ) -> bool {
        if substring.is_empty() {
            return false;
        }
        for (substring_index, c) in substring.chars().enumerate() {
            if let Some(next_char) = chars.get(index.saturating_add(substring_index)) {
                if *next_char != c {
                    return false;
                }
            } else {
                return false;
            }
        }
        for _ in 0..substring.len() {
            self.highlighting.push(hl_type);
            *index += 1;
        }
        true
    }

    fn highlight_keywords(
        &mut self,
        index: &mut usize,
        chars: &[char],
        keywords: &[String],
        hl_type: highlighting::Type,
    ) -> bool {
        if *index > 0 {
            #[allow(clippy::indexing_slicing, clippy::integer_arithmetic)]
            let prev_char = chars[*index - 1];
            if !is_sep(prev_char) {
                return false;
            }
        }
        for word in keywords {
            if *index < chars.len().saturating_sub(word.len()) {
                #[allow(clippy::indexing_slicing, clippy::integer_arithmetic)]
                let next_char = chars[*index + word.len()];
                if !is_sep(next_char) {
                    continue;
                }
            }

            if self.highlight_str(index, &word, chars, hl_type) {
                return true;
            }
        }
        false
    }

    fn highlight_primary_keywords(
        &mut self,
        index: &mut usize,
        opts: &HighlightingOptions,
        chars: &[char],
    ) -> bool {
        self.highlight_keywords(
            index,
            chars,
            opts.primary_keywords(),
            highlighting::Type::PrimaryKeywords,
        )
    }

    fn highlight_secondary_keywords(
        &mut self,
        index: &mut usize,
        opts: &HighlightingOptions,
        chars: &[char],
    ) -> bool {
        self.highlight_keywords(
            index,
            chars,
            opts.secondary_keywords(),
            highlighting::Type::SecondaryKeywords,
        )
    }

    // word: for searching highlight
    #[allow(clippy::indexing_slicing, clippy::integer_arithmetic)]
    pub fn highlight(
        &mut self,
        opts: &HighlightingOptions,
        word: &Option<String>,
        start_with_comment: bool,
    ) -> bool {
        let chars: Vec<char> = self.string.chars().collect();
        let mut index = 0; // byte index in chars

        // If the row is already highlighted(indicated by self.is_highlighted),
        // and we are not searching, we simply just stop highlighting it
        // but if the line is in a multiline comment, we still need to give
        // the correct return value
        if self.is_highlighted && word.is_none() {
            if let Some(hl_type) = self.highlighting.last() {
                if *hl_type == highlighting::Type::MultilineComment
                    && self.string.len() > 1
                    && self.string[self.string.len() - 2..] == *"*/"
                {
                    return true;
                }
            }
            return false;
        }

        self.highlighting = Vec::new();
        let mut in_multi_comment = start_with_comment;
        if in_multi_comment {
            let closing_index = if let Some(closing_index) = self.string.find("*/") {
                closing_index + 2
            } else {
                chars.len()
            };
            for _ in 0..closing_index {
                self.highlighting.push(highlighting::Type::MultilineComment);
            }
            index = closing_index; // if the whole line is commentted, will not enter the while loop
        }

        while let Some(c) = chars.get(index) {
            if self.highlight_multiline_comment(&mut index, opts, *c, &chars) {
                in_multi_comment = true;
                continue;
            }
            in_multi_comment = false;
            if self.highlight_char(&mut index, opts, *c, &chars)
                || self.highlight_comment(&mut index, opts, *c, &chars)
                || self.highlight_primary_keywords(&mut index, opts, &chars)
                || self.highlight_secondary_keywords(&mut index, opts, &chars)
                || self.highlight_string(&mut index, opts, *c, &chars)
                || self.highlight_numbers(&mut index, opts, *c, &chars)
            {
                continue;
            }
            self.highlighting.push(highlighting::Type::None);
            index += 1;
        }

        self.highlight_match(word);
        if in_multi_comment && &self.string[self.string.len().saturating_sub(2)..] != "*/" {
            return true; // we are still in the multiline comment
        }
        self.is_highlighted = true;
        false // we are out of the multiline comment
    }

    pub fn index_first_char(&self) -> usize {
        let mut index = 0;
        for (i, char) in self.string.chars().enumerate() {
            if char != ' ' {
                index = i;
                break;
            }
        }
        index
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.string.as_bytes()
    }
}

fn is_sep(c: char) -> bool {
    c.is_ascii_punctuation() || c.is_ascii_whitespace()
}
