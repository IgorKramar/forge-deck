use crate::{model::*, provider::Provider};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub enum Effect {
    Fetch {
        id: u64,
        request: Request,
        cancel: Arc<AtomicBool>,
    },
    SavePreferences,
    SwitchProvider,
    HostChanged,
    Exit,
}

#[derive(Clone)]
struct Snapshot {
    context: Context,
    rows: Vec<Value>,
    object: Value,
    text: String,
    selected: usize,
    scroll: u16,
    scroll_x: u16,
    can_next: bool,
}

pub struct App {
    pub state: State,
    history: Vec<Snapshot>,
    generation: u64,
    active: Option<(Arc<AtomicBool>, bool)>,
    can_next: bool,
    after_load: Option<String>,
    unknown_change: Option<String>,
}

impl App {
    pub fn new(host: String, demo: bool) -> Self {
        Self {
            state: State::new(host, demo),
            history: Vec::new(),
            generation: 0,
            active: None,
            can_next: false,
            after_load: None,
            unknown_change: None,
        }
    }

    pub fn cancel(&mut self) {
        if let Some((token, _)) = self.active.take() {
            token.store(true, Ordering::Relaxed);
        }
        self.generation = self.generation.wrapping_add(1);
        self.state.loading = false;
    }

    pub fn mutation_pending(&self) -> bool {
        self.active.as_ref().is_some_and(|(_, mutation)| *mutation)
    }

    fn fetch(&mut self, request: Request) -> Vec<Effect> {
        if request.is_mutation() {
            self.unknown_change = None;
            self.after_load = None;
        }
        self.cancel();
        let token = Arc::new(AtomicBool::new(false));
        self.active = Some((token.clone(), request.is_mutation()));
        self.state.loading = true;
        self.state.error = None;
        self.state.status = if request.is_mutation() {
            self.state
                .language
                .text("Sending change…", "Отправка изменения…")
        } else {
            self.state.language.text("Loading…", "Загрузка…")
        }
        .into();
        vec![Effect::Fetch {
            id: self.generation,
            request,
            cancel: token,
        }]
    }

    pub fn refresh(&mut self) -> Vec<Effect> {
        match self.request() {
            Ok(request) => self.fetch(request),
            Err(error) => {
                self.state.error = Some(error);
                Vec::new()
            }
        }
    }

    pub fn request(&self) -> Result<Request, String> {
        if self.state.provider == Provider::Github {
            return self.github_request();
        }

        let c = &self.state.context;
        let page = format!("per_page={PAGE_SIZE}&page={}", c.page);
        let search = encode(&c.query);
        let project = || {
            self.state.project_id().ok_or_else(|| {
                self.state
                    .language
                    .text("Select a project first", "Сначала выберите проект")
                    .to_owned()
            })
        };
        let list = match c.view {
            View::Projects => match &c.group {
                Some(group) => format!(
                    "groups/{}/projects?{page}&include_subgroups=true&with_shared=false&simple=true&order_by=last_activity_at&sort=desc&search={search}",
                    number(group, "id")
                ),
                None => format!(
                    "projects?{page}&simple=true&order_by=last_activity_at&sort=desc&search={search}"
                ),
            },
            View::Groups => {
                format!("groups?{page}&all_available=true&order_by=name&sort=asc&search={search}")
            }
            View::Favorites => {
                return Ok(Request::Projects(
                    self.state.favorites.iter().copied().collect(),
                ));
            }
            View::Overview | View::Settings => format!("projects/{}", project()?),
            View::Repository => {
                let branch = c
                    .project
                    .as_ref()
                    .map(|p| string(p, "default_branch"))
                    .filter(|s| *s != "—")
                    .unwrap_or("HEAD");
                format!(
                    "projects/{}/repository/tree?{page}&path={}&ref={}",
                    project()?,
                    encode(&c.path),
                    encode(branch)
                )
            }
            View::File => {
                return Ok(Request::text(format!(
                    "projects/{}/repository/blobs/{}/raw",
                    project()?,
                    encode(string(
                        c.file.as_ref().ok_or(
                            self.state
                                .language
                                .text("No file selected", "Файл не выбран")
                        )?,
                        "id"
                    ))
                )));
            }
            View::Branches => format!(
                "projects/{}/repository/branches?{page}&search={search}",
                project()?
            ),
            View::MergeRequests => {
                let prefix = self
                    .state
                    .project_id()
                    .map(|id| format!("projects/{id}/"))
                    .unwrap_or_default();
                format!(
                    "{prefix}merge_requests?{page}&state=opened&scope=all&order_by=updated_at&sort=desc&search={search}"
                )
            }
            View::MrDetails | View::MrDiff | View::MrNotes => {
                let iid = number(
                    c.mr.as_ref().ok_or(
                        self.state
                            .language
                            .text("No merge request selected", "MR не выбран"),
                    )?,
                    "iid",
                );
                let base = format!("projects/{}/merge_requests/{iid}", project()?);
                match c.view {
                    View::MrDiff => format!("{base}/diffs?{page}"),
                    View::MrNotes => format!("{base}/notes?{page}&sort=asc&order_by=created_at"),
                    _ => base,
                }
            }
            View::Pipelines => {
                let filter = if c.query.is_empty() {
                    String::new()
                } else {
                    format!("&ref={search}")
                };
                format!(
                    "projects/{}/pipelines?{page}&order_by=id&sort=desc{filter}",
                    project()?
                )
            }
            View::Jobs => format!(
                "projects/{}/pipelines/{}/jobs?{page}&include_retried=false",
                project()?,
                number(
                    c.pipeline.as_ref().ok_or(
                        self.state
                            .language
                            .text("No pipeline selected", "Пайплайн не выбран")
                    )?,
                    "id"
                )
            ),
            View::Trace => {
                return Ok(Request::text(format!(
                    "projects/{}/jobs/{}/trace",
                    project()?,
                    number(
                        c.job.as_ref().ok_or(
                            self.state
                                .language
                                .text("No job selected", "Джоба не выбрана")
                        )?,
                        "id"
                    )
                )));
            }
        };
        Ok(Request::get(list))
    }

    fn github_repo(&self) -> Result<String, String> {
        let full = self
            .state
            .context
            .project
            .as_ref()
            .and_then(|p| p["full_name"].as_str())
            .ok_or_else(|| {
                self.state
                    .language
                    .text(
                        "Select and open a repository",
                        "Выберите и откройте репозиторий",
                    )
                    .to_owned()
            })?;
        let parts: Vec<_> = full.split('/').collect();
        if parts.len() != 2
            || parts
                .iter()
                .any(|p| p.is_empty() || matches!(*p, "." | ".."))
        {
            return Err(self
                .state
                .language
                .text("Invalid repository name", "Некорректное имя репозитория")
                .into());
        }
        Ok(format!("repos/{}/{}", encode(parts[0]), encode(parts[1])))
    }

    fn github_request(&self) -> Result<Request, String> {
        let c = &self.state.context;
        let page = format!("per_page={PAGE_SIZE}&page={}", c.page);
        let search = encode(&c.query);
        let repo = || self.github_repo();
        let endpoint =
            match c.view {
                View::Projects if !c.query.is_empty() => {
                    let query = if let Some(group) = &c.group {
                        format!("org:{} {}", string(group, "full_path"), c.query)
                    } else {
                        c.query.clone()
                    };
                    format!("search/repositories?{page}&q={}", encode(&query))
                }
                View::Projects => {
                    if let Some(group) = &c.group {
                        format!(
                            "orgs/{}/repos?{page}&sort=updated&direction=desc",
                            encode(string(group, "full_path"))
                        )
                    } else {
                        format!("user/repos?{page}&sort=updated&direction=desc")
                    }
                }
                View::Groups => format!("user/orgs?{page}"),
                View::Favorites => {
                    return Ok(Request::Projects(
                        self.state.favorites.iter().copied().collect(),
                    ));
                }
                View::Overview | View::Settings => repo()?,
                View::Repository => {
                    let branch = c
                        .project
                        .as_ref()
                        .and_then(|p| p["default_branch"].as_str())
                        .unwrap_or("HEAD");
                    let path = c.path.split('/').map(encode).collect::<Vec<_>>().join("/");
                    format!("{}/contents/{path}?{page}&ref={}", repo()?, encode(branch))
                }
                View::File => {
                    return Ok(Request::text(format!(
                        "{}/git/blobs/{}",
                        repo()?,
                        encode(c.file.as_ref().and_then(|f| f["id"].as_str()).ok_or_else(
                            || {
                                self.state
                                    .language
                                    .text("No file selected", "Файл не выбран")
                                    .to_owned()
                            }
                        )?)
                    )));
                }
                View::Branches => format!("{}/branches?{page}", repo()?),
                View::MergeRequests if c.project.is_some() && c.query.is_empty() => format!(
                    "{}/pulls?{page}&state=open&sort=updated&direction=desc",
                    repo()?
                ),
                View::MergeRequests => {
                    let scope = c
                        .project
                        .as_ref()
                        .and_then(|p| p["full_name"].as_str())
                        .map(|name| format!("repo:{name}"))
                        .unwrap_or_else(|| "involves:@me".into());
                    format!(
                        "search/issues?{page}&sort=updated&order=desc&q={}",
                        encode(&format!("is:pr is:open {scope} {}", c.query))
                    )
                }
                View::MrDetails | View::MrDiff | View::MrNotes => {
                    let iid = c.mr.as_ref().map(|v| number(v, "iid")).ok_or_else(|| {
                        self.state
                            .language
                            .text("No pull request selected", "PR не выбран")
                            .to_owned()
                    })?;
                    match c.view {
                        View::MrDiff => format!("{}/pulls/{iid}/files?{page}", repo()?),
                        View::MrNotes => format!("{}/issues/{iid}/comments?{page}", repo()?),
                        _ => format!("{}/pulls/{iid}", repo()?),
                    }
                }
                View::Pipelines => format!(
                    "{}/actions/runs?{page}{}",
                    repo()?,
                    if c.query.is_empty() {
                        String::new()
                    } else {
                        format!("&branch={search}")
                    }
                ),
                View::Jobs => format!(
                    "{}/actions/runs/{}/jobs?{page}&filter=latest",
                    repo()?,
                    c.pipeline.as_ref().map(|v| number(v, "id")).unwrap_or(0)
                ),
                View::Trace => {
                    return Ok(Request::text(format!(
                        "{}/actions/jobs/{}/logs",
                        repo()?,
                        c.job.as_ref().map(|v| number(v, "id")).unwrap_or(0)
                    )));
                }
            };
        Ok(Request::get(endpoint))
    }

    pub fn connect(&mut self, provider: Provider, host: String) -> Vec<Effect> {
        self.cancel();
        self.history.clear();
        self.unknown_change = None;
        self.after_load = None;
        self.state.provider = provider;
        self.state.host = host;
        self.state.context = Context::default();
        self.state.rows.clear();
        self.state.object = Value::Null;
        self.state.text.clear();
        self.state.selected = 0;
        self.state.scroll = 0;
        self.state.scroll_x = 0;
        self.state.sidebar_selected = 0;
        self.state.popup = None;
        self.state.error = None;
        let mut effects = vec![Effect::HostChanged];
        effects.extend(self.refresh());
        effects
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            context: self.state.context.clone(),
            rows: self.state.rows.clone(),
            object: self.state.object.clone(),
            text: self.state.text.clone(),
            selected: self.state.selected,
            scroll: self.state.scroll,
            scroll_x: self.state.scroll_x,
            can_next: self.can_next,
        }
    }

    fn navigate(&mut self, context: Context) -> Vec<Effect> {
        self.unknown_change = None;
        self.after_load = None;
        if self.history.len() == 32 {
            self.history.remove(0);
        }
        self.history.push(self.snapshot());
        self.state.context = context;
        self.state.rows.clear();
        self.state.object = Value::Null;
        self.state.text.clear();
        self.state.selected = 0;
        self.state.scroll = 0;
        self.state.scroll_x = 0;
        self.state.focus = Focus::Main;
        self.state.popup = None;
        self.can_next = false;
        self.refresh()
    }

    fn back(&mut self) -> Vec<Effect> {
        self.cancel();
        self.state.error = None;
        self.state.popup = None;
        self.after_load = None;
        self.unknown_change = None;
        if let Some(previous) = self.history.pop() {
            self.state.context = previous.context;
            self.state.rows = previous.rows;
            self.state.object = previous.object;
            self.state.text = previous.text;
            self.state.selected = previous.selected;
            self.state.scroll = previous.scroll;
            self.state.scroll_x = previous.scroll_x;
            self.can_next = previous.can_next;
            self.localize_cached_text();
            self.state.status = self
                .state
                .language
                .text("Back · F5 to refresh", "Назад · F5 обновить данные")
                .into();
        }
        Vec::new()
    }

    fn localize_cached_text(&mut self) {
        let response = match self.state.context.view {
            View::MrDetails if self.state.object.is_object() => {
                Some(Response::Json(self.state.object.clone()))
            }
            View::MrDiff | View::MrNotes => {
                Some(Response::Json(Value::Array(self.state.rows.clone())))
            }
            _ => None,
        };
        if let Some(response) = response {
            let _ = self.apply(response);
        }
    }

    fn project_view(&mut self, view: View) -> Vec<Effect> {
        if self.state.context.view == View::Favorites
            && self
                .state
                .selected()
                .is_some_and(|row| row["_unavailable"] == true)
        {
            self.state.error = Some(
                self.state
                    .language
                    .text(
                        "Project unavailable. Press * to remove it from favorites",
                        "Проект недоступен. * — убрать его из избранного",
                    )
                    .into(),
            );
            return Vec::new();
        }
        if self.state.project_id().is_none() {
            if matches!(self.state.context.view, View::Projects | View::Favorites)
                && let Some(project) = self.state.selected().cloned()
            {
                let context = Context {
                    view,
                    project: Some(project),
                    ..Context::default()
                };
                return self.navigate(context);
            }
            self.state.error = Some(
                self.state
                    .language
                    .text("Select and open a project", "Выберите и откройте проект")
                    .into(),
            );
            return Vec::new();
        }
        let context = Context {
            view,
            project: self.state.context.project.clone(),
            ..Context::default()
        };
        self.navigate(context)
    }

    fn global(&mut self, index: usize) -> Vec<Effect> {
        self.state.sidebar_selected = index;
        if index == 4 {
            self.state.popup = Some(Popup::Input {
                title: self
                    .state
                    .language
                    .text("Server · hostname[:port]", "Сервер · hostname[:port]")
                    .to_owned(),
                value: self.state.host.clone(),
                purpose: InputPurpose::Host,
            });
            return Vec::new();
        }
        let view = match index {
            1 => View::Groups,
            2 => View::MergeRequests,
            3 => View::Favorites,
            _ => View::Projects,
        };
        self.navigate(Context {
            view,
            ..Context::default()
        })
    }

    fn enter(&mut self) -> Vec<Effect> {
        if self.state.focus == Focus::Sidebar {
            return self.global(self.state.sidebar_selected);
        }
        if self.state.loading {
            return Vec::new();
        }
        if self.state.context.view == View::Settings {
            return self.edit_setting();
        }
        let Some(row) = self.state.selected().cloned() else {
            return Vec::new();
        };
        if row["_unavailable"] == true && self.state.context.view == View::Repository {
            self.state.error = Some(
                self.state
                    .language
                    .text(
                        "Symlink or submodule: content preview is unavailable",
                        "Символическая ссылка или подмодуль: просмотр недоступен",
                    )
                    .into(),
            );
            return Vec::new();
        }
        if row["_unavailable"] == true {
            self.state.error = Some(
                self.state
                    .language
                    .text(
                        "Project unavailable. Press * to remove it from favorites",
                        "Проект недоступен. * — убрать его из избранного",
                    )
                    .into(),
            );
            return Vec::new();
        }
        let mut c = self.state.context.clone();
        c.page = 1;
        c.query.clear();
        match c.view {
            View::Projects | View::Favorites => {
                c.project = Some(row);
                c.group = None;
                c.view = View::Overview;
            }
            View::Groups => {
                c.group = Some(row);
                c.project = None;
                c.view = View::Projects;
            }
            View::Pipelines => {
                c.pipeline = Some(row);
                c.view = View::Jobs;
            }
            View::Jobs => {
                c.job = Some(row);
                c.view = View::Trace;
            }
            View::MergeRequests => {
                if c.project.is_none() && self.state.provider == Provider::Github {
                    c.project = row.get("_project").cloned();
                }
                if c.project.is_none() {
                    let id = number(&row, "project_id");
                    let name = row
                        .pointer("/references/full")
                        .and_then(Value::as_str)
                        .and_then(|r| r.split('!').next())
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            crate::tr!(self.state.language, "Project #{id}", "Проект #{id}")
                        });
                    c.project = Some(json!({"id":id,"name":name,"path_with_namespace":name}));
                }
                c.mr = Some(row);
                c.view = View::MrDetails;
            }
            View::Repository => {
                if string(&row, "type") == "tree" {
                    c.path = string(&row, "path").into();
                } else {
                    c.file = Some(row);
                    c.view = View::File;
                }
            }
            _ => return Vec::new(),
        }
        self.navigate(c)
    }

    fn move_selection(&mut self, delta: isize) {
        if self.state.focus == Focus::Sidebar {
            self.state.sidebar_selected = self
                .state
                .sidebar_selected
                .saturating_add_signed(delta)
                .min(4);
        } else if self.state.context.view.is_text()
            || self.state.context.view == View::Overview
            || self.state.focus == Focus::Details
        {
            self.state.scroll = self.state.scroll.saturating_add_signed(delta as i16);
        } else {
            let length = if self.state.context.view == View::Settings {
                self.state.settings().len()
            } else {
                self.state.rows.len()
            };
            self.state.selected = self
                .state
                .selected
                .saturating_add_signed(delta)
                .min(length.saturating_sub(1));
            self.state.scroll = 0;
        }
    }

    fn page(&mut self, next: bool) -> Vec<Effect> {
        if self.state.loading {
            return Vec::new();
        }
        if !matches!(
            self.state.context.view,
            View::Projects
                | View::Groups
                | View::MergeRequests
                | View::MrDiff
                | View::MrNotes
                | View::Pipelines
                | View::Jobs
                | View::Repository
                | View::Branches
        ) {
            return Vec::new();
        }
        if next && !self.can_next {
            self.state.status = self
                .state
                .language
                .text("Last page", "Последняя страница")
                .into();
            return Vec::new();
        }
        let page = self.state.context.page;
        self.state.context.page = if next {
            page.saturating_add(1)
        } else {
            page.saturating_sub(1).max(1)
        };
        if page == self.state.context.page {
            return Vec::new();
        }
        self.state.selected = 0;
        self.state.scroll = 0;
        self.refresh()
    }

    fn text_search(&mut self, next: bool, from_start: bool) {
        let query = self.state.context.query.to_lowercase();
        if query.is_empty() {
            return;
        }
        let positions: Vec<(usize, u16)> = self
            .state
            .text
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                let line = line.to_lowercase();
                line.find(&query).map(|column| {
                    (
                        i,
                        ratatui::text::Line::from(&line[..column])
                            .width()
                            .min(u16::MAX as usize) as u16,
                    )
                })
            })
            .collect();
        let current = self.state.scroll as usize;
        let position = if from_start {
            positions.first().copied()
        } else if next {
            positions
                .iter()
                .copied()
                .find(|(i, _)| *i > current)
                .or_else(|| positions.first().copied())
        } else {
            positions
                .iter()
                .copied()
                .rev()
                .find(|(i, _)| *i < current)
                .or_else(|| positions.last().copied())
        };
        if let Some((position, column)) = position {
            self.state.scroll = position.min(u16::MAX as usize) as u16;
            self.state.scroll_x = column.saturating_sub(8);
            self.state.status = crate::tr!(
                self.state.language,
                "Matches found: {}",
                "Найдено совпадений: {}",
                positions.len()
            );
        } else {
            self.state.status = self
                .state
                .language
                .text("No matches", "Совпадений нет")
                .into();
        }
    }

    fn edit_setting(&mut self) -> Vec<Effect> {
        if self.state.loading {
            return Vec::new();
        }
        let Some(setting) = self.state.settings().get(self.state.selected) else {
            return Vec::new();
        };
        let level = self
            .state
            .object
            .pointer("/permissions/project_access/access_level")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .max(
                self.state
                    .object
                    .pointer("/permissions/group_access/access_level")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
        if (self.state.provider == Provider::Gitlab && level < 40)
            || (self.state.provider == Provider::Github
                && self.state.object.pointer("/permissions/admin") != Some(&Value::Bool(true)))
        {
            self.state.error = Some(
                self.state
                    .language
                    .text(
                        if self.state.provider == Provider::Github {
                            "Editing requires repository admin permission"
                        } else {
                            "Editing requires the Maintainer or Owner role"
                        },
                        if self.state.provider == Provider::Github {
                            "Для редактирования требуются права администратора репозитория"
                        } else {
                            "Для редактирования требуется роль Maintainer или Owner"
                        },
                    )
                    .into(),
            );
            return Vec::new();
        }
        let Some(value) = self.state.object.get(setting.key).filter(|v| !v.is_null()) else {
            self.state.error = Some(
                self.state
                    .language
                    .text(
                        "The server did not provide this field",
                        "Сервер не предоставил это поле",
                    )
                    .into(),
            );
            return Vec::new();
        };
        self.state.popup = Some(Popup::Input {
            title: format!(
                "{} · {}",
                setting.label_in(self.state.language),
                setting.help_in(self.state.language)
            ),
            value: value_label(value),
            purpose: InputPurpose::Setting(self.state.selected),
        });
        Vec::new()
    }

    fn confirm_setting(&mut self, index: usize, input: String) -> Vec<Effect> {
        let setting = self.state.settings()[index];
        let value = match setting.parse_in(&input, self.state.language) {
            Ok(v) => v,
            Err(error) => {
                self.state.error = Some(error);
                return Vec::new();
            }
        };
        let Some(project) = self.state.project_id() else {
            return Vec::new();
        };
        let old = self
            .state
            .object
            .get(setting.key)
            .cloned()
            .unwrap_or(Value::Null);
        if old == value {
            self.state.popup = None;
            self.state.status = self
                .state
                .language
                .text("The value has not changed", "Значение не изменилось")
                .into();
            return Vec::new();
        }
        let body = crate::tr!(
            self.state.language,
            "Project: {}\n\n{}\n\nBefore: {}\nAfter: {}\n\nOnly this field will be changed.",
            "Проект: {}\n\n{}\n\nБыло: {}\nСтало: {}\n\nБудет изменено только это поле.",
            self.state
                .context
                .project
                .as_ref()
                .map(|p| string(p, "path_with_namespace"))
                .unwrap_or("—"),
            setting.label_in(self.state.language),
            value_label(&old),
            value_label(&value)
        );
        let mut patch = serde_json::Map::new();
        patch.insert(setting.key.into(), value);
        self.state.popup = Some(Popup::Confirm {
            title: self
                .state
                .language
                .text("Save setting?", "Сохранить настройку?")
                .into(),
            body,
            request: if self.state.provider == Provider::Github {
                match self.github_repo() {
                    Ok(endpoint) => Request::Json {
                        endpoint,
                        method: Method::Patch,
                        body: Some(Value::Object(patch)),
                    },
                    Err(error) => {
                        self.state.error = Some(error);
                        return Vec::new();
                    }
                }
            } else {
                Request::put(format!("projects/{project}"), Value::Object(patch))
            },
        });
        self.state.popup_scroll = 0;
        Vec::new()
    }

    fn ci_action(&mut self, action: &str) -> Vec<Effect> {
        if self.state.loading {
            return Vec::new();
        }
        let Some(project) = self.state.project_id() else {
            return Vec::new();
        };
        let view = self.state.context.view;
        let (entity, record) = match view {
            View::Pipelines => ("pipelines", self.state.selected()),
            View::Jobs => ("jobs", self.state.selected()),
            View::Trace => ("jobs", self.state.context.job.as_ref()),
            _ => return Vec::new(),
        };
        let Some(record) = record else {
            return Vec::new();
        };
        if self.state.provider == Provider::Github && action == "play" {
            self.state.error = Some(
                self.state
                    .language
                    .text(
                        "GitHub does not support manual job play here",
                        "GitHub не поддерживает ручной запуск джобы здесь",
                    )
                    .into(),
            );
            return Vec::new();
        }
        if action == "play" && (entity != "jobs" || string(record, "status") != "manual") {
            self.state.error = Some(
                self.state
                    .language
                    .text(
                        "Only manual jobs can be started",
                        "Запуск доступен только для ручной джобы",
                    )
                    .into(),
            );
            return Vec::new();
        }
        if self.state.provider == Provider::Github && action == "cancel" && entity == "jobs" {
            self.state.error=Some(self.state.language.text("GitHub supports canceling a workflow run, not individual jobs or manual job play", "GitHub поддерживает отмену запуска workflow, а не отдельной джобы или её ручной запуск").into());
            return Vec::new();
        }
        let id = number(record, "id");
        let label = match action {
            "retry" => self.state.language.text("Retry", "Повторить"),
            "cancel" => self.state.language.text("Cancel", "Отменить"),
            _ => self.state.language.text("Play", "Запустить"),
        };
        let name = string(record, "name").to_owned();
        self.state.error = None;
        self.state.popup = Some(Popup::Confirm {
            title: format!(
                "{label} {} #{id}?",
                if entity == "jobs" {
                    self.state.language.text("job", "джобу")
                } else {
                    if self.state.provider == Provider::Github {
                        self.state.language.text("workflow run", "запуск workflow")
                    } else {
                        self.state.language.text("pipeline", "пайплайн")
                    }
                }
            ),
            body: crate::tr!(
                self.state.language,
                "Project: {}\nResource: {name} #{id}\nAction: {label}\n\nThis action will be sent to {}.{}",
                "Проект: {}\nОбъект: {name} #{id}\nДействие: {label}\n\nДействие будет отправлено в {}.{}",
                self.state
                    .context
                    .project
                    .as_ref()
                    .map(|p| string(p, "path_with_namespace"))
                    .unwrap_or("—"),
                self.state.provider.name(),
                if self.state.provider == Provider::Github && entity == "jobs" && action == "retry"
                {
                    self.state.language.text(
                        " Dependent jobs will also be rerun.",
                        " Зависимые джобы также будут повторены.",
                    )
                } else {
                    ""
                }
            ),
            request: if self.state.provider == Provider::Github {
                let path = if entity == "jobs" { "jobs" } else { "runs" };
                let action = if action == "retry" { "rerun" } else { action };
                match self.github_repo() {
                    Ok(repo) => Request::post(format!("{repo}/actions/{path}/{id}/{action}")),
                    Err(error) => {
                        self.state.error = Some(error);
                        return Vec::new();
                    }
                }
            } else {
                Request::post(format!("projects/{project}/{entity}/{id}/{action}"))
            },
        });
        self.state.popup_scroll = 0;
        Vec::new()
    }

    fn popup_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let Some(popup) = self.state.popup.clone() else {
            return Vec::new();
        };
        match popup {
            Popup::Help => {
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.state.popup_scroll = self.state.popup_scroll.saturating_add(1)
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.state.popup_scroll = self.state.popup_scroll.saturating_sub(1)
                    }
                    KeyCode::PageDown => {
                        self.state.popup_scroll = self.state.popup_scroll.saturating_add(8)
                    }
                    KeyCode::PageUp => {
                        self.state.popup_scroll = self.state.popup_scroll.saturating_sub(8)
                    }
                    _ => self.state.popup = None,
                }
                Vec::new()
            }
            Popup::Confirm { request, .. } => match key.code {
                KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                    self.state.popup = None;
                    self.fetch(request)
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N' | 'q') => {
                    self.state.popup = None;
                    self.state.status = self
                        .state
                        .language
                        .text("Action canceled", "Действие отменено")
                        .into();
                    Vec::new()
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.state.popup_scroll = self.state.popup_scroll.saturating_add(1);
                    Vec::new()
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.state.popup_scroll = self.state.popup_scroll.saturating_sub(1);
                    Vec::new()
                }
                _ => Vec::new(),
            },
            Popup::Input {
                title,
                mut value,
                purpose,
            } => {
                match key.code {
                    KeyCode::Left | KeyCode::Right
                        if matches!(purpose, InputPurpose::Setting(_)) =>
                    {
                        if let InputPurpose::Setting(index) = purpose {
                            let choices = match self.state.settings()[index].kind {
                                SettingKind::Choice(v) => v,
                                SettingKind::Bool => &["false", "true"],
                                _ => &[],
                            };
                            if !choices.is_empty() {
                                let i = choices.iter().position(|v| *v == value).unwrap_or(0);
                                let next = if key.code == KeyCode::Right {
                                    (i + 1) % choices.len()
                                } else {
                                    (i + choices.len() - 1) % choices.len()
                                };
                                value = choices[next].into();
                            }
                        }
                    }
                    KeyCode::Esc => {
                        self.state.popup = None;
                        return Vec::new();
                    }
                    KeyCode::Backspace => {
                        value.pop();
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        value.clear()
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && !c.is_control() =>
                    {
                        if value.chars().count() < 4096 {
                            value.push(c);
                        }
                    }
                    KeyCode::Enter => {
                        self.state.error = None;
                        match purpose {
                            InputPurpose::Setting(index) => {
                                return self.confirm_setting(index, value);
                            }
                            InputPurpose::Search => {
                                self.state.popup = None;
                                self.state.context.query = value;
                                if self.state.context.view.is_text() {
                                    self.state.scroll = 0;
                                    self.state.scroll_x = 0;
                                    self.text_search(true, true);
                                    return Vec::new();
                                }
                                self.state.context.page = 1;
                                self.state.selected = 0;
                                return self.refresh();
                            }
                            InputPurpose::Host => {
                                if let Err(error) = crate::api::validate_host(&value) {
                                    self.state.error =
                                        Some(self.state.language.error_message(&error));
                                    return Vec::new();
                                }
                                return self.connect(self.state.provider, value);
                            }
                        }
                    }
                    _ => {}
                }
                self.state.popup = Some(Popup::Input {
                    title,
                    value,
                    purpose,
                });
                Vec::new()
            }
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return vec![Effect::Exit];
        }
        if self.state.popup.is_some() {
            return self.popup_key(key);
        }
        if self.mutation_pending()
            && !matches!(
                key.code,
                KeyCode::Char('q' | '?' | 'j' | 'k')
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::PageUp
                    | KeyCode::PageDown
            )
        {
            self.state.status = self
                .state
                .language
                .text(
                    "Waiting for the change result · exiting does not cancel the server operation",
                    "Ожидание результата изменения · выход не отменяет операцию на сервере",
                )
                .into();
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('q') => vec![Effect::Exit],
            KeyCode::Char('G') => vec![Effect::SwitchProvider],
            KeyCode::Char('L') => {
                self.state.language = self.state.language.toggle();
                self.localize_cached_text();
                self.after_load = self.unknown_notice();
                let mut effects = vec![Effect::SavePreferences];
                effects.extend(self.refresh());
                if let Some(warning) = self.unknown_notice() {
                    self.state.status = warning;
                }
                effects
            }
            KeyCode::Char('?') => {
                self.state.popup_scroll = 0;
                self.state.popup = Some(Popup::Help);
                Vec::new()
            }
            KeyCode::Tab => {
                self.state.focus = match self.state.focus {
                    Focus::Sidebar => Focus::Main,
                    Focus::Main => Focus::Details,
                    Focus::Details => Focus::Sidebar,
                };
                Vec::new()
            }
            KeyCode::BackTab => {
                self.state.focus = match self.state.focus {
                    Focus::Sidebar => Focus::Details,
                    Focus::Main => Focus::Sidebar,
                    Focus::Details => Focus::Main,
                };
                Vec::new()
            }
            KeyCode::Esc | KeyCode::Backspace => self.back(),
            KeyCode::Enter => self.enter(),
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_selection(1);
                Vec::new()
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_selection(-1);
                Vec::new()
            }
            KeyCode::PageDown => {
                self.move_selection(10);
                Vec::new()
            }
            KeyCode::PageUp => {
                self.move_selection(-10);
                Vec::new()
            }
            KeyCode::Home => {
                self.state.selected = 0;
                self.state.scroll = 0;
                self.state.scroll_x = 0;
                Vec::new()
            }
            KeyCode::End => {
                if self.state.context.view.is_text() {
                    self.state.scroll = self
                        .state
                        .text
                        .lines()
                        .count()
                        .saturating_sub(1)
                        .min(u16::MAX as usize) as u16;
                } else if self.state.context.view == View::Settings {
                    self.state.selected = self.state.settings().len() - 1;
                } else {
                    self.state.selected = self.state.rows.len().saturating_sub(1);
                }
                Vec::new()
            }
            KeyCode::Char('1') => self.global(0),
            KeyCode::Left | KeyCode::Right if self.state.context.view.is_text() => {
                self.state.scroll_x = self
                    .state
                    .scroll_x
                    .saturating_add_signed(if key.code == KeyCode::Right { 8 } else { -8 });
                Vec::new()
            }
            KeyCode::Char('2') => self.global(1),
            KeyCode::Char('3') => self.global(2),
            KeyCode::Char('H') => self.global(4),
            KeyCode::Char('o') => self.project_view(View::Overview),
            KeyCode::Char('p') => self.project_view(View::Pipelines),
            KeyCode::Char('m') => self.project_view(View::MergeRequests),
            KeyCode::Char('b') => self.project_view(View::Branches),
            KeyCode::Char('f') => self.project_view(View::Repository),
            KeyCode::Char('s') => self.project_view(View::Settings),
            KeyCode::Char('d') if self.state.context.mr.is_some() => {
                let mut c = self.state.context.clone();
                c.view = View::MrDiff;
                c.page = 1;
                c.query.clear();
                self.navigate(c)
            }
            KeyCode::Char('c') if self.state.context.mr.is_some() => {
                let mut c = self.state.context.clone();
                c.view = View::MrNotes;
                c.page = 1;
                c.query.clear();
                self.navigate(c)
            }
            KeyCode::Char('R') | KeyCode::F(5) => self.refresh(),
            KeyCode::Char('r') => self.ci_action("retry"),
            KeyCode::Char('x') => self.ci_action("cancel"),
            KeyCode::Char('S') => self.ci_action("play"),
            KeyCode::Char('e') if self.state.context.view == View::Settings => self.edit_setting(),
            KeyCode::Char('/') => {
                self.state.popup = Some(Popup::Input {
                    title: if self.state.context.view == View::Pipelines {
                        self.state.language.text(
                            "Filter by exact branch name",
                            "Фильтр по точному имени ветки",
                        )
                    } else {
                        self.state.language.text(
                            "Search · Enter to apply, Ctrl+U to clear",
                            "Поиск · Enter применить, Ctrl+U очистить",
                        )
                    }
                    .into(),
                    value: self.state.context.query.clone(),
                    purpose: InputPurpose::Search,
                });
                Vec::new()
            }
            KeyCode::Char('n') => {
                if self.state.context.view.is_text() {
                    self.text_search(true, false);
                    Vec::new()
                } else {
                    self.page(true)
                }
            }
            KeyCode::Char('N') => {
                if self.state.context.view.is_text() {
                    self.text_search(false, false);
                    Vec::new()
                } else {
                    self.page(false)
                }
            }
            KeyCode::Char(']') => self.page(true),
            KeyCode::Char('[') => self.page(false),
            KeyCode::Char('*') => {
                let project = if matches!(self.state.context.view, View::Projects | View::Favorites)
                {
                    self.state.selected()
                } else {
                    self.state.context.project.as_ref()
                };
                if let Some(id) = project.map(|p| number(p, "id")).filter(|id| *id > 0) {
                    if !self.state.favorites.remove(&id) {
                        self.state.favorites.insert(id);
                    }
                    self.state.status = self
                        .state
                        .language
                        .text("Favorites updated", "Избранное обновлено")
                        .into();
                    let mut effects = vec![Effect::SavePreferences];
                    if self.state.context.view == View::Favorites {
                        effects.extend(self.refresh());
                    }
                    effects
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    pub fn paste(&mut self, text: &str) {
        let settings = self.state.settings();
        if let Some(Popup::Input { value, purpose, .. }) = &mut self.state.popup {
            let description = matches!(purpose, InputPurpose::Setting(index) if settings[*index].key == "description");
            let room = 4096usize.saturating_sub(value.chars().count());
            value.extend(
                text.chars()
                    .filter(|c| !c.is_control() || (description && matches!(c, '\n' | '\t')))
                    .take(room),
            );
        }
    }

    pub fn accept(&mut self, id: u64, result: Result<Response, ApiError>) -> Vec<Effect> {
        if id != self.generation {
            return Vec::new();
        }
        let was_mutation = self.mutation_pending();
        self.active = None;
        self.state.loading = false;
        match result {
            Err(mut error) => {
                error.message = self.state.language.error_message(&error.message);
                if error.uncertain {
                    self.unknown_change = Some(error.message);
                    self.after_load = self.unknown_notice();
                    let effects = self.refresh();
                    self.state.status = self.unknown_notice().unwrap_or_default();
                    return effects;
                }
                if let Some(warning) = self.after_load.take() {
                    self.state.error = Some(crate::tr!(
                        self.state.language,
                        "{warning}\nReload failed: {}",
                        "{warning}\nПовторное чтение не удалось: {}",
                        error.message
                    ));
                } else {
                    self.state.error = Some(error.message);
                }
                Vec::new()
            }
            Ok(response) if was_mutation => {
                if let Response::Json(value) = response {
                    if self.state.context.view == View::Settings {
                        self.state.context.project = Some(value.clone());
                        self.state.object = value;
                    } else if self.state.context.view == View::Trace && value.is_object() {
                        self.state.context.job = Some(value);
                    }
                }
                self.after_load = Some(
                    self.state
                        .language
                        .text(
                            "Change applied. Data refreshed.",
                            "Изменение применено. Данные обновлены.",
                        )
                        .into(),
                );
                self.refresh()
            }
            Ok(response) => {
                if let Err(error) = self.apply(response) {
                    self.state.error = Some(match self.after_load.take() {
                        Some(warning) => format!("{warning}\n{error}"),
                        None => error,
                    });
                } else {
                    self.state.error = None;
                    self.state.status = self.after_load.take().unwrap_or_else(|| {
                        if self.state.context.view.is_text() {
                            self.state
                                .language
                                .text(
                                    "Data loaded · / search · R refresh",
                                    "Данные загружены · / поиск · R обновить",
                                )
                                .into()
                        } else {
                            crate::tr!(
                                self.state.language,
                                "Page {} · {} records · [ / ] to change page",
                                "Страница {} · {} записей · [ / ] перелистнуть",
                                self.state.context.page,
                                self.state.rows.len()
                            )
                        }
                    });
                }
                Vec::new()
            }
        }
    }

    fn apply(&mut self, response: Response) -> Result<(), String> {
        let view = self.state.context.view;
        match response {
            Response::Text { body, truncated } => {
                self.state.text = body;
                if truncated {
                    self.notice(self.state.language.text(
                        "Text truncated: showing the first 2 MiB. R to reload.",
                        "Текст сокращён: показаны первые 2 МиБ. R перечитать.",
                    ));
                }
            }
            Response::Json(value) => {
                if matches!(view, View::Overview | View::Settings | View::MrDetails) {
                    if !value.is_object() {
                        return Err(self
                            .state
                            .language
                            .text(
                                "The server returned an invalid object format",
                                "Сервер вернул неверный формат объекта",
                            )
                            .into());
                    }
                    self.state.object = value.clone();
                    if view == View::MrDetails {
                        self.state.context.mr = Some(value.clone());
                        self.state.text = crate::tr!(
                            self.state.language,
                            "{}{}  {}\n\n{} → {}\nStatus: {}\nAuthor: {}\n\n{}\n\n{}",
                            "{}{}  {}\n\n{} → {}\nСтатус: {}\nАвтор: {}\n\n{}\n\n{}",
                            self.state.provider.mr_prefix(),
                            number(&value, "iid"),
                            string(&value, "title"),
                            string(&value, "source_branch"),
                            string(&value, "target_branch"),
                            string(&value, "state"),
                            value
                                .pointer("/author/name")
                                .and_then(Value::as_str)
                                .unwrap_or("—"),
                            string(&value, "description"),
                            string(&value, "web_url")
                        );
                    } else {
                        self.state.context.project = Some(value);
                    }
                } else {
                    let Some(items) = value.as_array() else {
                        return Err(self
                            .state
                            .language
                            .text(
                                "The server returned an invalid list format",
                                "Сервер вернул неверный формат списка",
                            )
                            .into());
                    };
                    self.can_next = items.len() == PAGE_SIZE;
                    if self.state.provider == Provider::Github
                        && self.state.context.page >= 20
                        && (matches!(view, View::Repository)
                            || matches!(view, View::MergeRequests)
                                && (self.state.context.project.is_none()
                                    || !self.state.context.query.is_empty())
                            || view == View::Projects && !self.state.context.query.is_empty())
                    {
                        self.can_next = false;
                        self.notice(self.state.language.text(
                            "GitHub limits this list to 1,000 results; narrow the search",
                            "GitHub ограничивает этот список 1000 результатами; уточните поиск",
                        ));
                    }
                    self.state.rows = items.clone();
                    if (matches!(view, View::Jobs | View::Repository | View::Favorites)
                        || self.state.provider == Provider::Github
                            && matches!(view, View::Branches | View::Groups))
                        && !self.state.context.query.is_empty()
                    {
                        let query = self.state.context.query.to_lowercase();
                        self.state.rows.retain(|row| {
                            string(row, "name").to_lowercase().contains(&query)
                                || string(row, "status").to_lowercase().contains(&query)
                        });
                        self.notice(self.state.language.text(
                            "Search applies to the current page; [ / ] change page",
                            "Поиск применён к текущей странице; [ / ] меняют страницу",
                        ));
                    }
                    if view == View::MrDiff {
                        self.state.text = items
                            .iter()
                            .map(|file| {
                                format!(
                                    "diff --git a/{} b/{}\n{}{}",
                                    string(file, "old_path"),
                                    string(file, "new_path"),
                                    if file.get("too_large").and_then(Value::as_bool) == Some(true)
                                        || file.get("collapsed").and_then(Value::as_bool)
                                            == Some(true)
                                    {
                                        self.state.language.text(
                                            if self.state.provider==Provider::Github {"[GitHub did not include a text patch for this file]\n"}else{"[GitLab limited the diff for this file]\n"},
                                            if self.state.provider==Provider::Github {"[GitHub не вернул текстовый дифф этого файла]\n"}else{"[GitLab ограничил дифф этого файла]\n"},
                                        )
                                    } else {
                                        ""
                                    },
                                    string(file, "diff")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n");
                    }
                    if view == View::MrNotes {
                        self.state.text = items
                            .iter()
                            .map(|note| {
                                format!(
                                    "{} · {}\n{}",
                                    note.pointer("/author/name")
                                        .and_then(Value::as_str)
                                        .unwrap_or("—"),
                                    string(note, "created_at"),
                                    string(note, "body")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n────────────────────────────────\n\n");
                    }
                    if self.state.rows.is_empty() && view.is_text() {
                        self.state.text = self
                            .state
                            .language
                            .text("No data on this page", "На этой странице данных нет")
                            .into();
                    }
                    self.state.selected = self
                        .state
                        .selected
                        .min(self.state.rows.len().saturating_sub(1));
                }
            }
        }
        Ok(())
    }

    fn notice(&mut self, message: &str) {
        self.after_load = Some(match self.after_load.take() {
            Some(existing) => format!("{existing} · {message}"),
            None => message.into(),
        });
    }

    fn unknown_notice(&self) -> Option<String> {
        self.unknown_change.as_ref().map(|message| {
            crate::tr!(
                self.state.language,
                "Change outcome unknown. Verify the result in {}. {}",
                "Результат изменения неизвестен. Сверьте состояние в {}. {}",
                self.state.provider.name(),
                self.state.language.error_message(message)
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }
    fn enter() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }
    fn finish(app: &mut App, value: Value) {
        let id = app.generation;
        app.accept(id, Ok(Response::Json(value)));
    }
    #[test]
    fn project_navigation_history_and_stale_response() {
        let mut app = App::new("example.org".into(), true);
        app.state.language = Language::Ru;
        app.refresh();
        finish(
            &mut app,
            json!([{"id":7,"name":"api","path_with_namespace":"core/api"}]),
        );
        app.key(enter());
        let stale = app.generation;
        assert_eq!(app.state.context.view, View::Overview);
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        app.accept(stale, Ok(Response::Json(json!({"id":999}))));
        assert_eq!(app.state.context.view, View::Projects);
        assert_eq!(app.state.rows[0]["id"], 7);
    }
    #[test]
    fn settings_change_requires_confirmation_and_sends_one_field() {
        let mut app = App::new("example.org".into(), true);
        app.state.language = Language::Ru;
        app.state.context.view = View::Settings;
        app.state.context.project = Some(json!({"id":7,"path_with_namespace":"core/api"}));
        app.state.object =
            json!({"description":"старое","permissions":{"project_access":{"access_level":40}}});
        app.state.selected = 1;
        assert!(app.key(key('e')).is_empty());
        assert!(matches!(app.state.popup, Some(Popup::Input { .. })));
        app.confirm_setting(1, "новое".into());
        let Some(Popup::Confirm {
            request: Request::Json {
                body: Some(body), ..
            },
            ..
        }) = &app.state.popup
        else {
            panic!("expected confirmation")
        };
        assert_eq!(body, &json!({"description":"новое"}));
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.state.object["description"], "старое");
        app.confirm_setting(1, "новое".into());
        assert!(matches!(
            app.key(key('y')).as_slice(),
            [Effect::Fetch {
                request: Request::Json {
                    method: Method::Put,
                    ..
                },
                ..
            }]
        ));
    }
    #[test]
    fn unknown_mutation_is_reconciled_by_get_not_repeated() {
        let mut app = App::new("example.org".into(), true);
        app.state.language = Language::Ru;
        app.state.context.view = View::Settings;
        app.state.context.project = Some(json!({"id":7}));
        app.fetch(Request::put("projects/7", json!({"description":"new"})));
        let effects = app.accept(
            app.generation,
            Err(ApiError {
                message: "Тайм-аут".into(),
                uncertain: true,
                http_status: None,
            }),
        );
        assert!(matches!(
            effects.as_slice(),
            [Effect::Fetch {
                request: Request::Json {
                    method: Method::Get,
                    ..
                },
                ..
            }]
        ));
        finish(&mut app, json!({"id":7,"description":"new"}));
        assert!(app.state.status.contains("неизвестен"));
    }
    #[test]
    fn unicode_input_empty_lists_and_low_role_are_safe() {
        let mut app = App::new("example.org".into(), true);
        app.state.language = Language::Ru;
        app.key(enter());
        app.move_selection(100);
        assert_eq!(app.state.selected, 0);
        app.state.popup = Some(Popup::Input {
            title: "Поиск".into(),
            value: "Омск".into(),
            purpose: InputPurpose::Search,
        });
        app.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(matches!(&app.state.popup,Some(Popup::Input{value,..}) if value=="Омс"));
        app.state.popup = None;
        app.state.context.view = View::Settings;
        app.key(key('e'));
        assert!(app.state.popup.is_none());
        assert!(app.state.error.as_deref().unwrap().contains("Maintainer"));
    }
    #[test]
    fn query_is_encoded_and_pagination_is_explicit() {
        let mut app = App::new("example.org".into(), true);
        app.state.language = Language::Ru;
        app.state.context.query = "a&owned=true".into();
        let Request::Json { endpoint, .. } = app.request().unwrap() else {
            panic!()
        };
        assert!(endpoint.contains("search=a%26owned%3Dtrue"));
        assert!(app.page(true).is_empty());
        app.can_next = true;
        app.page(true);
        assert_eq!(app.state.context.page, 2);
    }

    #[test]
    fn long_line_search_end_and_history_use_logical_rows() {
        let mut app = App::new("example.org".into(), true);
        app.state.language = Language::Ru;
        app.state.context.view = View::File;
        app.state.text = format!("{}\n{}НАЙТИ\nпоследняя", "a".repeat(500), "界".repeat(60));
        app.state.context.query = "найти".into();
        app.text_search(true, false);
        assert_eq!(app.state.scroll, 1);
        assert_eq!(app.state.scroll_x, 112);
        app.key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(app.state.scroll, 2);
        let previous = app.state.scroll_x;
        app.key(key('1'));
        app.back();
        assert_eq!(app.state.scroll, 2);
        assert_eq!(app.state.scroll_x, previous);
        app.state.popup = Some(Popup::Input {
            title: "Поиск".into(),
            value: String::new(),
            purpose: InputPurpose::Search,
        });
        app.key(enter());
        assert_eq!(app.state.scroll, 0);
        assert_eq!(app.state.scroll_x, 0);
        app.state.popup = Some(Popup::Input {
            title: "Поиск".into(),
            value: "нет совпадений".into(),
            purpose: InputPurpose::Search,
        });
        app.key(enter());
        assert_eq!(app.state.scroll, 0);
    }

    #[test]
    fn unknown_result_notice_survives_filtered_or_truncated_refresh() {
        for view in [View::Jobs, View::Trace] {
            let mut app = App::new("example.org".into(), true);
            app.state.language = Language::Ru;
            app.state.context.view = view;
            app.state.context.project = Some(json!({"id":7}));
            app.state.context.pipeline = Some(json!({"id":8}));
            app.state.context.job = Some(json!({"id":9}));
            app.state.context.query = "test".into();
            app.fetch(Request::post("projects/7/jobs/9/retry"));
            assert!(app.key(key('H')).is_empty());
            assert!(app.key(key('R')).is_empty());
            assert!(app.mutation_pending());
            app.accept(
                app.generation,
                Err(ApiError {
                    message: "Тайм-аут".into(),
                    uncertain: true,
                    http_status: None,
                }),
            );
            let response = if view == View::Trace {
                Response::Text {
                    body: "test".into(),
                    truncated: true,
                }
            } else {
                Response::Json(json!([{"id":9,"name":"test"}]))
            };
            app.accept(app.generation, Ok(response));
            assert!(app.state.status.contains("Результат изменения неизвестен"));
            assert!(app.state.status.contains(if view == View::Trace {
                "сокращён"
            } else {
                "текущей странице"
            }));
        }
        let mut app = App::new("example.org".into(), true);
        app.state.language = Language::Ru;
        app.state.context.view = View::Favorites;
        app.state.rows = vec![json!({"id":7,"_unavailable":true})];
        app.state.favorites.insert(7);
        assert!(app.key(enter()).is_empty());
        assert_eq!(app.state.context.view, View::Favorites);
        app.key(key('*'));
        assert!(app.state.favorites.is_empty());
    }

    #[test]
    fn language_switch_reloads_owned_messages_and_keeps_unknown_outcome() {
        let mut app = App::new("example.org".into(), true);
        app.state.language = Language::Ru;
        app.state.context.view = View::Settings;
        app.state.context.project = Some(json!({"id":7}));
        app.state.error = Some("Для редактирования требуется роль Maintainer или Owner".into());
        assert!(
            app.key(key('L'))
                .iter()
                .any(|effect| matches!(effect, Effect::Fetch { .. }))
        );
        assert!(app.state.error.is_none());
        app.accept(
            app.generation,
            Err(ApiError {
                message: "GitLab отклонил запрос: недостаточно прав (403)".into(),
                uncertain: false,
                http_status: Some(403),
            }),
        );
        assert!(
            app.state
                .error
                .as_deref()
                .unwrap()
                .contains("insufficient permissions")
        );
        app.refresh();
        finish(&mut app, json!({"id":7}));
        assert!(app.state.status.starts_with("Page"));

        app.state.language = Language::Ru;
        app.fetch(Request::put("projects/7", json!({"description":"Новое"})));
        app.accept(
            app.generation,
            Ok(Response::Json(json!({"id":7,"description":"Новое"}))),
        );
        app.accept(
            app.generation,
            Err(ApiError {
                message: "Не удалось запустить glab".into(),
                uncertain: false,
                http_status: None,
            }),
        );
        assert!(
            app.state
                .error
                .as_deref()
                .unwrap()
                .contains("Повторное чтение")
        );
        app.key(key('L'));
        assert!(app.state.error.is_none());
        app.state.language = Language::Ru;
        app.fetch(Request::put("projects/7", json!({"description":"Новое"})));
        app.accept(
            app.generation,
            Ok(Response::Json(json!({"id":7,"description":"Новое"}))),
        );
        app.key(key('L'));
        finish(&mut app, json!({"id":7,"description":"Новое"}));
        assert!(app.state.status.starts_with("Page"));
        app.state.language = Language::Ru;
        app.fetch(Request::put("projects/7", json!({"description":"Проекты"})));
        app.accept(
            app.generation,
            Err(ApiError {
                message: "GitLab не ответил за отведённое время".into(),
                uncertain: true,
                http_status: None,
            }),
        );
        app.key(key('L'));
        assert!(app.state.status.contains("Change outcome unknown"));
        app.accept(
            app.generation,
            Err(ApiError {
                message: "Could not start glab".into(),
                uncertain: false,
                http_status: None,
            }),
        );
        let error = app.state.error.as_deref().unwrap();
        assert!(error.contains("Change outcome unknown") && error.contains("Reload failed"));
        assert!(!error.chars().any(|c| ('А'..='я').contains(&c)));

        let mut mr = App::new("example.org".into(), true);
        mr.state.context.project = Some(json!({"id":7}));
        mr.state.context.view = View::MrDetails;
        let source = json!({"iid":3,"title":"Описание / Description","description":"Проекты / Projects","state":"opened"});
        mr.apply(Response::Json(source.clone())).unwrap();
        assert!(mr.state.text.contains("Status:"));
        mr.key(key('f'));
        mr.key(key('L'));
        mr.back();
        assert!(mr.state.text.contains("Статус:"));
        assert!(mr.state.text.contains("Проекты / Projects"));
        assert_eq!(mr.state.object, source);
        mr.key(key('L'));
        assert!(mr.state.text.contains("Status:"));
        assert_eq!(mr.state.object, source);
    }
}
