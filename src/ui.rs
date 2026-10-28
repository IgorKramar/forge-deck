use crate::model::{Focus, Popup, SettingKind, State, View, number, string, value_label};
use crate::{i18n::Language, provider::Provider};
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
    let language = state.language;
    let area = frame.area();
    frame.render_widget(Block::default().style(fg(TEXT).bg(BG)), area);
    if area.width < 80 || area.height < 24 {
        let text = Text::from(vec![
            Line::styled("Forge Deck", bold(BLUE)),
            Line::from(""),
            Line::from(language.text(
                "Resize the terminal to at least 80 × 24.",
                "Увеличьте терминал минимум до 80 × 24.",
            )),
            Line::styled(
                crate::tr!(
                    language,
                    "Current size: {} × {}",
                    "Сейчас: {} × {}",
                    area.width,
                    area.height
                ),
                fg(MUTED),
            ),
            Line::from(""),
            Line::styled(language.text("q — quit", "q — выход"), fg(MUTED)),
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
        popup_draw(frame, area, popup, state.popup_scroll, state);
    }
}

fn header(frame: &mut Frame, area: Rect, state: &State) {
    let language = state.language;
    let parts = Layout::horizontal([
        Constraint::Length(18),
        Constraint::Min(10),
        Constraint::Length(16),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(Line::styled(" Forge Deck", bold(BLUE))),
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
        Span::styled(
            crate::tr!(language, "{spinner} loading", "{spinner} загрузка"),
            fg(CYAN),
        )
    } else if state.demo {
        Span::styled(
            format!(
                "{} {} ",
                state.provider.name(),
                language.text("DEMO", "ДЕМО")
            ),
            bold(GREEN),
        )
    } else {
        Span::styled(format!(" {} ", state.provider.name()), fg(PURPLE))
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            label,
            Span::styled(format!("· {}", language.code().to_uppercase()), fg(MUTED)),
        ]))
        .alignment(Alignment::Right),
        parts[2],
    );
}

fn tabs(frame: &mut Frame, area: Rect, state: &State) {
    let language = state.language;
    let current = state.context.view;
    let items = [
        (
            "o",
            language.text("Overview", "Обзор"),
            matches!(current, View::Overview),
        ),
        (
            "f",
            language.text("Files", "Файлы"),
            matches!(current, View::Repository | View::File | View::Branches),
        ),
        (
            "m",
            if state.provider == Provider::Github {
                "PR"
            } else {
                "MR"
            },
            matches!(
                current,
                View::MergeRequests | View::MrDetails | View::MrDiff | View::MrNotes
            ),
        ),
        (
            "p",
            if state.provider == Provider::Github {
                "Actions"
            } else {
                "CI / CD"
            },
            matches!(current, View::Pipelines | View::Jobs | View::Trace),
        ),
        (
            "s",
            language.text("Settings", "Настройки"),
            matches!(current, View::Settings),
        ),
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
    let language = state.language;
    let block = panel(
        language.text(" Browse ", " Обзор "),
        state.focus == Focus::Sidebar,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let labels = [
        if state.provider == Provider::Github {
            language.text("  Repositories", "  Репозитории")
        } else {
            language.text("  Projects", "  Проекты")
        },
        if state.provider == Provider::Github {
            language.text("  Organizations", "  Организации")
        } else {
            language.text("  Groups", "  Группы")
        },
        if state.provider == Provider::Github {
            language.text("  My PRs", "  Мои PR")
        } else {
            language.text("  All MRs", "  Все MR")
        },
        language.text("  Favorites", "  Избранное"),
        language.text("󰒓  Server", "󰒓  Сервер"),
    ];
    let mut lines = vec![
        Line::styled(language.text("WORKSPACE", "ПРОСТРАНСТВО"), fg(MUTED)),
        Line::from(""),
    ];
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
            Line::styled(language.text("CURRENT GROUP", "ТЕКУЩАЯ ГРУППА"), fg(MUTED)),
            Line::from(""),
            Line::styled(string(group, "full_path").to_owned(), fg(PURPLE)),
        ]);
    }
    if let Some(project) = &state.context.project {
        lines.extend([
            Line::from(""),
            Line::styled(language.text("PROJECT", "ПРОЕКТ"), fg(MUTED)),
            Line::from(""),
            Line::styled(string(project, "name").to_owned(), fg(BLUE)),
        ]);
    }
    if inner.height >= 25 {
        lines.extend([
            Line::from(""),
            Line::styled(
                language.text("Tab — change focus", "Tab — сменить фокус"),
                fg(MUTED),
            ),
            Line::styled(
                language.text("1 / 2 / 3 — sections", "1 / 2 / 3 — разделы"),
                fg(MUTED),
            ),
        ]);
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn list_view(frame: &mut Frame, area: Rect, state: &State) {
    let language = state.language;
    let title = format!(
        " {}  {} ",
        state.title(state.context.view),
        state.rows.len()
    );
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
        language.text("  / — search", "  / — поиск").into()
    } else {
        format!("  {}", state.context.query)
    };
    frame.render_widget(Paragraph::new(query).style(fg(MUTED)), sections[0]);
    if state.rows.is_empty() {
        empty(frame, sections[1], state);
        return;
    }
    let compact = inner.width < 75;
    let (headers, widths) = table_columns(state.context.view, compact, language, state.provider);
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
        View::Repository => crate::tr!(
            language,
            "{}  ·  page {}",
            "{}  ·  страница {}",
            if state.context.path.is_empty() {
                "/"
            } else {
                &state.context.path
            },
            state.context.page
        ),
        View::Favorites => crate::tr!(
            language,
            "{} favorite projects",
            "{} избранных проектов",
            state.rows.len()
        ),
        _ => crate::tr!(
            language,
            "Page {}  ·  up to {} entries  ·  n / N — browse",
            "Страница {}  ·  до {} записей  ·  n / N — листать",
            state.context.page,
            crate::model::PAGE_SIZE
        ),
    };
    frame.render_widget(Paragraph::new(hint).style(fg(MUTED)), sections[2]);
}

fn table_columns(
    view: View,
    compact: bool,
    language: Language,
    provider: Provider,
) -> (Vec<&'static str>, Vec<Constraint>) {
    let (a, b, c) = match view {
        View::Projects | View::Favorites => (
            if provider == Provider::Github {
                language.text("REPOSITORY", "РЕПОЗИТОРИЙ")
            } else {
                language.text("PROJECT", "ПРОЕКТ")
            },
            language.text("VISIBILITY", "ДОСТУП"),
            language.text("UPDATED", "ОБНОВЛЁН"),
        ),
        View::Groups => (
            if provider == Provider::Github {
                language.text("ORGANIZATION", "ОРГАНИЗАЦИЯ")
            } else {
                language.text("GROUP", "ГРУППА")
            },
            language.text("VISIBILITY", "ДОСТУП"),
            "ID",
        ),
        View::MergeRequests => (
            if provider == Provider::Github {
                "PULL REQUEST"
            } else {
                "MERGE REQUEST"
            },
            language.text("STATUS", "СТАТУС"),
            language.text("AUTHOR", "АВТОР"),
        ),
        View::Pipelines => (
            if provider == Provider::Github {
                language.text("WORKFLOW / BRANCH", "WORKFLOW / ВЕТКА")
            } else {
                language.text("PIPELINE / BRANCH", "ПАЙПЛАЙН / ВЕТКА")
            },
            language.text("STATUS", "СТАТУС"),
            if provider == Provider::Github {
                language.text("STARTED", "ЗАПУЩЕН")
            } else {
                language.text("DURATION", "ВРЕМЯ")
            },
        ),
        View::Jobs => (
            language.text("JOB / STAGE", "ДЖОБА / ЭТАП"),
            language.text("STATUS", "СТАТУС"),
            if provider == Provider::Github {
                language.text("STARTED", "ЗАПУЩЕН")
            } else {
                language.text("DURATION", "ВРЕМЯ")
            },
        ),
        View::Repository => (
            language.text("NAME", "ИМЯ"),
            language.text("TYPE", "ТИП"),
            language.text("MODE", "РЕЖИМ"),
        ),
        View::Branches => (
            language.text("BRANCH", "ВЕТКА"),
            language.text("PROTECTION", "ЗАЩИТА"),
            language.text("COMMIT", "КОММИТ"),
        ),
        _ => (
            language.text("NAME", "ИМЯ"),
            language.text("STATUS", "СТАТУС"),
            "ID",
        ),
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
    let language = state.language;
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
            format!(
                "{}{}  {}",
                state.provider.mr_prefix(),
                number(row, "iid"),
                string(row, "title")
            ),
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
            if state.provider == Provider::Github {
                date(string(
                    row,
                    if view == View::Jobs {
                        "started_at"
                    } else {
                        "created_at"
                    },
                ))
            } else {
                duration(row.get("duration"), language)
            },
        ),
        View::Jobs => (
            string(row, "name").into(),
            string(row, "stage").into(),
            string(row, "status").into(),
            if state.provider == Provider::Github {
                date(string(
                    row,
                    if view == View::Jobs {
                        "started_at"
                    } else {
                        "created_at"
                    },
                ))
            } else {
                duration(row.get("duration"), language)
            },
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
                language.text("directory", "каталог").into()
            } else {
                language.text("file", "файл").into()
            },
            string(row, "mode").into(),
        ),
        View::Branches => (
            format!(" {}", string(row, "name")),
            if row.get("default").and_then(Value::as_bool).unwrap_or(false) {
                language.text("default branch", "основная ветка").into()
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
                language.text("unprotected", "обычная").into()
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
    let language = state.language;
    let (title, hint) = if state.loading {
        (
            language.text("Loading data…", "Загрузка данных…"),
            language.text(
                "Keep navigating or press q to quit",
                "Можно продолжать навигацию или выйти по q",
            ),
        )
    } else if state.error.is_some() {
        (
            language.text("Could not load data", "Не удалось загрузить данные"),
            language.text(
                "F5 — retry reading · H — change server",
                "F5 — повторить чтение · H — сменить сервер",
            ),
        )
    } else if state.context.view == View::Favorites {
        (
            language.text("No favorite projects yet", "Пока нет избранных проектов"),
            language.text(
                "Open the project catalog and press * to add a favorite",
                "Откройте каталог и нажмите * на нужном проекте",
            ),
        )
    } else {
        (
            language.text("Nothing found", "Ничего не найдено"),
            language.text(
                "Try another search or the previous page",
                "Попробуйте другой поиск или предыдущую страницу",
            ),
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
    let language = state.language;
    let project = if state.object.is_object() {
        &state.object
    } else {
        state.context.project.as_ref().unwrap_or(&Value::Null)
    };
    let block = panel(
        if state.provider == Provider::Github {
            language.text("  Repository overview ", "  Обзор репозитория ")
        } else {
            language.text("  Project overview ", "  Обзор проекта ")
        },
        state.focus == Focus::Main,
    );
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
    lines.extend(project_facts(project, language, state.provider));
    lines.extend([
        Line::from(""),
        Line::styled(
            language.text("PROJECT SECTIONS", "РАЗДЕЛЫ ПРОЕКТА"),
            fg(MUTED),
        ),
        Line::from(""),
        Line::from(language.text(
            if state.provider == Provider::Github {
                "f  files       m  pull requests        p  Actions"
            } else {
                "f  files       m  merge requests       p  pipelines"
            },
            if state.provider == Provider::Github {
                "f  файлы       m  pull requests        p  Actions"
            } else {
                "f  файлы       m  merge requests       p  пайплайны"
            },
        )),
        Line::from(language.text(
            "b  branches    s  settings             Esc  back",
            "b  ветки       s  настройки            Esc  назад",
        )),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((state.scroll, 0)),
        inner,
    );
}

fn text_view(frame: &mut Frame, area: Rect, state: &State) {
    let language = state.language;
    let title = match state.context.view {
        View::Trace => state.context.job.as_ref().map(|v| {
            crate::tr!(
                language,
                "  {} · job log #{} ",
                "  {} · лог #{} ",
                string(v, "name"),
                number(v, "id")
            )
        }),
        View::File => state
            .context
            .file
            .as_ref()
            .map(|v| format!(" 󰈙 {} ", string(v, "path"))),
        View::MrDetails | View::MrDiff | View::MrNotes => state.context.mr.as_ref().map(|v| {
            format!(
                " {}{} · {} ",
                state.provider.mr_prefix(),
                number(v, "iid"),
                state.title(state.context.view)
            )
        }),
        _ => None,
    }
    .unwrap_or_else(|| format!(" {} ", state.title(state.context.view)));
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
    let language = state.language;
    let block = panel(
        if state.provider == Provider::Github {
            language.text(" 󰒓 Repository settings ", " 󰒓 Настройки репозитория ")
        } else {
            language.text(" 󰒓 Project settings ", " 󰒓 Настройки проекта ")
        },
        state.focus == Focus::Main,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = usize::from(inner.height / 4).max(1);
    let selected = state.selected.min(state.settings().len() - 1);
    let start = selected
        .saturating_sub(visible / 2)
        .min(state.settings().len().saturating_sub(visible));
    for (position, setting) in state
        .settings()
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
    {
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
            .map(|value| display_setting(value, language))
            .unwrap_or_else(|| {
                language
                    .text("unavailable on this server", "недоступно на этом сервере")
                    .into()
            });
        let label = format!(
            " {} · {} ",
            setting.group_in(language),
            setting.label_in(language)
        );
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

fn display_setting(value: &Value, language: Language) -> String {
    match value {
        Value::Bool(true) => language.text("  enabled", "  включено").into(),
        Value::Bool(false) => language.text("  disabled", "  выключено").into(),
        _ => value_label(value),
    }
}

fn details(frame: &mut Frame, area: Rect, state: &State) {
    let language = state.language;
    let title = if state.context.view == View::Settings {
        language.text(" About this field ", " О поле ")
    } else {
        language.text(" Preview ", " Предпросмотр ")
    };
    let block = panel(title, state.focus == Focus::Details);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = if state.context.view == View::Settings {
        let setting = &state.settings()[state.selected.min(state.settings().len() - 1)];
        let mut lines = vec![
            Line::styled(setting.label_in(language), bold(BLUE)),
            Line::from(""),
            Line::from(setting.help_in(language)),
            Line::from(""),
        ];
        if let SettingKind::Choice(values) = setting.kind {
            lines.extend([
                Line::styled(
                    language.text("Allowed values", "Допустимые значения"),
                    fg(MUTED),
                ),
                Line::from(values.join(" / ")),
                Line::from(""),
            ]);
        }
        lines.extend([
            Line::styled(setting.key, fg(MUTED)),
            Line::from(""),
            Line::from(language.text("e / Enter — edit", "e / Enter — изменить")),
            Line::from(""),
            Line::styled(
                language.text(
                    "Saving requires confirmation.",
                    "Сохранение потребует подтверждения.",
                ),
                fg(YELLOW),
            ),
        ]);
        lines
    } else if let Some(row) = match state.context.view {
        View::Trace => state.context.job.as_ref(),
        View::File => state.context.file.as_ref(),
        View::MrDetails | View::MrDiff | View::MrNotes => state.context.mr.as_ref(),
        _ => state.selected().or(state.context.project.as_ref()),
    } {
        match state.context.view {
            View::Projects | View::Favorites => project_preview(row, language, state.provider),
            View::Groups => vec![
                Line::styled(string(row, "name").to_owned(), bold(BLUE)),
                Line::from(""),
                Line::from(string(row, "full_path").to_owned()),
                Line::from(""),
                Line::from(string(row, "description").to_owned()),
                Line::from(""),
                Line::styled(
                    language.text("Enter — group projects", "Enter — проекты группы"),
                    fg(MUTED),
                ),
            ],
            View::Pipelines | View::Jobs | View::Trace => ci_preview(row, state),
            View::MergeRequests | View::MrDetails | View::MrDiff | View::MrNotes => {
                mr_preview(row, language, state.provider)
            }
            View::Repository | View::File => vec![
                Line::styled(string(row, "name").to_owned(), bold(BLUE)),
                Line::from(""),
                Line::from(string(row, "path").to_owned()),
                Line::from(""),
                Line::styled(language.text("Enter — open", "Enter — открыть"), fg(MUTED)),
            ],
            _ => vec![
                Line::styled(state.title(state.context.view), fg(MUTED)),
                Line::from(""),
                Line::from(string(row, "name").to_owned()),
            ],
        }
    } else {
        vec![Line::styled(
            language.text("Select an entry", "Выберите строку"),
            fg(MUTED),
        )]
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

fn project_preview(project: &Value, language: Language, provider: Provider) -> Vec<Line<'static>> {
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
    lines.extend(project_facts(project, language, provider));
    lines.extend([
        Line::from(""),
        Line::styled(
            language.text("Enter — open project", "Enter — открыть проект"),
            fg(MUTED),
        ),
    ]);
    lines
}

fn project_facts(project: &Value, language: Language, provider: Provider) -> Vec<Line<'static>> {
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
    let role = if provider == Provider::Github {
        if project.pointer("/permissions/admin") == Some(&Value::Bool(true)) {
            "Admin"
        } else if project.pointer("/permissions/push") == Some(&Value::Bool(true)) {
            "Write"
        } else if project.pointer("/permissions/pull") == Some(&Value::Bool(true)) {
            "Read"
        } else {
            "—"
        }
    } else {
        match role {
            Some(50..) => "Owner",
            Some(40..) => "Maintainer",
            Some(30..) => "Developer",
            Some(20..) => "Reporter",
            Some(_) => "Guest",
            None => "—",
        }
    };
    vec![
        fact(
            language.text("Visibility", "Доступ"),
            string(project, "visibility"),
        ),
        fact(language.text("Role", "Роль"), role),
        fact(
            language.text("Branch", "Ветка"),
            string(project, "default_branch"),
        ),
        fact(
            language.text("Project ID", "ID проекта"),
            &number(project, "id").to_string(),
        ),
        fact(
            language.text("Updated", "Обновлён"),
            &date(string(project, "last_activity_at")),
        ),
    ]
}

fn ci_preview(item: &Value, state: &State) -> Vec<Line<'static>> {
    let language = state.language;
    let status = string(item, "status");
    let mut lines = vec![
        Line::styled(format!("#{}", number(item, "id")), bold(BLUE)),
        Line::from(""),
        Line::styled(status.to_owned(), fg(status_color(status))),
        Line::from(""),
        fact(
            language.text("Duration", "Время"),
            &duration(item.get("duration"), language),
        ),
        fact(language.text("Stage", "Этап"), string(item, "stage")),
        fact(language.text("Branch", "Ветка"), string(item, "ref")),
        Line::from(""),
    ];
    if let Some(pipeline) = &state.context.pipeline {
        lines.extend([
            fact(
                if state.provider == Provider::Github {
                    "Workflow"
                } else {
                    language.text("Pipeline", "Пайплайн")
                },
                &format!("#{}", number(pipeline, "id")),
            ),
            Line::from(""),
        ]);
    }
    lines.extend([
        Line::styled(language.text("r — retry", "r — повторить"), fg(MUTED)),
        Line::styled(
            if state.provider == Provider::Github
                && matches!(state.context.view, View::Jobs | View::Trace)
            {
                language.text("Cancel from the workflow run", "Отмена в списке запусков")
            } else {
                language.text("x — cancel", "x — отменить")
            },
            fg(MUTED),
        ),
        Line::styled(
            if state.provider == Provider::Github {
                ""
            } else {
                language.text("S — play manual job", "S — ручной запуск")
            },
            fg(MUTED),
        ),
        Line::from(""),
        Line::styled(
            language.text(
                "Actions require confirmation.",
                "Действия требуют подтверждения.",
            ),
            fg(YELLOW),
        ),
    ]);
    lines
}

fn mr_preview(item: &Value, language: Language, provider: Provider) -> Vec<Line<'static>> {
    vec![
        Line::styled(
            format!("{}{}", provider.mr_prefix(), number(item, "iid")),
            bold(BLUE),
        ),
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
        Line::styled(language.text("d — diff", "d — дифф"), fg(MUTED)),
        Line::styled(
            language.text("c — discussions", "c — обсуждения"),
            fg(MUTED),
        ),
    ]
}

fn fact(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), fg(MUTED)),
        Span::styled(value.to_owned(), fg(SOFT)),
    ])
}

fn footer(frame: &mut Frame, area: Rect, state: &State) {
    let language = state.language;
    let contextual = match state.context.view {
        View::Projects | View::Groups | View::Favorites => language.text(
            "Enter open   / search   * favorite   n/N pages",
            "Enter открыть   / поиск   * избранное   n/N страницы",
        ),
        View::Pipelines => language.text(
            "Enter jobs   r retry   x cancel   n/N pages",
            "Enter джобы   r повторить   x отменить   n/N страницы",
        ),
        View::Jobs if state.provider == Provider::Github => language.text(
            "Enter log   r rerun job + dependents   n/N pages",
            "Enter лог   r повторить с зависимыми   n/N страницы",
        ),
        View::Jobs => language.text(
            "Enter log   r retry   x cancel   S play",
            "Enter лог   r повторить   x отменить   S запуск",
        ),
        View::Settings => language.text(
            "e / Enter edit   j/k field   Esc back",
            "e / Enter изменить   j/k поле   Esc назад",
        ),
        View::MergeRequests | View::MrDetails | View::MrDiff | View::MrNotes => language.text(
            "Enter details   d diff   c discussions   / search",
            "Enter подробно   d дифф   c обсуждения   / поиск",
        ),
        View::Trace | View::File => language.text(
            "j/k ↑↓ scroll   ←→ horizontal   / search   F5 refresh   Esc back",
            "j/k ↑↓ текст   ←→ по горизонтали   / поиск   F5 обновить   Esc назад",
        ),
        View::Repository => language.text(
            "Enter open   j/k select   n/N pages   Esc back",
            "Enter открыть   j/k выбор   n/N страницы   Esc назад",
        ),
        _ => language.text(
            "Enter open   j/k select   / search   Esc back",
            "Enter открыть   j/k выбор   / поиск   Esc назад",
        ),
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(contextual, fg(BLUE)),
            Line::styled(
                language.text("Tab focus   1/2/3 browse   G forge   H host   L en/ru   ? help   q quit", "Tab фокус   1/2/3 обзор   G платформа   H сервер   L en/ru   ? помощь   q выход"),
                fg(MUTED),
            ),
        ]),
        area,
    );
}

fn status_line(frame: &mut Frame, area: Rect, state: &State) {
    let language = state.language;
    let line = if let Some(error) = &state.error {
        Line::styled(format!(" {}", error.replace(['\n', '\r'], " ")), fg(RED))
    } else if !state.status.is_empty() {
        Line::styled(state.status.replace(['\n', '\r'], " "), fg(YELLOW))
    } else {
        Line::styled(
            crate::tr!(
                language,
                "{}  ·  {} entries  ·  {}",
                "{}  ·  {} записей  ·  {}",
                state.title(state.context.view),
                state.rows.len(),
                if state.demo {
                    language.text("demo data", "демонстрационные данные")
                } else {
                    language.text("F5 / R — refresh", "F5 / R — обновить")
                }
            ),
            fg(MUTED),
        )
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn popup_draw(frame: &mut Frame, area: Rect, popup: &Popup, scroll: u16, state: &State) {
    let language = state.language;
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
        Popup::Help => language.text(" ? Keybindings ", " ? Клавиши "),
        Popup::Input { title, .. } | Popup::Confirm { title, .. } => title.as_str(),
    };
    let block = panel(title.to_owned(), true).style(fg(TEXT).bg(SURFACE));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    match popup {
        Popup::Help => {
            let lines = [
                language.text(
                    "j / k, ↑ / ↓     select an entry or scroll text",
                    "j / k, ↑ / ↓     выбор строки или прокрутка текста",
                ),
                language.text(
                    "← / → in text    scroll horizontally",
                    "← / → в тексте   горизонтальная прокрутка",
                ),
                language.text(
                    "Tab              focus: menu → content → details",
                    "Tab              фокус: меню → содержимое → детали",
                ),
                language.text(
                    if state.provider == Provider::Github {
                        "1 / 2 / 3        repositories / organizations / my PRs"
                    } else {
                        "1 / 2 / 3        projects / groups / all MRs"
                    },
                    if state.provider == Provider::Github {
                        "1 / 2 / 3        репозитории / организации / мои PR"
                    } else {
                        "1 / 2 / 3        проекты / группы / все MR"
                    },
                ),
                language.text(
                    if state.provider == Provider::Github {
                        "o / f / m        overview / files / repository PRs"
                    } else {
                        "o / f / m        overview / files / project MRs"
                    },
                    if state.provider == Provider::Github {
                        "o / f / m        обзор / файлы / PR репозитория"
                    } else {
                        "o / f / m        обзор / файлы / MR текущего проекта"
                    },
                ),
                language.text(
                    "p / b / s        CI / branches / project settings",
                    "p / b / s        CI / ветки / настройки проекта",
                ),
                language.text(
                    "Enter            open selection; edit a setting",
                    "Enter            открыть выбранное; изменить поле настроек",
                ),
                language.text(
                    "Esc / Backspace  go back",
                    "Esc / Backspace  вернуться назад",
                ),
                language.text(
                    "/                search; Enter apply, Esc cancel",
                    "/                поиск; Enter применить, Esc отменить",
                ),
                language.text(
                    "n / N, [ / ]     pages; n/N in text — next/previous match",
                    "n / N, [ / ]     страницы; в тексте n/N — поиск далее/назад",
                ),
                language.text(
                    if state.provider == Provider::Github {
                        "d / c            pull request diff / general comments"
                    } else {
                        "d / c            merge request diff / discussions"
                    },
                    if state.provider == Provider::Github {
                        "d / c            дифф / общие комментарии pull request"
                    } else {
                        "d / c            дифф / обсуждения merge request"
                    },
                ),
                language.text(
                    if state.provider == Provider::Github {
                        "r / x            rerun / cancel workflow run (x runs only)"
                    } else {
                        "r / x / S        retry / cancel CI / play a manual job"
                    },
                    if state.provider == Provider::Github {
                        "r / x            повтор / отмена workflow (x только запуск)"
                    } else {
                        "r / x / S        повтор / отмена CI / запуск ручной джобы"
                    },
                ),
                language.text(
                    "F5 / R           refresh the current screen",
                    "F5 / R           перечитать текущий экран",
                ),
                language.text(
                    "*                add / remove a favorite project",
                    "*                добавить / убрать избранный проект",
                ),
                language.text(
                    "e                edit the selected setting",
                    "e                редактировать выбранную настройку",
                ),
                language.text(
                    "G / H            switch forge / connect to another server",
                    "G / H            сменить платформу / подключиться к серверу",
                ),
                language.text(
                    "L                switch English / Russian",
                    "L                переключить русский / английский",
                ),
                language.text(
                    "Enter / y        confirm action; Esc / n — cancel",
                    "Enter / y        подтвердить действие; Esc / n — отменить",
                ),
                language.text(
                    "q / Ctrl+C       quit; ? / Esc — close this help",
                    "q / Ctrl+C       выход; ? / Esc — закрыть эту помощь",
                ),
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
                crate::model::InputPurpose::Setting(index) => match state.settings()[*index].kind {
                    SettingKind::Choice(_) | SettingKind::Bool => language.text(
                        "← / → select   Enter continue   Esc cancel",
                        "← / → выбрать   Enter продолжить   Esc отменить",
                    ),
                    _ => language.text(
                        "Ctrl+U clear   Enter continue   Esc cancel",
                        "Ctrl+U очистить   Enter продолжить   Esc отменить",
                    ),
                },
                _ => language.text(
                    "Ctrl+U clear   Enter continue   Esc cancel",
                    "Ctrl+U очистить   Enter продолжить   Esc отменить",
                ),
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
                    Span::styled(
                        language.text("Esc / n  cancel", "Esc / n  отменить"),
                        fg(SOFT),
                    ),
                    Span::raw("     "),
                    Span::styled(
                        language.text("Enter / y  apply", "Enter / y  применить"),
                        bold(YELLOW),
                    ),
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
fn duration(value: Option<&Value>, language: Language) -> String {
    match value.and_then(Value::as_f64) {
        Some(seconds) if seconds.is_finite() && seconds >= 0.0 => {
            let seconds = seconds as u64;
            if seconds >= 60 {
                crate::tr!(
                    language,
                    "{}m {:02}s",
                    "{}м {:02}с",
                    seconds / 60,
                    seconds % 60
                )
            } else {
                crate::tr!(language, "{seconds}s", "{seconds}с")
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
    use crate::model::SETTINGS;
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
        state.language = Language::Ru;
        state.rows = vec![
            json!({"id":1,"name":"api-gateway","path_with_namespace":"platform/core/api-gateway","visibility":"private","description":"Единая точка входа","default_branch":"main"}),
        ];
        state
    }

    #[test]
    fn github_labels_use_native_terms_and_available_actions() {
        let mut state = state();
        state.provider = Provider::Github;
        state.host = "github.example.org".into();
        for language in [Language::En, Language::Ru] {
            state.language = language;
            let text = render(&state, 160, 48);
            assert!(text.contains(language.text("Repositories", "Репозитории")));
            assert!(text.contains(language.text("Organizations", "Организации")));
            assert!(text.contains(language.text("My PRs", "Мои PR")));
            state.context.view = View::Jobs;
            let text = render(&state, 160, 48);
            assert!(!text.contains("S play") && !text.contains("S запуск"));
            state.popup = Some(Popup::Help);
            let text = render(&state, 160, 48);
            assert!(text.contains(language.text("switch forge", "сменить платформу")));
            state.popup = None;
            state.context.view = View::Projects;
        }
    }
    #[test]
    fn english_labels_are_selected_without_translating_content() {
        let mut state = state();
        state.language = crate::i18n::Language::En;
        state.rows[0]["description"] = json!("User project content");
        let text = render(&state, 160, 48);
        assert!(text.contains("Projects"));
        assert!(text.contains("User project content"));
        assert!(
            !text
                .chars()
                .any(|c| ('А'..='я').contains(&c) || c == 'Ё' || c == 'ё')
        );
        state.language = crate::i18n::Language::Ru;
        assert!(render(&state, 160, 48).contains("Проекты"));
    }

    #[test]
    fn every_screen_and_dialog_localizes_only_owned_text() {
        let mut state = state();
        state.language = Language::En;
        let row = json!({"id":1,"iid":7,"name":"sample-project","path_with_namespace":"group/sample-project","description":"User content","title":"Merge request title","visibility":"private","default_branch":"main","status":"success","state":"opened","type":"tree","path":"src","mode":"040000","ref":"main","duration":125,"source_branch":"feature","target_branch":"main","author":{"username":"someone"}});
        state.rows = vec![row.clone()];
        state.context.project = Some(row.clone());
        state.context.mr = Some(row.clone());
        state.context.job = Some(row.clone());
        state.context.file = Some(row.clone());
        state.context.pipeline = Some(row.clone());
        state.object = row;
        state.text = "User log output\nSecond line".into();
        let assert_english = |text: String| {
            assert!(
                !text
                    .chars()
                    .any(|c| ('А'..='я').contains(&c) || matches!(c, 'Ё' | 'ё')),
                "{text}"
            );
        };
        for provider in [Provider::Gitlab, Provider::Github] {
            state.provider = provider;
            for view in [
                View::Projects,
                View::Groups,
                View::Favorites,
                View::Overview,
                View::Repository,
                View::File,
                View::MergeRequests,
                View::MrDetails,
                View::MrDiff,
                View::MrNotes,
                View::Pipelines,
                View::Jobs,
                View::Trace,
                View::Settings,
                View::Branches,
            ] {
                state.context.view = view;
                for (width, height) in [(160, 48), (100, 32), (80, 24), (72, 22)] {
                    assert_english(render(&state, width, height));
                }
            }
            state.context.view = View::Settings;
            for (index, setting) in state.settings().iter().enumerate() {
                state.selected = index;
                state.object[setting.key] = match setting.kind {
                    SettingKind::Text { .. } => json!("Plain text"),
                    SettingKind::Choice(values) => json!(values[0]),
                    SettingKind::Bool => json!(true),
                    SettingKind::Number { min } => json!(min),
                };
                state.focus = Focus::Details;
                assert_english(render(&state, 160, 48));
                state.focus = Focus::Main;
                assert_english(render(&state, 100, 32));
            }
        }
        state.provider = Provider::Gitlab;
        for popup in [
            Popup::Help,
            Popup::Input {
                title: "Project description".into(),
                value: "Unchanged value".into(),
                purpose: crate::model::InputPurpose::Setting(1),
            },
            Popup::Confirm {
                title: "Apply change?".into(),
                body: "Project: group/sample-project\nBefore: old\nAfter: new".into(),
                request: Request::post("projects/1/jobs/2/retry"),
            },
        ] {
            state.popup = Some(popup);
            for (width, height) in [(160, 48), (100, 32), (80, 24), (72, 22)] {
                assert_english(render(&state, width, height));
            }
        }
        state.popup = None;
        state.context.view = View::Favorites;
        state.rows.clear();
        for loading in [false, true] {
            state.loading = loading;
            assert_english(render(&state, 100, 32));
        }
        state.loading = false;
        state.error = Some("Network error".into());
        assert_english(render(&state, 160, 48));
        state.context.view = View::Projects;
        state.rows = vec![
            json!({"id":1,"name":"Имя пользователя","description":"User content","path_with_namespace":"group/project"}),
        ];
        assert!(render(&state, 160, 48).contains("Имя пользователя"));
    }

    #[test]
    fn renders_catalog_and_responsive_focus() {
        let mut state = state();
        for (width, height) in [(160, 48), (100, 32)] {
            let text = render(&state, width, height);
            assert!(text.contains("api-gateway"));
            assert!(text.contains("Forge Deck"));
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
