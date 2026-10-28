use crate::{
    i18n::Language,
    model::{ApiError, Method, Request, Response, encode},
};
use serde_json::{Value, json};

pub(crate) fn route(endpoint: &str) -> (&'static str, Option<&'static str>) {
    let path = endpoint.split('?').next().unwrap_or(endpoint);
    let parts: Vec<_> = path.split('/').collect();
    match parts.as_slice() {
        ["search", "issues"] => ("pull", Some("items")),
        ["search", "repositories"] => ("repo", Some("items")),
        ["user", "orgs"] => ("org", None),
        ["repos", _, _, rest @ ..] => match rest {
            ["contents", ..] => ("file", None),
            ["branches", ..] => ("branch", None),
            ["pulls"] => ("pull", None),
            ["pulls", _] => ("pull_detail", None),
            ["pulls", _, "files"] => ("diff", None),
            ["issues", _, "comments"] | ["pulls", _, "comments"] => ("note", None),
            ["actions", "runs"] => ("run", Some("workflow_runs")),
            ["actions", "runs", _, "jobs"] => ("job", Some("jobs")),
            ["actions", "runs", _, ..] => ("run", None),
            ["actions", "jobs", _, ..] => ("job", None),
            ["git", "blobs", _] => ("blob", None),
            _ => ("repo", None),
        },
        _ => ("repo", None),
    }
}

// Сохраняем нативные поля: изменяющие запросы используют ключи провайдера.
pub(crate) fn normalize(endpoint: &str, mut value: Value) -> Value {
    let path = endpoint.split('?').next().unwrap_or(endpoint);
    let (kind, envelope) = route(endpoint);
    if let Some(key) = envelope {
        value = value.get_mut(key).map(Value::take).unwrap_or(Value::Null);
    }
    if kind == "file"
        && let Value::Array(rows) = value
    {
        value = Value::Array(page(
            rows,
            endpoint.split_once('?').map(|(_, q)| q).unwrap_or(""),
        ));
    }
    if let Value::Array(rows) = &mut value {
        for row in rows {
            normalize_row(kind, path, row);
        }
    } else {
        normalize_row(kind, path, &mut value);
    }
    value
}

fn copy(value: &mut Value, to: &str, from: &str) {
    if let Some(field) = value.get(from).cloned() {
        value[to] = field;
    }
}
fn normalize_row(kind: &str, path: &str, value: &mut Value) {
    if !value.is_object() {
        return;
    }
    copy(value, "web_url", "html_url");
    match kind {
        "repo" => {
            if value.get("full_name").is_none() {
                return;
            }
            copy(value, "path_with_namespace", "full_name");
            copy(value, "last_activity_at", "updated_at");
            copy(value, "star_count", "stargazers_count");
            copy(value, "http_url_to_repo", "clone_url");
            copy(value, "ssh_url_to_repo", "ssh_url");
            if value.get("visibility").is_none() {
                value["visibility"] = if value["private"] == true {
                    "private"
                } else {
                    "public"
                }
                .into();
            }
        }
        "org" => {
            copy(value, "full_path", "login");
            copy(value, "name", "login");
        }
        "file" => {
            copy(value, "id", "sha");
            let directory = value["type"] == "dir";
            if !matches!(value["type"].as_str(), Some("dir" | "file"))
                || value
                    .get("submodule_git_url")
                    .is_some_and(|url| !url.is_null())
            {
                value["_unavailable"] = true.into();
                value["mode"] = "unsupported".into();
                return;
            }
            value["type"] = if directory { "tree" } else { "blob" }.into();
            value["mode"] = if directory { "040000" } else { "100644" }.into();
        }
        "branch" => {
            if let Some(sha) = value.pointer("/commit/sha").cloned() {
                value["commit"]["id"] = sha.clone();
                value["commit"]["short_id"] = sha
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(7)
                    .collect::<String>()
                    .into();
            }
        }
        "pull" | "pull_detail" => {
            copy(value, "iid", "number");
            copy(value, "description", "body");
            value["author"] = json!({"name":value.pointer("/user/login").cloned().unwrap_or(Value::Null),"username":value.pointer("/user/login").cloned().unwrap_or(Value::Null)});
            if let Some(branch) = value.pointer("/head/ref").cloned() {
                value["source_branch"] = branch;
            }
            if let Some(branch) = value.pointer("/base/ref").cloned() {
                value["target_branch"] = branch;
            }
            if value["state"] == "open" {
                value["state"] = "opened".into();
            }
            let project = value
                .pointer("/base/repo")
                .filter(|repo| {
                    repo["id"].as_u64().is_some_and(|id| id > 0)
                        && repo["full_name"].as_str().is_some()
                })
                .cloned();
            let full_name = project
                .as_ref()
                .and_then(|repo| repo["full_name"].as_str())
                .map(str::to_owned)
                .or_else(|| {
                    value
                        .get("repository_url")
                        .and_then(Value::as_str)
                        .and_then(|url| url.split_once("/repos/").map(|(_, repo)| repo.to_owned()))
                })
                .or_else(|| {
                    let parts: Vec<_> = path.split('/').collect();
                    match parts.as_slice() {
                        ["repos", owner, repo, "pulls", ..] => Some(format!("{owner}/{repo}")),
                        _ => None,
                    }
                });
            if let Some(repo) = full_name {
                value["references"] = json!({"full":format!("{}!{}", repo, value["number"])});
                let mut project = project.unwrap_or_else(||json!({"id":0,"name":repo.rsplit('/').next().unwrap_or(&repo),"full_name":repo}));
                normalize_row("repo", path, &mut project);
                value["_project"] = project;
            }
            if let Some(labels) = value.get_mut("labels").and_then(Value::as_array_mut) {
                for label in labels {
                    if let Some(name) = label.get("name").cloned() {
                        *label = name;
                    }
                }
            }
        }
        "diff" => {
            copy(value, "new_path", "filename");
            value["old_path"] = value
                .get("previous_filename")
                .or_else(|| value.get("filename"))
                .cloned()
                .unwrap_or(Value::Null);
            if value.get("patch").and_then(Value::as_str).is_none() {
                value["collapsed"] = true.into();
            }
            value["diff"] = value.get("patch").cloned().unwrap_or_else(|| "".into());
        }
        "note" => {
            value["author"] = json!({"name":value.pointer("/user/login").cloned().unwrap_or(Value::Null),"username":value.pointer("/user/login").cloned().unwrap_or(Value::Null)});
        }
        "run" | "job" => {
            let status = value
                .get("conclusion")
                .and_then(Value::as_str)
                .or_else(|| value.get("status").and_then(Value::as_str))
                .unwrap_or("unknown");
            value["status"] = match status {
                "failure" | "timed_out" | "startup_failure" => "failed",
                "in_progress" => "running",
                "cancelled" => "canceled",
                "waiting" | "pending" | "requested" => "pending",
                other => other,
            }
            .to_owned()
            .into();
            copy(value, "ref", "head_branch");
            copy(value, "sha", "head_sha");
            copy(value, "source", "event");
            if kind == "job" {
                value["stage"] = "Actions".into();
            }
        }
        _ => (),
    }
}

pub(crate) struct GitHubDemo {
    language: Language,
    repositories: Vec<Value>,
    runs: Vec<Value>,
    jobs: Vec<Value>,
}
impl GitHubDemo {
    pub(crate) fn new(language: Language) -> Self {
        let repositories = [(101,"api-gateway"),(102,"web-console"),(103,"design-system")].into_iter().map(|(id,name)|json!({
            "id":id,"name":name,"full_name":format!("demo/{name}"),"private":false,"visibility":"public",
            "description":language.text("A terminal-first developer workspace.","Рабочее пространство разработчика в терминале."),
            "default_branch":"main","archived":false,"html_url":format!("https://github.com/demo/{name}"),
            "clone_url":format!("https://github.com/demo/{name}.git"),"ssh_url":format!("git@github.com:demo/{name}.git"),
            "updated_at":"2026-10-28T02:30:00Z","created_at":"2026-09-01T09:00:00Z","topics":["rust","terminal"],"stargazers_count":42,"forks_count":3,
            "open_issues_count":7,"permissions":{"admin":true,"push":true,"pull":true},
            "delete_branch_on_merge":true,"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":true,"has_issues":true,"has_wiki":false
        })).collect();
        let runs = vec![
            json!({"id":1842,"name":"CI","display_title":"Improve terminal rendering","head_branch":"main","head_sha":"7f3a91cb812db72aaa45b65f4c213930b25ea321","event":"push","status":"completed","conclusion":"success","created_at":"2026-10-28T02:30:00Z","html_url":"https://github.com/demo/api-gateway/actions/runs/1842"}),
            json!({"id":1843,"name":"Release","head_branch":"main","head_sha":"b4829de1312db72aaa45b65f4c213930b25ea321","event":"workflow_dispatch","status":"in_progress","conclusion":null,"html_url":"https://github.com/demo/api-gateway/actions/runs/1843"}),
        ];
        let jobs = vec![
            json!({"id":9270,"name":"format","status":"completed","conclusion":"success","run_id":1842,"started_at":"2026-10-28T02:30:00Z","completed_at":"2026-10-28T02:30:04Z"}),
            json!({"id":9271,"name":"unit-tests","status":"completed","conclusion":"success","run_id":1842}),
            json!({"id":9272,"name":"release","status":"in_progress","conclusion":null,"run_id":1843}),
        ];
        let per_repository = |records: Vec<Value>, is_job: bool| {
            ["api-gateway", "web-console", "design-system"]
                .into_iter()
                .enumerate()
                .flat_map(|(index, name)| {
                    records.iter().cloned().map(move |mut record| {
                        let offset = index as u64 * 10_000;
                        record["id"] = (record["id"].as_u64().unwrap() + offset).into();
                        if is_job {
                            record["run_id"] = (record["run_id"].as_u64().unwrap() + offset).into();
                        }
                        if record.get("html_url").is_some() {
                            record["html_url"] = format!(
                                "https://github.com/demo/{name}/actions/runs/{}",
                                record["id"]
                            )
                            .into();
                        }
                        record["_demo_repository"] = name.into();
                        record
                    })
                })
                .collect::<Vec<_>>()
        };
        Self {
            language,
            repositories,
            runs: per_repository(runs, false),
            jobs: per_repository(jobs, true),
        }
    }
    pub(crate) fn set_language(&mut self, _language: Language) {
        // Язык демонстрационных данных задаётся при запуске, как у содержимого репозитория.
    }
    pub(crate) fn execute(&mut self, request: &Request) -> Result<Response, ApiError> {
        if let Request::Projects(ids) = request {
            let values = ids
                .iter()
                .map(|id| {
                    self.repositories
                        .iter()
                        .find(|repo| repo["id"].as_u64() == Some(*id))
                        .cloned()
                        .unwrap_or_else(
                            || json!({"id":id,"_unavailable":true,"name":format!("#{id}")}),
                        )
                })
                .map(|v| normalize("repositories/id", v))
                .collect();
            return Ok(Response::Json(Value::Array(values)));
        }
        let (endpoint, method, body, text) = match request {
            Request::Json {
                endpoint,
                method,
                body,
            } => (endpoint.as_str(), *method, body.as_ref(), false),
            Request::Text { endpoint } => (endpoint.as_str(), Method::Get, None, true),
            Request::Projects(_) => unreachable!(),
        };
        let (path, query) = endpoint.split_once('?').unwrap_or((endpoint, ""));
        let parts: Vec<_> = path.split('/').collect();
        if method == Method::Get {
            let response = match parts.as_slice() {
                ["user"] => json!({"login":"demo"}),
                ["user", "repos"] => Value::Array(page(self.repositories.clone(), query)),
                ["user", "orgs"] => Value::Array(page(
                    vec![
                        json!({"id":1,"login":"demo","description":self.language.text("Demo organization","Демонстрационная организация"),"html_url":"https://github.com/demo"}),
                    ],
                    query,
                )),
                ["orgs", "demo", "repos"] => Value::Array(page(self.repositories.clone(), query)),
                ["search", "repositories"] => {
                    json!({"items":page(self.repositories.clone(),query)})
                }
                ["search", "issues"] => {
                    let repository = self
                        .repositories
                        .iter()
                        .find(|repo| {
                            query.contains(&encode(&format!(
                                "repo:{}",
                                repo["full_name"].as_str().unwrap()
                            )))
                        })
                        .and_then(|repo| repo["name"].as_str())
                        .unwrap_or("api-gateway");
                    json!({"items":page(vec![self.pull(repository)],query)})
                }
                ["repositories", id] => self
                    .repositories
                    .iter()
                    .find(|repo| repo["id"].as_u64() == id.parse().ok())
                    .cloned()
                    .ok_or_else(not_found)?,
                ["repos", "demo", name, rest @ ..] => {
                    let repo = self
                        .repositories
                        .iter()
                        .find(|repo| repo["name"] == *name)
                        .cloned()
                        .ok_or_else(not_found)?;
                    match rest {
                        [] => repo,
                        ["contents"] | ["contents", ""] => Value::Array(page(
                            vec![
                                json!({"name":"README.md","path":"README.md","sha":"1111111111111111111111111111111111111111","type":"file"}),
                                json!({"name":"src","path":"src","sha":"2222222222222222222222222222222222222222","type":"dir"}),
                            ],
                            "",
                        )),
                        ["contents", "src"] => Value::Array(page(
                            vec![
                                json!({"name":"main.rs","path":"src/main.rs","sha":"3333333333333333333333333333333333333333","type":"file"}),
                            ],
                            "",
                        )),
                        ["git", "blobs", sha] if text => {
                            return Ok(Response::Text {
                                body: if *sha == "3333333333333333333333333333333333333333" {
                                    "fn main() {\n    println!(\"Hello, Forge Deck!\");\n}".into()
                                } else {
                                    self.language.text("# Demo repository\n\nBrowse code, pull requests and GitHub Actions in your terminal.\n","# Демонстрационный репозиторий\n\nКод, pull requests и GitHub Actions в терминале.\n").into()
                                },
                                truncated: false,
                            });
                        }
                        ["branches"] => Value::Array(page(
                            vec![
                                json!({"name":"main","protected":true,"commit":{"sha":"7f3a91cb812db72aaa45b65f4c213930b25ea321"}}),
                                json!({"name":"feat/retry-budget","protected":false,"commit":{"sha":"b4829de1312db72aaa45b65f4c213930b25ea321"}}),
                            ],
                            query,
                        )),
                        ["pulls"] => Value::Array(page(vec![self.pull(name)], query)),
                        ["pulls", "42"] => self.pull(name),
                        ["pulls", "42", "files"] => Value::Array(page(
                            vec![
                                json!({"filename":"src/main.rs","patch":"@@ -1,1 +1,2 @@\n fn main() {\n+    println!(\"Forge Deck\");","status":"modified"}),
                            ],
                            query,
                        )),
                        ["issues", "42", "comments"] => Value::Array(page(
                            vec![
                                json!({"id":1,"user":{"login":"alex"},"created_at":"2026-10-28T02:30:00Z","body":self.language.text("The checks passed. Ready for review.","Проверки прошли. Готово к ревью.")}),
                            ],
                            query,
                        )),
                        ["pulls", "42", "comments"] => Value::Array(Vec::new()),
                        ["actions", "runs"] => {
                            json!({"workflow_runs":page(self.runs.iter().filter(|r|r["_demo_repository"]==*name).cloned().collect(),query)})
                        }
                        ["actions", "runs", run, "jobs"] => {
                            json!({"jobs":page(self.jobs.iter().filter(|job|job["_demo_repository"]==*name && job["run_id"].as_u64()==run.parse().ok()).cloned().collect(),query)})
                        }
                        ["actions", "jobs", job, "logs"] if text => {
                            if !self.jobs.iter().any(|j| {
                                j["_demo_repository"] == *name
                                    && j["id"].as_u64() == job.parse().ok()
                            }) {
                                return Err(not_found());
                            }
                            return Ok(Response::Text{body:"2026-10-28T02:30:00Z Run cargo test --locked\n2026-10-28T02:30:01Z running 30 tests\n2026-10-28T02:30:02Z test result: ok. 30 passed; 0 failed\n".into(),truncated:false});
                        }
                        _ => return Err(not_found()),
                    }
                }
                _ => return Err(not_found()),
            };
            return Ok(Response::Json(normalize(endpoint, response)));
        }
        if let ["repos", "demo", name, rest @ ..] = parts.as_slice() {
            let repo = self
                .repositories
                .iter_mut()
                .find(|repo| repo["name"] == *name)
                .ok_or_else(not_found)?;
            if method == Method::Patch && rest.is_empty() {
                let fields = body
                    .and_then(Value::as_object)
                    .filter(|fields| fields.len() == 1)
                    .ok_or_else(invalid)?;
                let (key, value) = fields.iter().next().unwrap();
                let valid = match key.as_str() {
                    "description" => value.as_str().is_some_and(|v| v.chars().count() <= 4096),
                    "default_branch" => value
                        .as_str()
                        .is_some_and(|v| matches!(v, "main" | "feat/retry-budget")),
                    "delete_branch_on_merge"
                    | "allow_merge_commit"
                    | "allow_squash_merge"
                    | "allow_rebase_merge"
                    | "has_issues"
                    | "has_wiki" => value.is_boolean(),
                    _ => false,
                };
                if !valid {
                    return Err(invalid());
                }
                repo[key] = value.clone();
                return Ok(Response::Json(normalize(endpoint, repo.clone())));
            }
            if method == Method::Post {
                match rest {
                    ["actions", "runs", id, action] if matches!(*action, "rerun" | "cancel") => {
                        let run = self
                            .runs
                            .iter_mut()
                            .find(|run| {
                                run["_demo_repository"] == *name
                                    && run["id"].as_u64() == id.parse().ok()
                            })
                            .ok_or_else(not_found)?;
                        run["status"] = if *action == "cancel" {
                            "completed"
                        } else {
                            "queued"
                        }
                        .into();
                        run["conclusion"] = if *action == "cancel" {
                            "cancelled".into()
                        } else {
                            Value::Null
                        };
                        return Ok(Response::Json(Value::Null));
                    }
                    ["actions", "jobs", id, "rerun"] => {
                        let job = self
                            .jobs
                            .iter_mut()
                            .find(|job| {
                                job["_demo_repository"] == *name
                                    && job["id"].as_u64() == id.parse().ok()
                            })
                            .ok_or_else(not_found)?;
                        job["status"] = "queued".into();
                        job["conclusion"] = Value::Null;
                        return Ok(Response::Json(Value::Null));
                    }
                    _ => (),
                }
            }
        }
        Err(invalid())
    }
    fn pull(&self, repository: &str) -> Value {
        let repo = self
            .repositories
            .iter()
            .find(|repo| repo["name"] == repository)
            .cloned()
            .unwrap_or(Value::Null);
        json!({"id":501,"number":42,"title":self.language.text("Refine upstream retries","Уточнить повторы запросов к upstream"),"body":self.language.text("Add a bounded retry budget and timeout checks.\n\n## Checks\n- cargo test\n- cargo clippy","Ограниченный бюджет повторов и проверка тайм-аутов.\n\n## Проверки\n- cargo test\n- cargo clippy"),"state":"open","draft":false,"head":{"ref":"feat/retry-budget"},"base":{"ref":"main","repo":repo},"user":{"login":"demo"},"labels":[{"name":"backend"},{"name":"review"}],"html_url":format!("https://github.com/demo/{repository}/pull/42"),"repository_url":format!("https://api.github.com/repos/demo/{repository}"),"created_at":"2026-10-28T01:30:00Z","updated_at":"2026-10-28T02:30:00Z"})
    }
}
fn page(mut values: Vec<Value>, query: &str) -> Vec<Value> {
    let params: std::collections::BTreeMap<_, _> = query
        .split('&')
        .filter_map(|part| part.split_once('='))
        .collect();
    let page = params
        .get("page")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let per_page = params
        .get("per_page")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    if let Some(q) = params.get("q") {
        let q = decode(q);
        let query = q
            .split_whitespace()
            .filter(|s| !s.contains(':'))
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if !query.is_empty() {
            values.retain(|v| v.to_string().to_lowercase().contains(&query));
        }
    }
    if let Some(branch) = params.get("branch") {
        let branch = decode(branch);
        values.retain(|v| v["head_branch"] == branch);
    }
    values
        .into_iter()
        .skip(page.saturating_sub(1).saturating_mul(per_page))
        .take(per_page)
        .collect()
}
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            )
        {
            out.push((hi * 16 + lo) as u8);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn not_found() -> ApiError {
    ApiError {
        message: "GitHub resource unavailable (404)".into(),
        uncertain: false,
        http_status: Some(404),
    }
}
fn invalid() -> ApiError {
    ApiError {
        message: "Unsupported GitHub demo request or value".into(),
        uncertain: false,
        http_status: Some(422),
    }
}

pub(crate) fn viewer_query(endpoint: &str, login: &str) -> String {
    endpoint.replace("involves%3A%40me", &format!("involves%3A{}", encode(login)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_envelopes_and_user_content_are_preserved() {
        let pr = normalize(
            "search/issues",
            json!({"items":[{"number":42,"title":"Мой PR","body":"Русский текст","state":"open","user":{"login":"igor"},"repository_url":"https://api.github.com/repos/igor/deck"}]}),
        );
        assert_eq!(pr[0]["iid"], 42);
        assert_eq!(pr[0]["description"], "Русский текст");
        assert_eq!(pr[0]["_project"]["full_name"], "igor/deck");
        let runs = normalize(
            "repos/a/b/actions/runs?page=1",
            json!({"workflow_runs":[{"id":7,"status":"completed","conclusion":"failure","head_branch":"main"}]}),
        );
        assert_eq!(runs[0]["status"], "failed");
        assert_eq!(runs[0]["ref"], "main");
        let jobs = normalize(
            "repos/pulls/contents/actions/runs/7/jobs",
            json!({"jobs":[{"id":8,"status":"completed","conclusion":"success"}]}),
        );
        assert_eq!(jobs[0]["id"], 8);
        assert_eq!(jobs[0]["stage"], "Actions");
        let pull = normalize(
            "repos/pulls/contents/pulls/42",
            json!({"number":42,"base":{"ref":"main","repo":{"id":101,"name":"contents","full_name":"pulls/contents"}}}),
        );
        assert_eq!(pull["_project"]["id"], 101);
        assert_eq!(pull["_project"]["path_with_namespace"], "pulls/contents");

        let notes = normalize(
            "repos/a/b/issues/42/comments",
            json!([{"body":"Текст","user":{"login":"igor"}}]),
        );
        assert_eq!(notes[0]["author"]["name"], "igor");
        assert_eq!(notes[0]["body"], "Текст");
    }
    #[test]
    fn contents_are_paged_locally_and_unsupported_entries_cannot_be_opened() {
        let rows=(0..73).map(|i|json!({"name":format!("file-{i}"),"path":format!("file-{i}"),"sha":format!("{i:040}"),"type":"file"})).collect::<Vec<_>>();
        let page = normalize("repos/a/b/contents?per_page=50&page=2", json!(rows));
        assert_eq!(page.as_array().unwrap().len(), 23);
        assert_eq!(page[0]["name"], "file-50");
        let rows = normalize(
            "repos/a/b/contents",
            json!([{"name":"link","type":"symlink","sha":"123"},{"name":"sub","type":"file","submodule_git_url":"https://example.org/a.git","sha":"456"}]),
        );
        assert_eq!(rows[0]["_unavailable"], true);
        assert_eq!(rows[1]["_unavailable"], true);
        let diff = normalize(
            "repos/a/b/pulls/42/files",
            json!([{"filename":"binary.png"}]),
        );
        assert_eq!(diff[0]["collapsed"], true);
        assert_eq!(diff[0]["diff"], "");
        for repository in ["demo/deck", "pulls/contents", "contents/jobs", "git/blobs"] {
            for directory in [
                "files",
                "comments",
                "jobs",
                "branches",
                "pulls",
                "actions/runs",
            ] {
                let endpoint =
                    format!("repos/{repository}/contents/{directory}?per_page=50&page=1");
                let rows = normalize(
                    &endpoint,
                    json!([{"name":"test.rs","path":format!("{directory}/test.rs"),"sha":"123","type":"file"}]),
                );
                assert_eq!(rows[0]["id"], "123", "{endpoint}: {rows}");
                assert_eq!(rows[0]["type"], "blob", "{endpoint}");
            }
            let endpoint = format!("repos/{repository}");
            let repo = normalize(
                &endpoint,
                json!({"id":101,"name":"deck","full_name":repository}),
            );
            assert_eq!(repo["id"], 101, "{endpoint}: {repo}");
            assert_eq!(repo["path_with_namespace"], repository, "{endpoint}");
        }
    }

    #[test]
    fn demo_mutations_and_language_preserve_repository_data() {
        let mut demo = GitHubDemo::new(Language::En);
        let request = Request::Json {
            endpoint: "repos/demo/api-gateway".into(),
            method: Method::Patch,
            body: Some(json!({"description":"Мой текст"})),
        };
        demo.execute(&request).unwrap();
        demo.set_language(Language::Ru);
        let Response::Json(repo) = demo
            .execute(&Request::get("repos/demo/api-gateway"))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(repo["description"], "Мой текст");
        demo.execute(&Request::post(
            "repos/demo/api-gateway/actions/runs/1842/rerun",
        ))
        .unwrap();
        let Response::Json(runs) = demo
            .execute(&Request::get("repos/demo/api-gateway/actions/runs"))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(runs[0]["status"], "queued");
        let Response::Json(other_runs) = demo
            .execute(&Request::get("repos/demo/web-console/actions/runs"))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(other_runs[0]["status"], "success");
        assert!(
            demo.execute(&Request::post(
                "repos/demo/web-console/actions/runs/1842/cancel"
            ))
            .is_err()
        );
        let Response::Json(other_pr) = demo
            .execute(&Request::get("repos/demo/web-console/pulls/42"))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(
            other_pr["web_url"],
            "https://github.com/demo/web-console/pull/42"
        );

        assert!(
            demo.execute(&Request::post(
                "repos/demo/api-gateway/actions/jobs/9270/cancel"
            ))
            .is_err()
        );
    }
}
