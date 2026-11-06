use crate::app::{App, Sort};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};
use ratatui::Frame;
use reaper_core::{model::human_size, DepKind};

const ACCENT: Color = Color::Red;
const DIM: Style = Style::new().fg(Color::DarkGray);

pub fn draw(f: &mut Frame, app: &mut App) {
    let [header, main, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(f.area());
    let [list, detail] =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(main);

    draw_header(f, app, header);
    draw_table(f, app, list);
    draw_detail(f, app, detail);
    draw_footer(f, app, footer);
    if app.show_help {
        draw_help(f);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            " REAPER ",
            Style::new()
                .bg(ACCENT)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "  {} packages · {} installed · {} in unneeded dependencies",
            app.graph.len(),
            human_size(app.total_size),
            human_size(app.orphan_size)
        )),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_table(f: &mut Frame, app: &mut App, area: Rect) {
    let rows: Vec<Row> = app
        .rows
        .iter()
        .filter_map(|id| app.graph.get(id))
        .map(|p| {
            Row::new(vec![
                Cell::from(p.id.name.clone()),
                Cell::from(p.id.source.to_string()).style(DIM),
                Cell::from(human_size(p.size)),
                Cell::from(if p.explicit { "explicit" } else { "dep" }).style(DIM),
            ])
        })
        .collect();
    let title = format!(
        " Packages ({}){}{} ",
        app.rows.len(),
        if app.orphans_only {
            " · orphans only"
        } else {
            ""
        },
        if app.sort == Sort::Name {
            " · by name"
        } else {
            " · by size"
        }
    );
    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(9),
        ],
    )
    .header(
        Row::new(["Name", "Source", "Size", "Reason"])
            .style(Style::new().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(Style::new().bg(ACCENT).fg(Color::Black))
    .highlight_symbol("▶ ");
    f.render_stateful_widget(table, area, &mut app.table);
}

fn names(list: &[(String, DepKind)], max: usize) -> String {
    if list.is_empty() {
        return "none".into();
    }
    let mut s: Vec<String> = list
        .iter()
        .take(max)
        .map(|(n, k)| {
            if *k == DepKind::Optional {
                format!("{n}?")
            } else {
                n.clone()
            }
        })
        .collect();
    if list.len() > max {
        s.push(format!("… +{}", list.len() - max));
    }
    s.join(", ")
}

fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Details ");
    let (Some(p), Some(d)) = (app.selected(), app.detail.as_ref()) else {
        f.render_widget(
            Paragraph::new("Nothing selected.").style(DIM).block(block),
            area,
        );
        return;
    };
    let label = |s: &str| Span::styled(format!("{s}: "), Style::new().add_modifier(Modifier::BOLD));
    let lines = vec![
        Line::from(Span::styled(
            p.id.name.clone(),
            Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "{} · {} · {}",
            p.version,
            p.id.source,
            human_size(p.size)
        )),
        Line::from(p.description.clone()),
        Line::raw(""),
        Line::from(vec![label("Depends on"), Span::raw(names(&d.deps, 12))]),
        Line::raw(""),
        Line::from(vec![label("Required by"), Span::raw(names(&d.rdeps, 12))]),
        Line::raw(""),
        Line::from(vec![
            label("If removed"),
            Span::raw(format!(
                "breaks {} package(s), frees {} orphaned dep(s)",
                d.broken, d.freed
            )),
        ]),
        Line::from(vec![
            label("You would reclaim"),
            Span::raw(human_size(d.reclaim)),
        ]),
        Line::raw(""),
        Line::styled("read-only: Reaper never removes anything yet", DIM),
    ];
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(block),
        area,
    );
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let line = if app.searching || !app.filter.is_empty() {
        Line::from(vec![
            Span::styled("/", Style::new().fg(ACCENT)),
            Span::raw(app.filter.clone()),
            Span::styled(
                if app.searching { "█" } else { "" },
                Style::new().fg(ACCENT),
            ),
        ])
    } else {
        Line::styled(
            " j/k move · / search · o orphans · s sort · ? help · q quit",
            DIM,
        )
    };
    f.render_widget(Paragraph::new(line), area);
}

fn draw_help(f: &mut Frame) {
    let area = f.area();
    let w = 46.min(area.width);
    let h = 14.min(area.height);
    let r = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    let text = "\
 j / ↓ , k / ↑   move
 PgUp / PgDn     move by 10
 g / G           first / last
 /               search by name
 Enter / Esc     accept / clear search
 o               unneeded dependencies only
 s               toggle sort (size / name)
 q , Ctrl-C      quit

 press any key to close";
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Help ")),
        r,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use reaper_core::{Graph, Package, PkgId, Source};

    #[test]
    fn renders_without_panicking() {
        let g = Graph::new(
            vec![Package {
                id: PkgId::new(Source::Pacman, "firefox"),
                version: "1.0".into(),
                description: "browser".into(),
                size: 5 << 20,
                explicit: true,
            }],
            vec![],
        );
        let mut app = App::new(g);
        for (w, h) in [(120, 30), (40, 10), (10, 3)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            t.draw(|f| draw(f, &mut app)).unwrap();
        }
        app.show_help = true;
        let mut t = Terminal::new(TestBackend::new(120, 30)).unwrap();
        t.draw(|f| draw(f, &mut app)).unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Help"));
    }
}
