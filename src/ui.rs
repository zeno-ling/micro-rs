use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use crate::editor::Editor;

pub fn draw(frame: &mut Frame, ed: &Editor) {
    let area = frame.area();
    let rows = Layout::vertical([
        Constraint::Length(1), // tab bar
        Constraint::Min(1),    // editor
        Constraint::Length(1), // status
        Constraint::Length(1), // menu
    ]).split(area);

    draw_tabs(frame, rows[0], ed);
    draw_panes(frame, rows[1], ed);
    draw_status(frame, rows[2], ed);
    draw_menu(frame, rows[3], ed);
}

fn draw_tabs(frame: &mut Frame, area: Rect, ed: &Editor) {
    let titles: Vec<Line> = ed.tabs.iter().enumerate().map(|(i, t)| {
        let name = t.path.as_ref().map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned())
            .unwrap_or_else(|| "[No Name]".into());
        let mark = if t.panes[t.active_pane].buf.dirty { "*" } else { "" };
        let style = if i == ed.active_tab { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default() };
        Line::from(Span::styled(format!(" {name}{mark} "), style))
    }).collect();
    let tabs = Tabs::new(titles).select(ed.active_tab).highlight_style(Style::default().fg(Color::Cyan));
    frame.render_widget(tabs, area);
}

fn draw_panes(frame: &mut Frame, area: Rect, ed: &Editor) {
    let panes = &ed.tabs[ed.active_tab].panes;
    let constraints: Vec<Constraint> = panes.iter().map(|_| Constraint::Ratio(1, panes.len() as u32)).collect();
    let chunks = Layout::default()
        .direction(if ed.tabs[ed.active_tab].vertical { Direction::Vertical } else { Direction::Horizontal })
        .constraints(constraints)
        .split(area);
    for (i, pane) in panes.iter().enumerate() {
        draw_pane(frame, chunks[i], ed, pane, i == ed.tabs[ed.active_tab].active_pane);
    }
    if let Some(prompt) = &ed.prompt {
        let parea = Rect { y: area.y + area.height.saturating_sub(1), height: 1, ..area };
        let p = Paragraph::new(format!("{}{}", prompt.label, prompt.input)).style(Style::default().bg(Color::DarkGray));
        frame.render_widget(p, parea);
    }
}

fn draw_pane(frame: &mut Frame, area: Rect, ed: &Editor, pane: &crate::editor::Pane, focused: bool) {
    let buf = &pane.buf;
    let top = pane.top;
    let height = area.height as usize;
    let gutter_w = if ed.config.line_numbers { 5 } else { 0 };
    let mut lines: Vec<Line> = Vec::new();
    for i in 0..height {
        let idx = top + i;
        if idx >= buf.line_count() { break; }
        let content = buf.line(idx);
        let mut spans: Vec<Span> = Vec::new();
        if ed.config.line_numbers {
            spans.push(Span::styled(format!("{:>4} ", idx + 1), Style::default().fg(Color::DarkGray)));
        }
        let sel = buf.sel_anchor;
        let in_sel = if let Some(anchor) = sel {
            let (a, b) = (anchor.line.min(buf.cursor.line), anchor.line.max(buf.cursor.line));
            idx >= a && idx <= b
        } else { false };
        for sp in syntax_spans(ed, &content) {
            if in_sel { spans.push(sp.patch_style(Style::default().bg(Color::Indexed(236)))); }
            else { spans.push(sp); }
        }
        lines.push(Line::from(spans));
    }
    let block = Block::default().borders(Borders::NONE).border_style(
        if focused { Style::default().fg(Color::Cyan) } else { Style::default() }
    );
    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(p, area);
    if focused && ed.config.line_numbers && gutter_w > 0 {
        let cy = area.y + buf.cursor.line.saturating_sub(top) as u16;
        if cy < area.y + area.height {
            frame.set_cursor_position((area.x + gutter_w as u16 + buf.cursor.col as u16, cy));
        }
    }
}

fn syntax_spans(ed: &Editor, content: &str) -> Vec<Span<'static>> {
    if let Some(syn) = &ed.highlighter {
        let regions = syn.highlight(content, &ed.config.theme);
        if !regions.is_empty() {
            return regions.into_iter().map(|(rgb, t)| {
                Span::styled(t, Style::default().fg(Color::Rgb(rgb[0], rgb[1], rgb[2])))
            }).collect();
        }
    }
    vec![Span::raw(content.to_string())]
}

fn draw_status(frame: &mut Frame, area: Rect, ed: &Editor) {
    let pane = &ed.tabs[ed.active_tab].panes[ed.tabs[ed.active_tab].active_pane];
    let buf = &pane.buf;
    let text = format!(
        " {} | Ln {}, Col {} | {} lines | {} | Ctrl-Q quit  Ctrl-S save  Ctrl-F find",
        ed.status_message,
        buf.cursor.line + 1,
        buf.cursor.col + 1,
        buf.line_count(),
        if buf.dirty { "modified" } else { "saved" },
    );
    let p = Paragraph::new(Line::from(Span::styled(text, Style::default().bg(Color::Blue).fg(Color::White))));
    frame.render_widget(p, area);
}

fn draw_menu(frame: &mut Frame, area: Rect, ed: &Editor) {
    let menu = if let Some(idx) = ed.menu_open {
        ed.menus[idx].1.iter().map(|(k, c)| format!(" {k}:{c} ")).collect::<Vec<_>>().join(" | ")
    } else {
        ed.menus.iter().map(|(k, _)| format!(" {k} ")).collect::<Vec<_>>().join(" | ")
    };
    let style = if ed.menu_open.is_some() {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let p = Paragraph::new(Line::from(Span::styled(menu, style)));
    frame.render_widget(p, area);
}
