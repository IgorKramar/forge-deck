use crate::model::{Focus, Popup, SETTINGS, SettingKind, State, View, number, string, value_label};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Padding, Paragraph, Row, Table, TableState, Wrap,
    },
};
use serde_json::Value;

const BG: Color = Color::Rgb(26, 27, 38);
const SURFACE: Color = Color::Rgb(36, 40, 59);
const BORDER: Color = Color::Rgb(65, 72, 104);
const TEXT: Color = Color::Rgb(192, 202, 245);
const SOFT: Color = Color::Rgb(169, 177, 214);
const MUTED: Color = Color::Rgb(115, 124, 164);
const BLUE: Color = Color::Rgb(122, 162, 247);
const GREEN: Color = Color::Rgb(158, 206, 106);
const CYAN: Color = Color::Rgb(125, 207, 255);
const PURPLE: Color = Color::Rgb(187, 154, 247);
const YELLOW: Color = Color::Rgb(224, 175, 104);
const RED: Color = Color::Rgb(247, 118, 142);

fn fg(color: Color) -> Style {
    Style::default().fg(color)
}
fn bold(color: Color) -> Style {
    fg(color).add_modifier(Modifier::BOLD)
}
fn panel(title: impl Into<Line<'static>>, active: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(fg(if active { BLUE } else { BORDER }))
        .title(title.into().style(fg(if active { BLUE } else { MUTED })))
        .padding(Padding::new(2, 2, 1, 1))
        .style(fg(SOFT).bg(BG))
}

pub fn draw(frame: &mut Frame, state: &State) {
    let area = frame.area();
    frame.render_widget(Block::default().style(fg(TEXT).bg(BG)), area);
    if area.width < 80 || area.height < 24 {
        let text = Text::from(vec![
            Line::styled("GitLab Deck", bold(BLUE)),
            Line::from(""),
            Line::from("Увеличьте терминал минимум до 80 × 24."),
            Line::styled(
                format!("Сейчас: {} × {}", area.width, area.height),
                fg(MUTED),
            ),
            Line::from(""),
            Line::styled("q — выход", fg(MUTED)),
        ]);
        frame.render_widget(
            Paragraph::new(text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false }),
            centered(area, area.width.saturating_sub(4), 8),
        );
        return;
    }
    let inner = area.inner(ratatui::layout::Margin::new(2, 1));
    let project_tabs = u16::from(state.context.project.is_some()) * 2;
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(project_tabs),
        Constraint::Min(8),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .split(inner);
    header(frame, rows[0], state);
    if project_tabs > 0 {
        tabs(frame, rows[1], state);
    }
    workspace(frame, rows[2], state);
    footer(frame, rows[3], state);
    status_line(frame, rows[4], state);
    if let Some(popup) = &state.popup {
        popup_draw(frame, area, popup, state.popup_scroll);
    }
}

fn header(frame: &mut Frame, area: Rect, state: &State) {
    let parts = Layout::horizontal([
        Constraint::Length(18),
        Constraint::Min(10),
        Constraint::Length(16),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(Line::styled(" GitLab Deck", bold(BLUE))),
        parts[0],
    );
    let context = state
        .context
        .project
        .as_ref()
        .map(|p| string(p, "path_with_namespace"))
        .or_else(|| state.context.group.as_ref().map(|g| string(g, "full_path")));
    let mut host = vec![Span::styled(state.host.clone(), fg(SOFT))];
    if let Some(context) = context {
        host.push(Span::styled(format!("  /  {context}"), fg(MUTED)));
    }
    frame.render_widget(Paragraph::new(Line::from(host)), parts[1]);
    let label = if state.loading {
        let spinner = ["◐", "◓", "◑", "◒"][(state.tick as usize / 2) % 4];
        Span::styled(format!("{spinner} загрузка"), fg(CYAN))
    } else if state.demo {
        Span::styled(" ДЕМО ", bold(GREEN))
    } else {
        Span::styled(" GitLab ", fg(PURPLE))
    };
    frame.render_widget(
        Paragraph::new(Line::from(label)).alignment(Alignment::Right),
        parts[2],
    );
}

fn tabs(frame: &mut Frame, area: Rect, state: &State) {
    let current = state.context.view;
    let items = [
        ("o", "Обзор", matches!(current, View::Overview)),
        (
            "f",
            "Файлы",
            matches!(current, View::Repository | View::File | View::Branches),
        ),
        (
            "m",
            "MR",
            matches!(
                current,
                View::MergeRequests | View::MrDetails | View::MrDiff | View::MrNotes
            ),
        ),
        (
            "p",
            "CI / CD",
            matches!(current, View::Pipelines | View::Jobs | View::Trace),
        ),
        ("s", "Настройки", matches!(current, View::Settings)),
    ];
    let mut spans = Vec::new();
    for (key, label, selected) in items {
        spans.push(Span::styled(
            format!(" {key} "),
            if selected { bold(BLUE) } else { fg(MUTED) },
        ));
        spans.push(Span::styled(
            format!("{label}   "),
            if selected { fg(BLUE) } else { fg(SOFT) },
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn workspace(frame: &mut Frame, area: Rect, state: &State) {
    let show_sidebar = area.width >= 136 || state.focus == Focus::Sidebar;
    let show_details = state.focus == Focus::Details
        || (area.width >= 136 && !matches!(state.context.view, View::Overview));
    let sizes = [
        Constraint::Length(if show_sidebar { 20 } else { 0 }),
        Constraint::Length(u16::from(show_sidebar)),
        Constraint::Min(30),
        Constraint::Length(u16::from(show_details)),
        Constraint::Length(if show_details { 29 } else { 0 }),
    ];
    let columns = Layout::horizontal(sizes).split(area);
    if show_sidebar {
        sidebar(frame, columns[0], state);
    }
    match state.context.view {
        View::Settings => settings(frame, columns[2], state),
        View::Overview => overview(frame, columns[2], state),
        view if view.is_text() => text_view(frame, columns[2], state),
        _ => list_view(frame, columns[2], state),
    }
    if show_details {
        details(frame, columns[4], state);
    }
}

fn sidebar(frame: &mut Frame, area: Rect, state: &State) {
    let block = panel(" Обзор ", state.focus == Focus::Sidebar);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let labels = [
        "  Проекты",
        "  Группы",
        "  Все MR",
        "  Избранное",
        "󰒓  Сервер",
    ];
    let mut lines = vec![Line::styled("ПРОСТРАНСТВО", fg(MUTED)), Line::from("")];
    for (index, label) in labels.iter().enumerate() {
        let active = index == state.sidebar_selected;
        lines.push(Line::styled(
            format!(" {}{label}", if active { "▏" } else { " " }),
            if active {
                bold(BLUE).bg(SURFACE)
            } else {
                fg(SOFT)
            },
        ));
        lines.push(Line::from(""));
    }
    if let Some(group) = &state.context.group {
        lines.extend([
            Line::styled("ТЕКУЩАЯ ГРУППА", fg(MUTED)),
            Line::from(""),
            Line::styled(string(group, "full_path").to_owned(), fg(PURPLE)),
        ]);
    }
    if let Some(project) = &state.context.project {
        lines.extend([
            Line::from(""),
            Line::styled("ПРОЕКТ", fg(MUTED)),
            Line::from(""),
            Line::styled(string(project, "name").to_owned(), fg(BLUE)),
        ]);
    }
    if inner.height >= 25 {
        lines.extend([
            Line::from(""),
            Line::styled("Tab — сменить фокус", fg(MUTED)),
            Line::styled("1 / 2 / 3 — разделы", fg(MUTED)),
        ]);
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn list_view(frame: &mut Frame, area: Rect, state: &State) {
    let title = format!(" {}  {} ", state.context.view.title(), state.rows.len());
    let block = panel(title, state.focus == Focus::Main);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }
    let sections = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);
    let query = if state.context.query.is_empty() {
        "  / — поиск".into()
    } else {
        format!("  {}", state.context.query)
    };
    frame.render_widget(Paragraph::new(query).style(fg(MUTED)), sections[0]);
    if state.rows.is_empty() {
        empty(frame, sections[1], state);
        return;
    }
    let compact = inner.width < 75;
    let (headers, widths) = table_columns(state.context.view, compact);
    let table_rows = state.rows.iter().map(|row| {
        let cells = table_cells(row, state.context.view, compact, state);
        Row::new(cells).height(2).bottom_margin(1)
    });
    let table = Table::new(table_rows, widths)
        .header(Row::new(headers).style(fg(MUTED)).height(2))
        .column_spacing(2)
        .row_highlight_style(fg(TEXT).bg(SURFACE))
        .highlight_symbol("▏ ");
    let mut table_state =
        TableState::default().with_selected(Some(state.selected.min(state.rows.len() - 1)));
    frame.render_stateful_widget(table, sections[1], &mut table_state);
    let hint = match state.context.view {
        View::Repository => format!(
            "{}  ·  страница {}",
            if state.context.path.is_empty() {
                "/"
            } else {
                &state.context.path
            },
            state.context.page
        ),
        View::Favorites => format!("{} избранных проектов", state.rows.len()),
        _ => format!(
            "Страница {}  ·  до {} записей  ·  n / N — листать",
            state.context.page,
            crate::model::PAGE_SIZE
        ),
    };
    frame.render_widget(Paragraph::new(hint).style(fg(MUTED)), sections[2]);
}

fn table_columns(view: View, compact: bool) -> (Vec<&'static str>, Vec<Constraint>) {
    let (a, b, c) = match view {
        View::Projects | View::Favorites => ("ПРОЕКТ", "ДОСТУП", "ОБНОВЛЁН"),
        View::Groups => ("ГРУППА", "ДОСТУП", "ID"),
        View::MergeRequests => ("MERGE REQUEST", "СТАТУС", "АВТОР"),
        View::Pipelines => ("ПАЙПЛАЙН / ВЕТКА", "СТАТУС", "ВРЕМЯ"),
        View::Jobs => ("ДЖОБА / ЭТАП", "СТАТУС", "ВРЕМЯ"),
        View::Repository => ("ИМЯ", "ТИП", "РЕЖИМ"),
        View::Branches => ("ВЕТКА", "ЗАЩИТА", "КОММИТ"),
        _ => ("ИМЯ", "СТАТУС", "ID"),
    };
    if compact {
        (
            vec![a, b],
            vec![Constraint::Min(20), Constraint::Length(14)],
        )
    } else {
        (
            vec![a, b, c],
            vec![
                Constraint::Min(24),
                Constraint::Length(16),
                Constraint::Length(16),
            ],
        )
    }
}

fn table_cells<'a>(row: &'a Value, view: View, compact: bool, state: &State) -> Vec<Cell<'a>> {
    let (name, sub, status, third) = match view {
        View::Projects | View::Favorites => {
            let star = if state.favorites.contains(&number(row, "id")) {
                " "
            } else {
                " "
            };
            (
                format!("{star}{}", string(row, "name")),
                string(row, "path_with_namespace").to_owned(),
                string(row, "visibility").to_owned(),
                date(string(row, "last_activity_at")),
            )
        }
        View::Groups => (
            format!(" {}", string(row, "name")),
            string(row, "full_path").to_owned(),
            string(row, "visibility").to_owned(),
            number(row, "id").to_string(),
        ),
        View::MergeRequests => (
            format!("!{}  {}", number(row, "iid"), string(row, "title")),
            format!(
                "{} → {}",
                string(row, "source_branch"),
                string(row, "target_branch")
            ),
            if row.get("draft").and_then(Value::as_bool).unwrap_or(false) {
                "draft".into()
            } else {
                string(row, "state").into()
            },
            row.get("author")
                .map(|a| string(a, "username"))
                .unwrap_or("—")
                .to_owned(),
        ),
        View::Pipelines => (
            format!("#{}  {}", number(row, "id"), string(row, "ref")),
            short(string(row, "sha"), 8),
            string(row, "status").into(),
            duration(row.get("duration")),
        ),
        View::Jobs => (
            string(row, "name").into(),
            string(row, "stage").into(),
            string(row, "status").into(),
            duration(row.get("duration")),
        ),
        View::Repository => (
            format!(
                "{} {}",
                if string(row, "type") == "tree" {
                    ""
                } else {
                    "󰈙"
                },
                string(row, "name")
            ),
            String::new(),
            if string(row, "type") == "tree" {
                "каталог".into()
            } else {
                "файл".into()
            },
            string(row, "mode").into(),
        ),
        View::Branches => (
            format!(" {}", string(row, "name")),
            if row.get("default").and_then(Value::as_bool).unwrap_or(false) {
                "основная ветка".into()
            } else {
                String::new()
            },
            if row
                .get("protected")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                " protected".into()
            } else {
                "обычная".into()
            },
            row.get("commit")
                .map(|c| short(string(c, "short_id"), 8))
                .unwrap_or_else(|| "—".into()),
        ),
        _ => (
            string(row, "name").into(),
            String::new(),
            string(row, "status").into(),
            number(row, "id").to_string(),
        ),
    };
    let label = match status.as_str() {
        "success" | "passed" => format!(" {status}"),
        "failed" => format!(" {status}"),
        "running" => format!(" {status}"),
        "manual" => format!(" {status}"),
        "private" => " private".into(),
        _ => status.clone(),
    };
    let mut cells = vec![
        Cell::from(Text::from(vec![
            Line::styled(name, fg(TEXT)),
            Line::styled(sub, fg(MUTED)),
        ])),
        Cell::from(label).style(fg(status_color(&status))),
    ];
    if !compact {
        cells.push(Cell::from(third).style(fg(MUTED)));
    }
    cells
}

fn empty(frame: &mut Frame, area: Rect, state: &State) {
    let (title, hint) = if state.loading {
        (
            "Загрузка данных…",
            "Можно продолжать навигацию или выйти по q",
        )
    } else if state.error.is_some() {
        (
            "Не удалось загрузить данные",
            "F5 — повторить чтение · H — сменить сервер",
        )
    } else if state.context.view == View::Favorites {
        (
            "Пока нет избранных проектов",
            "Откройте каталог и нажмите * на нужном проекте",
        )
    } else {
        (
            "Ничего не найдено",
            "Попробуйте другой поиск или предыдущую страницу",
        )
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled(title, fg(SOFT)),
            Line::from(""),
            Line::styled(hint, fg(MUTED)),
        ])
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn overview(frame: &mut Frame, area: Rect, state: &State) {
    let project = if state.object.is_object() {
        &state.object
    } else {
        state.context.project.as_ref().unwrap_or(&Value::Null)
    };
    let block = panel("  Обзор проекта ", state.focus == Focus::Main);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = vec![
        Line::styled(string(project, "name").to_owned(), bold(BLUE)),
        Line::styled(string(project, "path_with_namespace").to_owned(), fg(MUTED)),
        Line::from(""),
        Line::from(string(project, "description").to_owned()),
        Line::from(""),
        Line::from(""),
    ];
    lines.extend(project_facts(project));
    lines.extend([
        Line::from(""),
        Line::styled("РАЗДЕЛЫ ПРОЕКТА", fg(MUTED)),
        Line::from(""),
        Line::from("f  файлы       m  merge requests       p  пайплайны"),
        Line::from("b  ветки       s  настройки            Esc  назад"),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((state.scroll, 0)),
        inner,
    );
}

fn text_view(frame: &mut Frame, area: Rect, state: &State) {
    let title = match state.context.view {
        View::Trace => state
            .context
            .job
            .as_ref()
            .map(|v| format!("  {} · лог #{} ", string(v, "name"), number(v, "id"))),
        View::File => state
            .context
            .file
            .as_ref()
            .map(|v| format!(" 󰈙 {} ", string(v, "path"))),
        View::MrDetails | View::MrDiff | View::MrNotes => state
            .context
            .mr
            .as_ref()
            .map(|v| format!(" !{} · {} ", number(v, "iid"), state.context.view.title())),
        _ => None,
    }
    .unwrap_or_else(|| format!(" {} ", state.context.view.title()));
    let block = panel(title, state.focus == Focus::Main);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if state.text.is_empty() {
        empty(frame, inner, state);
        return;
    }
    let query = state.context.query.to_lowercase();
    let lines = state
        .text
        .lines()
        .map(|line| {
            let color = if state.context.view == View::MrDiff {
                if line.starts_with("+++") || line.starts_with("---") {
                    BLUE
                } else if line.starts_with('+') {
                    GREEN
                } else if line.starts_with('-') {
                    RED
                } else if line.starts_with("@@") {
                    PURPLE
                } else {
                    SOFT
                }
            } else if line.contains("error:") || line.contains("FAILED") {
                RED
            } else if line.contains("passed") || line.contains("Finished") {
                GREEN
            } else {
                SOFT
            };
            let matched = !query.is_empty() && line.to_lowercase().contains(&query);
            Line::styled(
                line.to_owned(),
                if matched {
                    fg(BG).bg(YELLOW)
                } else {
                    fg(color)
                },
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).scroll((state.scroll, state.scroll_x)),
        inner,
    );
}

fn settings(frame: &mut Frame, area: Rect, state: &State) {
    let block = panel(" 󰒓 Настройки проекта ", state.focus == Focus::Main);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = usize::from(inner.height / 4).max(1);
    let selected = state.selected.min(SETTINGS.len() - 1);
    let start = selected
        .saturating_sub(visible / 2)
        .min(SETTINGS.len().saturating_sub(visible));
    for (position, setting) in SETTINGS.iter().enumerate().skip(start).take(visible) {
        let y = inner.y + ((position - start) * 4) as u16;
        let field_area = Rect::new(
            inner.x,
            y,
            inner.width,
            3.min(inner.bottom().saturating_sub(y)),
        );
        let available = state.object.get(setting.key).is_some_and(|v| !v.is_null());
        let value = state
            .object
            .get(setting.key)
            .filter(|v| !v.is_null())
            .map(display_setting)
            .unwrap_or_else(|| "недоступно на этом сервере".into());
        let label = format!(" {} · {} ", setting.group, setting.label);
        let field = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(fg(if position == selected { BLUE } else { BORDER }))
            .title(Span::styled(
                label,
                fg(if position == selected { BLUE } else { MUTED }),
            ))
            .padding(Padding::horizontal(1));
        frame.render_widget(
            Paragraph::new(value)
                .style(fg(if available { TEXT } else { MUTED }))
                .block(field),
            field_area,
        );
    }
}

fn display_setting(value: &Value) -> String {
    match value {
        Value::Bool(true) => "  включено".into(),
        Value::Bool(false) => "  выключено".into(),
        _ => value_label(value),
    }
}

fn details(frame: &mut Frame, area: Rect, state: &State) {
    let title = if state.context.view == View::Settings {
        " О поле "
    } else {
        " Предпросмотр "
    };
    let block = panel(title, state.focus == Focus::Details);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = if state.context.view == View::Settings {
        let setting = &SETTINGS[state.selected.min(SETTINGS.len() - 1)];
        let mut lines = vec![
            Line::styled(setting.label, bold(BLUE)),
            Line::from(""),
            Line::from(setting.help),
            Line::from(""),
        ];
        if let SettingKind::Choice(values) = setting.kind {
            lines.extend([
                Line::styled("Допустимые значения", fg(MUTED)),
                Line::from(values.join(" / ")),
                Line::from(""),
            ]);
        }
        lines.extend([
            Line::styled(setting.key, fg(MUTED)),
            Line::from(""),
            Line::from("e / Enter — изменить"),
            Line::from(""),
            Line::styled("Сохранение потребует подтверждения.", fg(YELLOW)),
        ]);
        lines
    } else if let Some(row) = match state.context.view {
        View::Trace => state.context.job.as_ref(),
        View::File => state.context.file.as_ref(),
        View::MrDetails | View::MrDiff | View::MrNotes => state.context.mr.as_ref(),
        _ => state.selected().or(state.context.project.as_ref()),
    } {
        match state.context.view {
            View::Projects | View::Favorites => project_preview(row),
            View::Groups => vec![
                Line::styled(string(row, "name").to_owned(), bold(BLUE)),
                Line::from(""),
                Line::from(string(row, "full_path").to_owned()),
                Line::from(""),
                Line::from(string(row, "description").to_owned()),
                Line::from(""),
                Line::styled("Enter — проекты группы", fg(MUTED)),
            ],
            View::Pipelines | View::Jobs | View::Trace => ci_preview(row, state),
            View::MergeRequests | View::MrDetails | View::MrDiff | View::MrNotes => mr_preview(row),
            View::Repository | View::File => vec![
                Line::styled(string(row, "name").to_owned(), bold(BLUE)),
                Line::from(""),
                Line::from(string(row, "path").to_owned()),
                Line::from(""),
                Line::styled("Enter — открыть", fg(MUTED)),
            ],
            _ => vec![
                Line::styled(state.context.view.title(), fg(MUTED)),
                Line::from(""),
                Line::from(string(row, "name").to_owned()),
            ],
        }
    } else {
        vec![Line::styled("Выберите строку", fg(MUTED))]
    };
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((
            if state.focus == Focus::Details {
                state.scroll
            } else {
                0
            },
            0,
        )),
        inner,
    );
}

fn project_preview(project: &Value) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled(string(project, "name").to_owned(), bold(BLUE)),
        Line::styled(string(project, "path_with_namespace").to_owned(), fg(MUTED)),
        Line::from(""),
        Line::from(string(project, "description").to_owned()),
        Line::from(""),
    ];
    if let Some(topics) = project.get("topics").and_then(Value::as_array) {
        lines.extend([
            Line::styled(
                topics
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" · "),
                fg(PURPLE),
            ),
            Line::from(""),
        ]);
    }
    lines.extend(project_facts(project));
    lines.extend([
        Line::from(""),
        Line::styled("Enter — открыть проект", fg(MUTED)),
    ]);
    lines
}

fn project_facts(project: &Value) -> Vec<Line<'static>> {
    let role = ["project_access", "group_access"]
        .iter()
        .filter_map(|key| {
            project
                .get("permissions")?
                .get(key)?
                .get("access_level")?
                .as_u64()
        })
        .max();
    let role = match role {
        Some(50..) => "Owner",
        Some(40..) => "Maintainer",
        Some(30..) => "Developer",
        Some(20..) => "Reporter",
        Some(_) => "Guest",
        None => "—",
    };
    vec![
        fact("Доступ", string(project, "visibility")),
        fact("Роль", role),
        fact("Ветка", string(project, "default_branch")),
        fact("ID проекта", &number(project, "id").to_string()),
        fact("Обновлён", &date(string(project, "last_activity_at"))),
    ]
}

fn ci_preview(item: &Value, state: &State) -> Vec<Line<'static>> {
    let status = string(item, "status");
    let mut lines = vec![
        Line::styled(format!("#{}", number(item, "id")), bold(BLUE)),
        Line::from(""),
        Line::styled(status.to_owned(), fg(status_color(status))),
        Line::from(""),
        fact("Время", &duration(item.get("duration"))),
        fact("Этап", string(item, "stage")),
        fact("Ветка", string(item, "ref")),
        Line::from(""),
    ];
    if let Some(pipeline) = &state.context.pipeline {
        lines.extend([
            fact("Пайплайн", &format!("#{}", number(pipeline, "id"))),
            Line::from(""),
        ]);
    }
    lines.extend([
        Line::styled("r — повторить", fg(MUTED)),
        Line::styled("x — отменить", fg(MUTED)),
        Line::styled("S — ручной запуск", fg(MUTED)),
        Line::from(""),
        Line::styled("Действия требуют подтверждения.", fg(YELLOW)),
    ]);
    lines
}

fn mr_preview(item: &Value) -> Vec<Line<'static>> {
    vec![
        Line::styled(format!("!{}", number(item, "iid")), bold(BLUE)),
        Line::from(""),
        Line::from(string(item, "title").to_owned()),
        Line::from(""),
        Line::styled(
            format!(
                "{} → {}",
                string(item, "source_branch"),
                string(item, "target_branch")
            ),
            fg(PURPLE),
        ),
        Line::from(""),
        Line::from(string(item, "description").to_owned()),
        Line::from(""),
        Line::styled("d — дифф", fg(MUTED)),
        Line::styled("c — обсуждения", fg(MUTED)),
    ]
}

fn fact(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), fg(MUTED)),
        Span::styled(value.to_owned(), fg(SOFT)),
    ])
}

fn footer(frame: &mut Frame, area: Rect, state: &State) {
    let contextual = match state.context.view {
        View::Projects | View::Groups | View::Favorites => {
            "Enter открыть   / поиск   * избранное   n/N страницы"
        }
        View::Pipelines => "Enter джобы   r повторить   x отменить   n/N страницы",
        View::Jobs => "Enter лог   r повторить   x отменить   S запуск",
        View::Settings => "e / Enter изменить   j/k поле   Esc назад",
        View::MergeRequests | View::MrDetails | View::MrDiff | View::MrNotes => {
            "Enter подробно   d дифф   c обсуждения   / поиск"
        }
        View::Trace | View::File => {
            "j/k ↑↓ текст   ←→ по горизонтали   / поиск   F5 обновить   Esc назад"
        }
        View::Repository => "Enter открыть   j/k выбор   n/N страницы   Esc назад",
        _ => "Enter открыть   j/k выбор   / поиск   Esc назад",
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(contextual, fg(BLUE)),
            Line::styled(
                "Tab фокус    1 проекты  2 группы  3 MR    H сервер    ? помощь    q выход",
                fg(MUTED),
            ),
        ]),
        area,
    );
}

fn status_line(frame: &mut Frame, area: Rect, state: &State) {
    let line = if let Some(error) = &state.error {
        Line::styled(format!(" {}", error.replace(['\n', '\r'], " ")), fg(RED))
    } else if !state.status.is_empty() {
        Line::styled(state.status.replace(['\n', '\r'], " "), fg(YELLOW))
    } else {
        Line::styled(
            format!(
                "{}  ·  {} записей  ·  {}",
                state.context.view.title(),
                state.rows.len(),
                if state.demo {
                    "демонстрационные данные"
                } else {
                    "F5 / R — обновить"
                }
            ),
            fg(MUTED),
        )
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn popup_draw(frame: &mut Frame, area: Rect, popup: &Popup, scroll: u16) {
    let (width, height) = match popup {
        Popup::Help => (90, 23),
        Popup::Input { .. } => (85, 10),
        Popup::Confirm { .. } => (90, 18),
    };
    let popup_area = centered(
        area,
        width.min(area.width.saturating_sub(6)),
        height.min(area.height.saturating_sub(2)),
    );
    frame.render_widget(Clear, popup_area);
    let title = match popup {
        Popup::Help => " ? Клавиши ",
        Popup::Input { title, .. } | Popup::Confirm { title, .. } => title.as_str(),
    };
    let block = panel(title.to_owned(), true).style(fg(TEXT).bg(SURFACE));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    match popup {
        Popup::Help => {
            let lines = [
                "j / k, ↑ / ↓     выбор строки или прокрутка текста",
                "← / → в тексте   горизонтальная прокрутка",
                "Tab              фокус: меню → содержимое → детали",
                "1 / 2 / 3        проекты / группы / все MR",
                "o / f / m        обзор / файлы / MR текущего проекта",
                "p / b / s        CI / ветки / настройки проекта",
                "Enter            открыть выбранное; изменить поле настроек",
                "Esc / Backspace  вернуться назад",
                "/                поиск; Enter применить, Esc отменить",
                "n / N, [ / ]     страницы; в тексте n/N — поиск далее/назад",
                "d / c            дифф / обсуждения merge request",
                "r / x / S        повтор / отмена CI / запуск ручной джобы",
                "F5 / R           перечитать текущий экран",
                "*                добавить / убрать избранный проект",
                "e                редактировать выбранную настройку",
                "H                подключиться к другому серверу",
                "Enter / y        подтвердить действие; Esc / n — отменить",
                "q / Ctrl+C       выход; ? / Esc — закрыть эту помощь",
            ];
            frame.render_widget(
                Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );
        }
        Popup::Input { value, purpose, .. } => {
            let sections = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(inner);
            let input = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(fg(BLUE))
                .padding(Padding::horizontal(1));
            let input_inner = input.inner(sections[1]);
            let display = value.replace('\n', " ↵ ").replace('\t', " ⇥ ");
            let text_width = Line::from(display.as_str()).width();
            let offset = text_width
                .saturating_sub(usize::from(input_inner.width.saturating_sub(1)))
                .min(u16::MAX as usize) as u16;
            frame.render_widget(
                Paragraph::new(display).scroll((0, offset)).block(input),
                sections[1],
            );
            let hint = match purpose {
                crate::model::InputPurpose::Setting(index) => match SETTINGS[*index].kind {
                    SettingKind::Choice(_) | SettingKind::Bool => {
                        "← / → выбрать   Enter продолжить   Esc отменить"
                    }
                    _ => "Ctrl+U очистить   Enter продолжить   Esc отменить",
                },
                _ => "Ctrl+U очистить   Enter продолжить   Esc отменить",
            };
            frame.render_widget(Paragraph::new(hint).style(fg(MUTED)), sections[2]);
            if input_inner.width > 0 && input_inner.height > 0 {
                frame.set_cursor_position((
                    input_inner.x
                        + (text_width.saturating_sub(usize::from(offset)) as u16)
                            .min(input_inner.width - 1),
                    input_inner.y,
                ));
            }
        }
        Popup::Confirm { body, .. } => {
            let sections =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
            frame.render_widget(
                Paragraph::new(body.as_str())
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                sections[0],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Esc / n  отменить", fg(SOFT)),
                    Span::raw("     "),
                    Span::styled("Enter / y  применить", bold(YELLOW)),
                ])),
                sections[1],
            );
        }
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    )
}
fn short(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}
fn date(value: &str) -> String {
    short(value, 16).replace('T', " ")
}
fn duration(value: Option<&Value>) -> String {
    match value.and_then(Value::as_f64) {
        Some(seconds) if seconds.is_finite() && seconds >= 0.0 => {
            let seconds = seconds as u64;
            if seconds >= 60 {
                format!("{}м {:02}с", seconds / 60, seconds % 60)
            } else {
                format!("{seconds}с")
            }
        }
        _ => "—".into(),
    }
}
fn status_color(status: &str) -> Color {
    match status {
        "success" | "passed" | "opened" => GREEN,
        "failed" => RED,
        "running" | "pending" => CYAN,
        "manual" | "draft" | "scheduled" => YELLOW,
        "canceled" | "skipped" => MUTED,
        "private" | "internal" | "public" => PURPLE,
        _ => SOFT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Request;
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;

    fn render(state: &State, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, state)).unwrap();
        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .chunks(usize::from(width))
            .map(|line| line.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn state() -> State {
        let mut state = State::new("gitlab.example.org".into(), true);
        state.rows = vec![
            json!({"id":1,"name":"api-gateway","path_with_namespace":"platform/core/api-gateway","visibility":"private","description":"Единая точка входа","default_branch":"main"}),
        ];
        state
    }

    #[test]
    fn renders_catalog_and_responsive_focus() {
        let mut state = state();
        for (width, height) in [(160, 48), (100, 32)] {
            let text = render(&state, width, height);
            assert!(text.contains("api-gateway"));
            assert!(text.contains("GitLab Deck"));
            assert!(text.contains("Проекты"));
        }
        state.focus = Focus::Sidebar;
        assert!(render(&state, 100, 32).contains("Избранное"));
        state.focus = Focus::Details;
        assert!(render(&state, 100, 32).contains("Предпросмотр"));
        let small = render(&state, 72, 22);
        assert!(small.contains("80 × 24"));
    }

    #[test]
    fn text_scroll_does_not_count_wrapped_rows() {
        let mut state = state();
        state.context.view = View::File;
        state.text = format!("{}\nTARGET_LINE\nTHE_END", "x".repeat(1000));
        state.scroll = 1;
        let text = render(&state, 100, 32);
        assert!(text.contains("TARGET_LINE"));
        assert!(text.contains("THE_END"));
        state.scroll = 2;
        assert!(!render(&state, 100, 32).contains("TARGET_LINE"));
    }

    #[test]
    fn renders_log_settings_and_modals_without_overflow() {
        let mut state = state();
        state.context.project = state.rows.first().cloned();
        state.context.view = View::Trace;
        state.context.job = Some(json!({"id":9271,"name":"unit-tests","status":"success"}));
        state.text = "Running tests\n38 passed\nerror: пример".into();
        assert!(render(&state, 160, 48).contains("38 passed"));
        state.context.view = View::Settings;
        state.object =
            json!({"name":"api-gateway","description":"Описание","visibility":"private"});
        assert!(render(&state, 160, 48).contains("Название проекта"));
        state.selected = SETTINGS.len() - 1;
        assert!(render(&state, 100, 32).contains("Тайм-аут"));
        for popup in [
            Popup::Help,
            Popup::Input {
                title: "Описание".into(),
                value: "Текст 日本語".repeat(20),
                purpose: crate::model::InputPurpose::Setting(1),
            },
            Popup::Confirm {
                title: "Применить изменение?".into(),
                body: "Проект api-gateway\nОписание: старое → новое".into(),
                request: Request::post("projects/1"),
            },
        ] {
            state.popup = Some(popup);
            for (width, height) in [(160, 48), (100, 32), (80, 24), (72, 22)] {
                assert!(!render(&state, width, height).is_empty());
            }
        }
    }
}
