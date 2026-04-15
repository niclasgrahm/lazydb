use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
    Frame,
};

use crate::app::{App, Focus};
use crate::db::{QueryResult, Value};

pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Focus::Results;
    let border_color = if focused { Color::Cyan } else { Color::DarkGray };

    let Some(result) = &app.query_result else {
        let block = Block::default()
            .title(" Results ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        frame.render_widget(block, area);
        return;
    };

    let title = format!(
        " Results ({} row{}) ",
        result.rows.len(),
        if result.rows.len() == 1 { "" } else { "s" }
    );

    let table = ResultTable {
        result,
        title,
        border_color,
    };
    frame.render_widget(table, area);
}

struct ResultTable<'a> {
    result: &'a QueryResult,
    title: String,
    border_color: Color,
}

impl<'a> Widget for ResultTable<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 3 {
            return;
        }

        let border_style = Style::default().fg(self.border_color);
        let col_count = self.result.columns.len();
        if col_count == 0 {
            return;
        }

        // Compute column widths: max of header and data
        let mut widths: Vec<usize> = self
            .result
            .columns
            .iter()
            .enumerate()
            .map(|(i, col)| {
                let max_data = self
                    .result
                    .rows
                    .iter()
                    .map(|r| r.get(i).map(|v| v.to_string().len()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                col.len().max(max_data).max(1)
            })
            .collect();

        // Available width for content: total area minus borders and separators
        // Layout: │ pad col pad │ pad col pad │ ... │
        // = (col_count + 1) border chars + col_count * 2 padding chars + sum(widths)
        let overhead = (col_count + 1 + col_count * 2) as u16;
        if area.width < overhead {
            return;
        }
        let available = (area.width - overhead) as usize;
        let total_width: usize = widths.iter().sum();
        if total_width > available {
            // Shrink columns proportionally
            for w in &mut widths {
                *w = (*w * available / total_width).max(1);
            }
        }

        let x0 = area.x;
        let y0 = area.y;

        // Row layout:
        // y0:     top border    ┌───┬───┐
        // y0+1:   header row    │ a │ b │
        // y0+2:   header sep    ├───┼───┤
        // y0+3..: data rows     │ 1 │ 2 │
        // last:   bottom border └───┴───┘

        let max_data_rows = if area.height > 4 {
            (area.height - 4) as usize // top + header + sep + bottom
        } else {
            0
        };

        // Top border
        self.draw_horizontal(buf, x0, y0, &widths, '┌', '┬', '┐', border_style);

        // Overlay title on top border
        if self.title.len() > 0 {
            let title_x = x0 + 2;
            let max_title = (area.width as usize).saturating_sub(4);
            let title: String = self.title.chars().take(max_title).collect();
            buf.set_string(title_x, y0, &title, border_style);
        }

        // Header row
        let header_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        self.draw_row(
            buf,
            x0,
            y0 + 1,
            &widths,
            &self.result.columns,
            |_| header_style,
            border_style,
        );

        // Header separator
        self.draw_horizontal(buf, x0, y0 + 2, &widths, '├', '┼', '┤', border_style);

        // Data rows
        let visible_rows = self.result.rows.len().min(max_data_rows);
        for (i, row) in self.result.rows.iter().take(visible_rows).enumerate() {
            let strs: Vec<String> = row.iter().map(|v| v.to_string()).collect();
            let values = &self.result.rows[i];
            self.draw_row(
                buf,
                x0,
                y0 + 3 + i as u16,
                &widths,
                &strs,
                |col_idx| value_style(values.get(col_idx)),
                border_style,
            );
        }

        // Bottom border
        let bottom_y = y0 + 3 + visible_rows as u16;
        if bottom_y < y0 + area.height {
            self.draw_horizontal(buf, x0, bottom_y, &widths, '└', '┴', '┘', border_style);
        }
    }
}

impl<'a> ResultTable<'a> {
    fn draw_horizontal(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        widths: &[usize],
        left: char,
        mid: char,
        right: char,
        style: Style,
    ) {
        let mut cx = x;
        buf.set_string(cx, y, left.to_string(), style);
        cx += 1;
        for (i, &w) in widths.iter().enumerate() {
            // pad + content + pad
            let seg: String = "─".repeat(w + 2);
            buf.set_string(cx, y, &seg, style);
            cx += (w + 2) as u16;
            if i + 1 < widths.len() {
                buf.set_string(cx, y, mid.to_string(), style);
            } else {
                buf.set_string(cx, y, right.to_string(), style);
            }
            cx += 1;
        }
    }

    fn draw_row(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        widths: &[usize],
        cells: &[String],
        cell_style: impl Fn(usize) -> Style,
        border_style: Style,
    ) {
        let mut cx = x;
        buf.set_string(cx, y, "│", border_style);
        cx += 1;
        for (i, &w) in widths.iter().enumerate() {
            let text = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            // Truncate if needed
            let display: String = if text.len() > w {
                text.chars().take(w.saturating_sub(1)).collect::<String>() + "…"
            } else {
                format!("{:width$}", text, width = w)
            };
            buf.set_string(cx, y, " ", border_style);
            cx += 1;
            buf.set_string(cx, y, &display, cell_style(i));
            cx += w as u16;
            buf.set_string(cx, y, " ", border_style);
            cx += 1;
            if i + 1 < widths.len() {
                buf.set_string(cx, y, "│", border_style);
            } else {
                buf.set_string(cx, y, "│", border_style);
            }
            cx += 1;
        }
    }
}

fn value_style(val: Option<&Value>) -> Style {
    match val {
        Some(Value::Null) => Style::default().fg(Color::DarkGray),
        Some(Value::Int(_) | Value::Float(_)) => Style::default().fg(Color::Cyan),
        Some(Value::Bool(_)) => Style::default().fg(Color::Magenta),
        Some(Value::Text(_)) => Style::default().fg(Color::White),
        None => Style::default(),
    }
}
