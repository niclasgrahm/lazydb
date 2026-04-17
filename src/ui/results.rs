use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
    Frame,
};

use crate::app::{App, Focus, RESULTS_PAGE_SIZE};
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

    let duration_str = app
        .query_duration
        .map(|d| {
            if d.as_secs() >= 1 {
                format!("{:.2}s", d.as_secs_f64())
            } else {
                format!("{:.1}ms", d.as_secs_f64() * 1000.0)
            }
        })
        .unwrap_or_default();

    let title = format!(
        " Results: {} row{}{} ",
        result.rows.len(),
        if result.rows.len() == 1 { "" } else { "s" },
        if duration_str.is_empty() {
            String::new()
        } else {
            format!(" in {duration_str}")
        },
    );

    let table = ResultTable {
        result,
        title,
        border_color,
        scroll_row: app.results_scroll_row,
        scroll_col: app.results_scroll_col,
        page: app.results_page,
        has_more: app.results_has_more,
    };
    frame.render_widget(table, area);
}

struct ResultTable<'a> {
    result: &'a QueryResult,
    title: String,
    border_color: Color,
    scroll_row: usize,
    scroll_col: usize,
    page: usize,
    has_more: bool,
}

impl<'a> Widget for ResultTable<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 4 {
            return;
        }

        let border_style = Style::default().fg(self.border_color);
        let col_count = self.result.columns.len();
        if col_count == 0 {
            return;
        }

        // Compute natural column widths: max of header and data
        let widths: Vec<usize> = self
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

        let x0 = area.x;
        let y0 = area.y;
        let bottom_y = y0 + area.height - 1;

        // Row layout:
        // y0:       top border    ┌───┬───┐
        // y0+1:     header row    │ a │ b │
        // y0+2:     header sep    ├───┼───┤
        // y0+3..:   data rows     │ 1 │ 2 │
        // bottom_y-1: status line
        // bottom_y: bot border    └───┴───┘

        let max_data_rows = if area.height > 5 {
            (area.height - 5) as usize
        } else {
            0
        };

        // Determine visible columns based on scroll_col and available width
        let inner_width = (area.width - 2) as usize; // minus left+right border
        let visible_cols = self.visible_columns(&widths, inner_width);

        let vis_widths: Vec<usize> = visible_cols.iter().map(|&i| widths[i]).collect();

        // Top border
        self.draw_horizontal(buf, x0, y0, area.width, &vis_widths, '┌', '┬', '┐', border_style);

        // Overlay title on top border
        if !self.title.is_empty() {
            let title_x = x0 + 2;
            let max_title = (area.width as usize).saturating_sub(4);
            let title: String = self.title.chars().take(max_title).collect();
            buf.set_string(title_x, y0, &title, border_style);
        }

        // Header row
        let header_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let header_cells: Vec<String> = visible_cols
            .iter()
            .map(|&i| self.result.columns[i].clone())
            .collect();
        self.draw_row(
            buf,
            x0,
            y0 + 1,
            area.width,
            &vis_widths,
            &header_cells,
            |_| header_style,
            border_style,
        );

        // Header separator
        self.draw_horizontal(buf, x0, y0 + 2, area.width, &vis_widths, '├', '┼', '┤', border_style);

        // Data rows with vertical scrolling
        let total_rows = self.result.rows.len();
        let visible_rows = max_data_rows.min(total_rows.saturating_sub(self.scroll_row));
        for vi in 0..visible_rows {
            let ri = self.scroll_row + vi;
            let row = &self.result.rows[ri];
            let strs: Vec<String> = visible_cols.iter().map(|&i| {
                row.get(i).map(|v| v.to_string()).unwrap_or_default()
            }).collect();
            let values: Vec<Option<&Value>> = visible_cols.iter().map(|&i| row.get(i)).collect();
            self.draw_row(
                buf,
                x0,
                y0 + 3 + vi as u16,
                area.width,
                &vis_widths,
                &strs,
                |col_idx| value_style(values.get(col_idx).copied().flatten()),
                border_style,
            );
        }

        // Empty rows to fill remaining space
        let empty: Vec<String> = vec![String::new(); vis_widths.len()];
        for vi in visible_rows..max_data_rows {
            self.draw_row(
                buf,
                x0,
                y0 + 3 + vi as u16,
                area.width,
                &vis_widths,
                &empty,
                |_| Style::default(),
                border_style,
            );
        }

        // Status line
        let status_y = bottom_y - 1;
        let status = self.build_status(total_rows, col_count, max_data_rows);
        // Fill status line background
        let bg_style = Style::default().fg(Color::DarkGray);
        buf.set_string(x0, status_y, "│", border_style);
        let fill = " ".repeat((area.width - 2) as usize);
        buf.set_string(x0 + 1, status_y, &fill, bg_style);
        buf.set_string(x0 + area.width - 1, status_y, "│", border_style);
        // Write status text
        let max_status = (area.width - 4) as usize;
        let status_text: String = status.chars().take(max_status).collect();
        buf.set_string(x0 + 2, status_y, &status_text, bg_style);

        // Bottom border
        self.draw_horizontal(buf, x0, bottom_y, area.width, &vis_widths, '└', '┴', '┘', border_style);
    }
}

impl<'a> ResultTable<'a> {
    fn visible_columns(&self, widths: &[usize], available: usize) -> Vec<usize> {
        let mut cols = Vec::new();
        let mut used = 0;
        for i in self.scroll_col..widths.len() {
            // Each column needs: │ pad content pad = 1 + 1 + width + 1 = width + 3
            // Plus the final │ = 1
            let needed = widths[i] + 3;
            if cols.is_empty() {
                // Always show at least one column
                cols.push(i);
                used += needed + 1; // +1 for closing border
            } else if used + needed <= available {
                cols.push(i);
                used += needed;
            } else {
                break;
            }
        }
        cols
    }

    fn build_status(&self, total_rows: usize, total_cols: usize, visible_rows: usize) -> String {
        let row_start = self.scroll_row + 1;
        let row_end = (self.scroll_row + visible_rows).min(total_rows);
        let page_offset = self.page * RESULTS_PAGE_SIZE;

        let row_info = if total_rows == 0 {
            "no rows".to_string()
        } else {
            format!(
                "rows {}-{} of {}",
                page_offset + row_start,
                page_offset + row_end,
                if self.has_more {
                    format!("{}+", page_offset + total_rows)
                } else {
                    (page_offset + total_rows).to_string()
                }
            )
        };

        let col_info = format!(
            "cols {}-{} of {}",
            self.scroll_col + 1,
            (self.scroll_col + 1).min(total_cols),
            total_cols,
        );

        let page_info = if self.page > 0 || self.has_more {
            let more = if self.has_more { " →" } else { "" };
            let prev = if self.page > 0 { "← " } else { "" };
            format!("  page {}{prev}{more}", self.page + 1)
        } else {
            String::new()
        };

        format!("{row_info}  {col_info}{page_info}")
    }

    fn draw_horizontal(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        total_width: u16,
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
        // Fill any remaining space with the horizontal line
        let end_x = x + total_width;
        if cx < end_x {
            buf.set_string(cx - 1, y, "─".repeat((end_x - cx) as usize + 1), style);
            buf.set_string(end_x - 1, y, right.to_string(), style);
        }
    }

    fn draw_row(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        total_width: u16,
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
            buf.set_string(cx, y, "│", border_style);
            cx += 1;
        }
        // Fill remaining space and close with border
        let end_x = x + total_width;
        if cx < end_x {
            let fill = " ".repeat((end_x - cx - 1) as usize);
            buf.set_string(cx, y, &fill, border_style);
            buf.set_string(end_x - 1, y, "│", border_style);
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
