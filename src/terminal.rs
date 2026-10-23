use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use std::collections::VecDeque;
use vte::{Params, Parser, Perform};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellStyle {
    pub fg: Color32,
    pub bg: Option<Color32>,
    pub bold: bool,
    pub underline: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self {
            fg: Color32::from_rgb(220, 220, 220), // Light gray/white default
            bg: None,
            bold: false,
            underline: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalCell {
    pub c: char,
    pub style: CellStyle,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            c: ' ',
            style: CellStyle::default(),
        }
    }
}

#[allow(dead_code)]
pub struct TerminalScreen {
    pub cols: usize,
    pub rows: usize,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub cursor_visible: bool,
    pub grid: Vec<Vec<TerminalCell>>,
    pub scrollback: VecDeque<Vec<TerminalCell>>,
    pub max_scrollback: usize,
    current_style: CellStyle,
    pub title: String,
    parser: Parser,
    pub scroll_offset: usize,
}

#[allow(dead_code)]
impl TerminalScreen {
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = vec![vec![TerminalCell::default(); cols]; rows];
        Self {
            cols,
            rows,
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: true,
            grid,
            scrollback: VecDeque::new(),
            max_scrollback: 5000,
            current_style: CellStyle::default(),
            title: "Terminal".to_string(),
            parser: Parser::new(),
            scroll_offset: 0,
        }
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        if new_cols == 0 || new_rows == 0 || (new_cols == self.cols && new_rows == self.rows) {
            return;
        }

        let mut new_grid = vec![vec![TerminalCell::default(); new_cols]; new_rows];
        for r in 0..new_rows.min(self.rows) {
            for c in 0..new_cols.min(self.cols) {
                new_grid[r][c] = self.grid[r][c];
            }
        }

        self.cols = new_cols;
        self.rows = new_rows;
        self.grid = new_grid;
        self.cursor_x = self.cursor_x.min(new_cols.saturating_sub(1));
        self.cursor_y = self.cursor_y.min(new_rows.saturating_sub(1));
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            let mut parser = std::mem::replace(&mut self.parser, Parser::new());
            parser.advance(self, b);
            self.parser = parser;
        }
    }

    fn newline(&mut self) {
        if self.cursor_y + 1 >= self.rows {
            // Push top line to scrollback
            if !self.grid.is_empty() {
                let top_line = self.grid.remove(0);
                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.pop_front();
                }
                self.scrollback.push_back(top_line);
                self.grid.push(vec![TerminalCell::default(); self.cols]);
            }
        } else {
            self.cursor_y += 1;
        }
    }

    fn carriage_return(&mut self) {
        self.cursor_x = 0;
    }

    fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        }
    }

    pub fn clear(&mut self) {
        for row in &mut self.grid {
            for cell in row {
                *cell = TerminalCell::default();
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    /// Exports all screen and scrollback content as plaintext
    pub fn get_text_content(&self) -> String {
        let mut out = String::new();
        for line in &self.scrollback {
            let s: String = line.iter().map(|c| c.c).collect();
            out.push_str(s.trim_end());
            out.push('\n');
        }
        for line in &self.grid {
            let s: String = line.iter().map(|c| c.c).collect();
            out.push_str(s.trim_end());
            out.push('\n');
        }
        out
    }
}

impl Perform for TerminalScreen {
    fn print(&mut self, c: char) {
        if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.newline();
        }
        if self.cursor_y < self.rows && self.cursor_x < self.cols {
            self.grid[self.cursor_y][self.cursor_x] = TerminalCell {
                c,
                style: self.current_style,
            };
            self.cursor_x += 1;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.carriage_return(),
            b'\x08' => self.backspace(),
            b'\t' => {
                let next_tab = (self.cursor_x / 8 + 1) * 8;
                self.cursor_x = next_tab.min(self.cols.saturating_sub(1));
            }
            b'\x07' => {} // Bell
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: char) {
        let get_param = |idx: usize, def: u16| -> u16 {
            params.iter().nth(idx).and_then(|p| p.first()).copied().unwrap_or(def)
        };

        match action {
            // Cursor Up
            'A' => {
                let n = get_param(0, 1) as usize;
                self.cursor_y = self.cursor_y.saturating_sub(n);
            }
            // Cursor Down
            'B' => {
                let n = get_param(0, 1) as usize;
                self.cursor_y = (self.cursor_y + n).min(self.rows.saturating_sub(1));
            }
            // Cursor Forward
            'C' => {
                let n = get_param(0, 1) as usize;
                self.cursor_x = (self.cursor_x + n).min(self.cols.saturating_sub(1));
            }
            // Cursor Back
            'D' => {
                let n = get_param(0, 1) as usize;
                self.cursor_x = self.cursor_x.saturating_sub(n);
            }
            // Cursor Position (H or f)
            'H' | 'f' => {
                let row = get_param(0, 1).saturating_sub(1) as usize;
                let col = get_param(1, 1).saturating_sub(1) as usize;
                self.cursor_y = row.min(self.rows.saturating_sub(1));
                self.cursor_x = col.min(self.cols.saturating_sub(1));
            }
            // Erase in Display (J)
            'J' => {
                let mode = get_param(0, 0);
                match mode {
                    0 => {
                        // Clear from cursor to end
                        if self.cursor_y < self.rows {
                            for x in self.cursor_x..self.cols {
                                self.grid[self.cursor_y][x] = TerminalCell::default();
                            }
                            for y in (self.cursor_y + 1)..self.rows {
                                for x in 0..self.cols {
                                    self.grid[y][x] = TerminalCell::default();
                                }
                            }
                        }
                    }
                    1 => {
                        // Clear from start to cursor
                        for y in 0..self.cursor_y {
                            for x in 0..self.cols {
                                self.grid[y][x] = TerminalCell::default();
                            }
                        }
                        if self.cursor_y < self.rows {
                            for x in 0..=self.cursor_x.min(self.cols.saturating_sub(1)) {
                                self.grid[self.cursor_y][x] = TerminalCell::default();
                            }
                        }
                    }
                    2 | 3 => self.clear(),
                    _ => {}
                }
            }
            // Erase in Line (K)
            'K' => {
                let mode = get_param(0, 0);
                if self.cursor_y < self.rows {
                    match mode {
                        0 => {
                            for x in self.cursor_x..self.cols {
                                self.grid[self.cursor_y][x] = TerminalCell::default();
                            }
                        }
                        1 => {
                            for x in 0..=self.cursor_x.min(self.cols.saturating_sub(1)) {
                                self.grid[self.cursor_y][x] = TerminalCell::default();
                            }
                        }
                        2 => {
                            for x in 0..self.cols {
                                self.grid[self.cursor_y][x] = TerminalCell::default();
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Select Graphic Rendition (m)
            'm' => {
                if params.is_empty() {
                    self.current_style = CellStyle::default();
                    return;
                }
                let mut iter = params.iter();
                while let Some(param) = iter.next() {
                    let code = param.first().copied().unwrap_or(0);
                    match code {
                        0 => self.current_style = CellStyle::default(),
                        1 => self.current_style.bold = true,
                        4 => self.current_style.underline = true,
                        22 => self.current_style.bold = false,
                        24 => self.current_style.underline = false,
                        30..=37 => self.current_style.fg = ansi_16_color(code - 30, false),
                        39 => self.current_style.fg = CellStyle::default().fg,
                        40..=47 => self.current_style.bg = Some(ansi_16_color(code - 40, false)),
                        49 => self.current_style.bg = None,
                        90..=97 => self.current_style.fg = ansi_16_color(code - 90, true),
                        100..=107 => self.current_style.bg = Some(ansi_16_color(code - 100, true)),
                        // 256 / true color sequences
                        38 => {
                            if let Some(next) = iter.next() {
                                if next.first() == Some(&5) {
                                    if let Some(c) = iter.next().and_then(|p| p.first()) {
                                        self.current_style.fg = xterm_256_color(*c as u8);
                                    }
                                }
                            }
                        }
                        48 => {
                            if let Some(next) = iter.next() {
                                if next.first() == Some(&5) {
                                    if let Some(c) = iter.next().and_then(|p| p.first()) {
                                        self.current_style.bg = Some(xterm_256_color(*c as u8));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() >= 2 && (params[0] == b"0" || params[0] == b"2") {
            if let Ok(title) = std::str::from_utf8(params[1]) {
                self.title = title.to_string();
            }
        }
    }
}

pub fn render_terminal_ui(
    ui: &mut egui::Ui,
    screen: &mut TerminalScreen,
    font_id: FontId,
) -> egui::Response {
    let font_size = font_id.size;
    let char_width = font_size * 0.60;
    let line_height = font_size * 1.25;

    let total_width = (screen.cols as f32) * char_width;
    let total_height = (screen.rows as f32) * line_height;

    let (response, painter) =
        ui.allocate_painter(Vec2::new(total_width, total_height), egui::Sense::click_and_drag());

    let rect = response.rect;

    // Draw dark terminal background
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 24));

    // Render screen grid
    for (row_idx, row) in screen.grid.iter().enumerate() {
        let y = rect.min.y + (row_idx as f32) * line_height;

        for (col_idx, cell) in row.iter().enumerate() {
            let x = rect.min.x + (col_idx as f32) * char_width;
            let cell_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(char_width, line_height));

            // Background fill if cell has custom background
            if let Some(bg_color) = cell.style.bg {
                painter.rect_filled(cell_rect, 0.0, bg_color);
            }

            // Draw character
            if cell.c != ' ' && cell.c != '\0' {
                painter.text(
                    Pos2::new(x, y),
                    egui::Align2::LEFT_TOP,
                    cell.c.to_string(),
                    font_id.clone(),
                    cell.style.fg,
                );
            }

            // Underline
            if cell.style.underline {
                painter.line_segment(
                    [Pos2::new(x, y + line_height - 1.0), Pos2::new(x + char_width, y + line_height - 1.0)],
                    Stroke::new(1.0_f32, cell.style.fg),
                );
            }
        }
    }

    // Render blinking/solid cursor
    if screen.cursor_visible && screen.cursor_y < screen.rows && screen.cursor_x < screen.cols {
        let cx = rect.min.x + (screen.cursor_x as f32) * char_width;
        let cy = rect.min.y + (screen.cursor_y as f32) * line_height;
        let cursor_rect = Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(char_width, line_height));
        painter.rect_filled(cursor_rect, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 160));
    }

    response
}

fn ansi_16_color(code: u16, bright: bool) -> Color32 {
    match (code, bright) {
        (0, false) => Color32::from_rgb(0, 0, 0),         // Black
        (1, false) => Color32::from_rgb(205, 49, 49),     // Red
        (2, false) => Color32::from_rgb(13, 188, 121),    // Green
        (3, false) => Color32::from_rgb(229, 229, 16),    // Yellow
        (4, false) => Color32::from_rgb(36, 114, 200),    // Blue
        (5, false) => Color32::from_rgb(188, 63, 188),    // Magenta
        (6, false) => Color32::from_rgb(17, 168, 205),    // Cyan
        (7, false) => Color32::from_rgb(229, 229, 229),   // White
        (0, true) => Color32::from_rgb(102, 102, 102),    // Bright Black
        (1, true) => Color32::from_rgb(241, 76, 76),      // Bright Red
        (2, true) => Color32::from_rgb(35, 209, 139),     // Bright Green
        (3, true) => Color32::from_rgb(245, 245, 67),     // Bright Yellow
        (4, true) => Color32::from_rgb(59, 142, 234),     // Bright Blue
        (5, true) => Color32::from_rgb(214, 112, 214),    // Bright Magenta
        (6, true) => Color32::from_rgb(41, 184, 219),     // Bright Cyan
        (7, true) => Color32::from_rgb(255, 255, 255),    // Bright White
        _ => Color32::WHITE,
    }
}

fn xterm_256_color(code: u8) -> Color32 {
    if code < 16 {
        ansi_16_color((code % 8) as u16, code >= 8)
    } else if code < 232 {
        let c = code - 16;
        let b = (c % 6) * 51;
        let g = ((c / 6) % 6) * 51;
        let r = (c / 36) * 51;
        Color32::from_rgb(r, g, b)
    } else {
        let gray = 8 + (code - 232) * 10;
        Color32::from_rgb(gray, gray, gray)
    }
}
