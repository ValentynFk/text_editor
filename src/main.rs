extern crate console;
extern crate terminal_size;
use console::{Key, Term};
use std::array;
use std::io::{Read, SeekFrom, Write};
use std::thread::{current, sleep};
use std::time::Duration;
use terminal_size::terminal_size;

/// Site is a main platform where all rendering happens,
/// don't judge me about my names
trait Screen {
    /// Basically, template method that renders text
    fn render(&mut self) {
        // This actually does all rendering
        if let Err(e) = self.render_impl() {
            panic!("{}", e);
        }
    }
    #[inline(always)]
    fn get_term(&mut self) -> &mut Term;
    #[inline(always)]
    fn render_impl(&mut self) -> std::io::Result<()>;
}
struct TextWidget {
    size: (u16, u16),
    offset: u64,
    line: u64,
    cursor: (u16, u16),
    is_update: bool,
    file: std::fs::File,
    filename: String,
    term: Term,
    buff: Vec<String>,
    lines_len: Vec<usize>,
}
impl Screen for TextWidget {
    fn get_term(&mut self) -> &mut Term {
        &mut self.term
    }
    fn render_impl(&mut self) -> std::io::Result<()> {
        if self.is_update {
            // We are rendering a whole terminal window
            self.term.move_cursor_to(0, 0);

            let mut height = self.size.0 as usize;
            let mut position = self.offset;
            let width = self.size.1 as usize;
            self.buff.clear();

            // Try to fill whole screen
            let mut chunk = vec![0u8; width];
            use std::io::Seek;
            // Current position is where rendering begins - first character of top line on screen
            self.file.seek(SeekFrom::Start(position)).unwrap();
            // Chunk typically is equal to the width of terminal window - number of available columns
            let size = self.file.read(&mut chunk).unwrap();
            if size != 0 {
                let mut lines: Vec<&[u8]> = chunk[0..size].split(|c| *c == '\n' as u8).collect();
                let last_line_size = lines.pop().unwrap().len();
                for line in lines {
                    self.term.clear_line();
                    self.term.write(&line);
                    self.buff.push(line.iter().map(|c| *c as char).collect());
                    self.term.move_cursor_down(1);
                    height -= 1;
                    if height == 1 {
                        self.term.flush();
                        self.is_update = false;
                        let mut pos = self.buff[self.cursor.1 as usize].len();
                        if pos != 0 {
                            pos -= 1;
                        }
                        self.term.move_cursor_to(
                            std::cmp::min(self.cursor.0 as usize, pos),
                            self.cursor.1 as usize,
                        );
                        return Ok(());
                    }
                }
                position += (size - last_line_size) as u64;
            }
            loop {
                self.term.clear_line();
                self.term.write("~".as_bytes());
                self.term.move_cursor_down(1);
                height -= 1;
                if height == 1 {
                    self.term.flush();
                    self.is_update = false;
                    let mut pos = self.buff[self.cursor.1 as usize].len();
                    if pos != 0 {
                        pos -= 1;
                    }
                    self.term.move_cursor_to(
                        std::cmp::min(self.cursor.0 as usize, pos),
                        self.cursor.1 as usize,
                    );
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}
impl TextWidget {
    /// Creates new Site, tho nothing special here
    pub fn new(name: &str) -> Self {
        // Create a path to the desired file
        let path = std::path::Path::new(name);
        // Open the path in read-only mode, returns `io::Result<File>`
        let file = std::fs::File::open(&path).unwrap();
        let term = Term::stdout();
        let mut filename = String::new();
        filename.push_str(name);
        Self {
            size: term.size(),
            cursor: (0, 0),
            line: 1,
            offset: 0,
            is_update: true,
            file,
            filename,
            term,
            buff: vec![String::new(); 1],
            lines_len: vec![],
        }
    }

    pub fn key_cb(&mut self) -> bool {
        match self.term.read_key().unwrap() {
            Key::Char(c) => match c {
                'k' => {
                    if self.cursor.1 != 0 {
                        self.cursor.1 -= 1;
                        self.is_update = true;
                    } else {
                        if let Some(prev_line_length) = self.lines_len.pop() {
                            self.offset -= prev_line_length as u64;
                            self.is_update = true;
                        }
                    }
                }
                'h' => {
                    let mut pos = self.buff[self.cursor.1 as usize].len() as u16;
                    if pos != 0 {
                        pos -= 1;
                    }
                    if self.cursor.0 >= pos + 1 {
                        self.cursor.0 = pos;
                    }
                    if self.cursor.0 != 0 {
                        self.cursor.0 -= 1;
                        self.is_update = true;
                    }
                }
                'j' => {
                    if self.cursor.1 < (self.buff.len() - 1) as u16 {
                        self.cursor.1 += 1;
                        self.is_update = true;
                    } else {
                        self.lines_len.push(self.buff[0].len() + 1);
                        self.offset += (self.buff[0].len() + 1) as u64;
                        if self.cursor.1 != 0 {
                            self.cursor.1 -= 1;
                        }
                        self.is_update = true;
                    }
                }
                'l' | ' ' => {
                    let mut pos = self.buff[self.cursor.1 as usize].len() as u16;
                    if pos != 0 {
                        pos -= 1;
                    }
                    if self.cursor.0 < pos {
                        self.cursor.0 += 1;
                        self.is_update = true;
                    }
                }
                'i' => {
                    self.insert_mode();
                }
                'A' => {
                    self.lines_len.push(self.buff[0].len() + 1);
                    self.offset += (self.buff[0].len() + 1) as u64;
                    if self.cursor.1 != 0 {
                        self.cursor.1 -= 1;
                    }
                    self.is_update = true;
                }
                'B' => {
                    if let Some(prev_line_length) = self.lines_len.pop() {
                        self.offset -= prev_line_length as u64;
                        if self.cursor.1 < (self.buff.len() - 1) as u16 {
                            self.cursor.1 += 1;
                        }
                        self.is_update = true;
                    }
                }
                ':' => {
                    self.is_update = true;
                    return self.process_cmd();
                }
                _ => (),
            },
            Key::Enter => {
                if self.cursor.1 < (self.buff.len() - 1) as u16 {
                    self.cursor.1 += 1;
                    self.is_update = true;
                }
            }
            _ => (),
        }
        false
    }

    pub fn resize_cb(&mut self) {
        let size = self.term.size();
        if size != self.size {
            self.size = size;
            self.is_update = true;
        }
    }

    pub fn process_cmd(&mut self) -> bool {
        self.term.move_cursor_to(self.size.0 as usize, self.size.1 as usize);
        self.term.clear_line();
        self.term.write(":".as_bytes());
        let mut string = String::with_capacity(20);
        loop {
            match self.term.read_key().unwrap() {
                Key::Enter => {
                    // q is quit, w is write
                    if string.contains('w') {
                        self.write();
                        self.term.clear_line();
                        self.term.write(format!("Written {}", self.filename).as_bytes());
                    }
                    return if string.contains('q') {
                        self.term.move_cursor_to(0, 0);
                        self.term.clear_to_end_of_screen();
                        true
                    } else {
                        false
                    };
                }
                Key::Backspace => {
                    if !string.is_empty() {
                        string.pop();
                        self.term.move_cursor_left(1);
                        self.term.write(" ".as_bytes());
                        self.term.move_cursor_left(1);
                    }
                }
                Key::Char(char) => {
                    self.term.write(&vec![char as u8; 1]);
                    string.push(char);
                }
                _ => (),
            }
        }
    }

    pub fn write(&mut self) {
        use std::io::Seek;
        self.file.seek(SeekFrom::Start(self.line)).unwrap();
        for line in &self.buff {
            self.file.write(line.as_bytes());
        }
    }

    pub fn cursor_offset(&self) -> i64 {
        0
    }

    pub fn insert_mode(&mut self) {
        use std::io::Seek;
        // Offset to the beginning of rendering area + offset within rendering area
        self.file.seek(SeekFrom::Start(self.line)).unwrap();
        self.file.seek(SeekFrom::Current(self.cursor_offset())).unwrap();
    }
}

fn main() {
    let mut text_w = TextWidget::new("/Users/vfaychuk/petprojects/ngide/sample.txt");
    loop {
        text_w.render();
        if text_w.key_cb() {
            break;
        }
        text_w.resize_cb();
        sleep(Duration::new(0, 10000));
    }
}
