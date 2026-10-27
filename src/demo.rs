use crate::model::{ApiError, Method, Request, Response, SETTINGS, value_label};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) struct Demo {
    projects: Vec<Value>,
    groups: Vec<Value>,
    mrs: Vec<Value>,
    pipelines: Vec<Value>,
    jobs: Vec<Value>,
}

const README: &str = "1111111111111111111111111111111111111111";
const CARGO: &str = "2222222222222222222222222222222222222222";
const SOURCE: &str = "3333333333333333333333333333333333333333";

impl Demo {
    pub(crate) fn new() -> Self {
        let projects = [
            (101, "api-gateway", "platform/core", "Единая точка входа для внутренних сервисов.", "rust"),
            (102, "web-console", "products/console", "Панель управления продуктами.", "typescript"),
            (103, "billing-service", "products/billing", "Счета, подписки и история оплат.", "rust"),
            (104, "gitlab-runner", "platform/infrastructure", "Конфигурация раннеров и сборок.", "infra"),
            (105, "design-system", "products/shared", "Компоненты интерфейса и темы.", "design"),
            (106, "terraform-live", "platform/infrastructure", "Описание инфраструктуры.", "terraform"),
            (107, "event-bus", "platform/core", "Доставка событий между сервисами.", "rust"),
        ].into_iter().map(|(id, name, namespace, description, topic)| json!({
            "id":id,"name":name,"path":name,"path_with_namespace":format!("{namespace}/{name}"),
            "name_with_namespace":format!("{namespace} / {name}"),"description":description,
            "visibility":"private","default_branch":"main","archived":false,
            "namespace":{"id":if namespace.starts_with("platform") {1} else {2},"name":namespace,"full_path":namespace,"kind":"group"},
            "web_url":format!("https://gitlab.example.org/{namespace}/{name}"),
            "http_url_to_repo":format!("https://gitlab.example.org/{namespace}/{name}.git"),
            "ssh_url_to_repo":format!("git@gitlab.example.org:{namespace}/{name}.git"),
            "last_activity_at":"2026-10-27T09:42:00Z","created_at":"2026-09-01T09:00:00Z",
            "topics":[topic,"internal"],"star_count":3,"forks_count":0,"open_issues_count":12,
            "permissions":{"project_access":{"access_level":40},"group_access":null},
            "merge_method":"merge","squash_option":"default_off",
            "only_allow_merge_if_pipeline_succeeds":true,
            "only_allow_merge_if_all_discussions_are_resolved":true,
            "remove_source_branch_after_merge":true,
            "issues_access_level":"enabled","merge_requests_access_level":"enabled",
            "wiki_access_level":"disabled","builds_access_level":"enabled",
            "shared_runners_enabled":true,"build_timeout":3600,
        })).collect();
        let groups = vec![
            json!({"id":1,"name":"Platform","path":"platform","full_path":"platform","description":"Общие сервисы и инфраструктура","visibility":"private","web_url":"https://gitlab.example.org/groups/platform"}),
            json!({"id":2,"name":"Products","path":"products","full_path":"products","description":"Продуктовые команды","visibility":"private","web_url":"https://gitlab.example.org/groups/products"}),
        ];
        let mrs = vec![
            json!({"id":501,"iid":42,"project_id":101,"title":"Уточнить повторы запросов к upstream","description":"Добавлен ограниченный бюджет повторов и проверка тайм-аутов.\n\n## Проверки\n- cargo test\n- cargo clippy","state":"opened","source_branch":"feat/retry-budget","target_branch":"main","draft":false,"merge_status":"can_be_merged","detailed_merge_status":"mergeable","author":{"name":"Igor Kramar","username":"ikramar"},"labels":["backend","review"],"created_at":"2026-10-27T08:10:00Z","updated_at":"2026-10-27T09:40:00Z","web_url":"https://gitlab.example.org/platform/core/api-gateway/-/merge_requests/42","changes_count":"2","head_pipeline":{"id":1842,"status":"success","ref":"feat/retry-budget"}}),
            json!({"id":502,"iid":7,"project_id":102,"title":"Добавить страницу состояния сервисов","description":"Список сервисов, состояния и история последних проверок.","state":"opened","source_branch":"feat/status-page","target_branch":"main","draft":true,"merge_status":"unchecked","author":{"name":"Alex","username":"alex"},"labels":["frontend"],"created_at":"2026-10-27T07:20:00Z","updated_at":"2026-10-27T09:30:00Z","web_url":"https://gitlab.example.org/products/console/web-console/-/merge_requests/7","head_pipeline":{"id":1843,"status":"running","ref":"feat/status-page"}}),
        ];
        let pipelines = vec![
            json!({"id":1842,"iid":93,"project_id":101,"status":"success","ref":"main","sha":"7f3a91cb812db72aaa45b65f4c213930b25ea321","source":"push","created_at":"2026-10-27T09:40:00Z","updated_at":"2026-10-27T09:42:14Z","duration":134,"web_url":"https://gitlab.example.org/platform/core/api-gateway/-/pipelines/1842"}),
            json!({"id":1843,"iid":64,"project_id":102,"status":"running","ref":"feat/status-page","sha":"b4829de1312db72aaa45b65f4c213930b25ea321","source":"merge_request_event","created_at":"2026-10-27T09:41:00Z","updated_at":"2026-10-27T09:42:00Z","duration":60,"web_url":"https://gitlab.example.org/products/console/web-console/-/pipelines/1843"}),
            json!({"id":1844,"iid":26,"project_id":103,"status":"failed","ref":"main","sha":"a61cdab1312db72aaa45b65f4c213930b25ea321","source":"push","created_at":"2026-10-27T09:28:00Z","updated_at":"2026-10-27T09:30:00Z","duration":120,"web_url":"https://gitlab.example.org/products/billing/billing-service/-/pipelines/1844"}),
        ];
        let jobs = [(9270,101,1842,"fmt","checks","success",4), (9271,101,1842,"unit-tests","tests","success",42), (9272,101,1842,"container","build","success",31), (9273,101,1842,"deploy","deploy","manual",0), (9274,102,1843,"integration","tests","running",60), (9275,103,1844,"unit-tests","tests","failed",23)]
            .into_iter().map(|(id,project_id,pipeline_id,name,stage,status,duration)| json!({
                "id":id,"project_id":project_id,"name":name,"stage":stage,"status":status,
                "duration":duration,"ref":"main","allow_failure":false,
                "pipeline":{"id":pipeline_id,"project_id":project_id,"ref":"main","status":status},
                "runner":{"id":3,"description":"runner-03","runner_type":"instance_type"},
                "commit":{"short_id":"7f3a91c","title":"Обновить обработку запросов"},
                "created_at":"2026-10-27T09:40:00Z","web_url":format!("https://gitlab.example.org/projects/{project_id}/-/jobs/{id}")
            })).collect();
        Self {
            projects,
            groups,
            mrs,
            pipelines,
            jobs,
        }
    }

    pub(crate) fn execute(&mut self, request: &Request) -> Result<Response, ApiError> {
        match request {
            Request::Projects(ids) => ids
                .iter()
                .map(|id| self.project(*id).cloned())
                .collect::<Result<Vec<_>, _>>()
                .map(|rows| Response::Json(Value::Array(rows))),
            Request::Json {
                endpoint,
                method,
                body,
            } => self.route(endpoint, *method, body.as_ref(), false),
            Request::Text { endpoint } => self.route(endpoint, Method::Get, None, true),
        }
    }

    fn project(&self, id: u64) -> Result<&Value, ApiError> {
        self.projects
            .iter()
            .find(|p| p["id"].as_u64() == Some(id))
            .ok_or_else(not_found)
    }

    fn route(
        &mut self,
        endpoint: &str,
        method: Method,
        body: Option<&Value>,
        text: bool,
    ) -> Result<Response, ApiError> {
        let (path, query) = endpoint.split_once('?').unwrap_or((endpoint, ""));
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let params = params(query)?;
        if method == Method::Get && !text {
            if parts == ["projects"] {
                return list(self.projects.clone(), &params);
            }
            if parts == ["groups"] {
                return list(self.groups.clone(), &params);
            }
            if parts == ["merge_requests"] {
                return list(self.mrs.clone(), &params);
            }
            if let ["groups", id, "projects"] = parts.as_slice() {
                let id = identifier(id)?;
                let group = self
                    .groups
                    .iter()
                    .find(|g| g["id"].as_u64() == Some(id))
                    .ok_or_else(not_found)?;
                let prefix = format!("{}/", group["full_path"].as_str().unwrap_or_default());
                return list(
                    self.projects
                        .iter()
                        .filter(|p| {
                            p["path_with_namespace"]
                                .as_str()
                                .is_some_and(|p| p.starts_with(&prefix))
                        })
                        .cloned()
                        .collect(),
                    &params,
                );
            }
        }
        let ["projects", project_id, rest @ ..] = parts.as_slice() else {
            return Err(not_found());
        };
        let project_id = identifier(project_id)?;
        let project = self.project(project_id)?.clone();
        match (method, rest) {
            (Method::Put, []) if !text => {
                let fields = body
                    .and_then(Value::as_object)
                    .filter(|fields| fields.len() == 1)
                    .ok_or_else(|| invalid("Изменяется ровно одно поле проекта"))?;
                let (key, value) = fields.iter().next().unwrap();
                let setting = SETTINGS
                    .iter()
                    .find(|s| s.key == key && project.get(key).is_some())
                    .ok_or_else(|| invalid("Это поле недоступно для редактирования"))?;
                if setting
                    .parse(&value_label(value))
                    .map_err(|_| invalid("Недопустимое значение настройки"))?
                    != *value
                {
                    return Err(invalid("Неверный JSON-тип настройки"));
                }
                let stored = self
                    .projects
                    .iter_mut()
                    .find(|p| p["id"].as_u64() == Some(project_id))
                    .unwrap();
                stored[key] = value.clone();
                Ok(Response::Json(stored.clone()))
            }
            (Method::Get, []) if !text => Ok(Response::Json(project)),
            (Method::Get, ["repository", "branches"]) if !text => list(
                vec![
                    json!({"name":"main","default":true,"protected":true,"merged":false,"commit":{"short_id":"7f3a91c","title":"Обновить обработку запросов"}}),
                    json!({"name":"feat/retry-budget","default":false,"protected":false,"merged":false,"commit":{"short_id":"c913bad","title":"Добавить бюджет повторов"}}),
                ],
                &params,
            ),
            (Method::Get, ["repository", "tree"]) if !text => {
                if params
                    .get("ref")
                    .is_some_and(|branch| !["main", "feat/retry-budget"].contains(&branch.as_str()))
                {
                    return Err(not_found());
                }
                let path = params.get("path").map(String::as_str).unwrap_or("");
                let entries = match path {
                    "" => vec![
                        json!({"id":"4444444444444444444444444444444444444444","name":"src","type":"tree","path":"src","mode":"040000"}),
                        json!({"id":README,"name":"README.md","type":"blob","path":"README.md","mode":"100644"}),
                        json!({"id":CARGO,"name":"Cargo.toml","type":"blob","path":"Cargo.toml","mode":"100644"}),
                    ],
                    "src" => vec![
                        json!({"id":SOURCE,"name":"lib.rs","type":"blob","path":"src/lib.rs","mode":"100644"}),
                    ],
                    _ => return Err(not_found()),
                };
                list(entries, &params)
            }
            (Method::Get, ["repository", "blobs", sha, "raw"]) if text => {
                let body = match *sha {
                    README => format!("# {}\n\n{}\n\n## Разработка\n\n```sh\ncargo test\ncargo clippy\n```\n", project["name"].as_str().unwrap_or_default(), project["description"].as_str().unwrap_or_default()),
                    CARGO => format!("[package]\nname = \"{}\"\nversion = \"0.4.2\"\nedition = \"2024\"\n",project["path"].as_str().unwrap_or_default()),
                    SOURCE => "pub fn health() -> &'static str {\n    \"ok\"\n}\n\n#[test]\nfn service_is_healthy() {\n    assert_eq!(health(), \"ok\");\n}\n".into(),
                    _ => return Err(not_found()),
                };
                Ok(Response::Text {
                    body,
                    truncated: false,
                })
            }
            (Method::Get, ["merge_requests"]) if !text => list(
                self.mrs
                    .iter()
                    .filter(|mr| mr["project_id"].as_u64() == Some(project_id))
                    .cloned()
                    .collect(),
                &params,
            ),
            (Method::Get, ["merge_requests", iid, tail @ ..]) if !text => {
                let iid = identifier(iid)?;
                let mr = self
                    .mrs
                    .iter()
                    .find(|m| {
                        m["project_id"].as_u64() == Some(project_id)
                            && m["iid"].as_u64() == Some(iid)
                    })
                    .cloned()
                    .ok_or_else(not_found)?;
                match *tail {
                    [] => Ok(Response::Json(mr)),
                    ["diffs"] => list(
                        vec![
                            json!({"old_path":"src/lib.rs","new_path":"src/lib.rs","new_file":false,"renamed_file":false,"deleted_file":false,"collapsed":false,"too_large":false,"diff":"@@ -1,3 +1,5 @@\n pub fn health() -> &'static str {\n-    \"starting\"\n+    \"ok\"\n }\n+\n+// Проверка сервиса проходит через общий маршрут.\n"}),
                        ],
                        &params,
                    ),
                    ["notes"] => list(
                        vec![
                            json!({"id":601,"body":"Добавлена проверка тайм-аутов. Можно смотреть.","system":false,"author":{"name":"Igor Kramar","username":"ikramar"},"created_at":"2026-10-27T09:20:00Z"}),
                            json!({"id":602,"body":"Спасибо, поведение upstream теперь понятно.","system":false,"author":{"name":"Alex","username":"alex"},"created_at":"2026-10-27T09:35:00Z"}),
                        ],
                        &params,
                    ),
                    _ => Err(not_found()),
                }
            }
            (Method::Get, ["pipelines"]) if !text => list(
                self.pipelines
                    .iter()
                    .filter(|p| p["project_id"].as_u64() == Some(project_id))
                    .cloned()
                    .collect(),
                &params,
            ),
            (Method::Get, ["pipelines", pipeline_id, "jobs"]) if !text => {
                let pipeline_id = identifier(pipeline_id)?;
                self.pipelines
                    .iter()
                    .find(|p| {
                        p["project_id"].as_u64() == Some(project_id)
                            && p["id"].as_u64() == Some(pipeline_id)
                    })
                    .ok_or_else(not_found)?;
                list(
                    self.jobs
                        .iter()
                        .filter(|job| {
                            job["project_id"].as_u64() == Some(project_id)
                                && job["pipeline"]["id"].as_u64() == Some(pipeline_id)
                        })
                        .cloned()
                        .collect(),
                    &params,
                )
            }
            (Method::Get, ["jobs", job_id, "trace"]) if text => {
                let job_id = identifier(job_id)?;
                let job = self
                    .jobs
                    .iter()
                    .find(|j| {
                        j["project_id"].as_u64() == Some(project_id)
                            && j["id"].as_u64() == Some(job_id)
                    })
                    .ok_or_else(not_found)?;
                let tail = if job["status"] == "failed" {
                    "test auth::expired_token ... FAILED\n\nFAILED: 37 passed; 1 failed\nERROR: Job failed: exit code 101\n"
                } else {
                    "test routes::health ... ok\ntest auth::expired_token ... ok\ntest proxy::upstream_timeout ... ok\n\ntest result: ok. 38 passed; 0 failed; 0 ignored\nJob succeeded\n"
                };
                Ok(Response::Text {
                    body: format!(
                        "Running with gitlab-runner 18.5.0\nPreparing the docker executor\nUsing image rust:1.90\n\n$ cargo test --locked\n   Compiling {} v0.4.2\n    Finished test profile in 18.7s\n\nrunning 38 tests\n{tail}",
                        project["name"].as_str().unwrap_or_default()
                    ),
                    truncated: false,
                })
            }
            (Method::Post, ["pipelines", pipeline_id, action])
                if !text && ["retry", "cancel"].contains(action) =>
            {
                let pipeline_id = identifier(pipeline_id)?;
                let pipeline = self
                    .pipelines
                    .iter_mut()
                    .find(|p| {
                        p["project_id"].as_u64() == Some(project_id)
                            && p["id"].as_u64() == Some(pipeline_id)
                    })
                    .ok_or_else(not_found)?;
                let status = if *action == "cancel" {
                    "canceled"
                } else {
                    "pending"
                };
                if *action == "retry"
                    && !self.jobs.iter().any(|job| {
                        job["project_id"].as_u64() == Some(project_id)
                            && job["pipeline"]["id"].as_u64() == Some(pipeline_id)
                            && ["failed", "canceled"].iter().any(|s| job["status"] == *s)
                    })
                {
                    return Ok(Response::Json(pipeline.clone()));
                }
                pipeline["status"] = json!(status);
                for job in self.jobs.iter_mut().filter(|j| {
                    j["project_id"].as_u64() == Some(project_id)
                        && j["pipeline"]["id"].as_u64() == Some(pipeline_id)
                }) {
                    if *action == "cancel"
                        || ["failed", "canceled"].iter().any(|s| job["status"] == *s)
                    {
                        job["status"] = json!(status);
                    }
                    job["pipeline"]["status"] = json!(status);
                }
                Ok(Response::Json(pipeline.clone()))
            }
            (Method::Post, ["jobs", job_id, action])
                if !text && ["retry", "cancel", "play"].contains(action) =>
            {
                let job_id = identifier(job_id)?;
                let job = self
                    .jobs
                    .iter_mut()
                    .find(|j| {
                        j["project_id"].as_u64() == Some(project_id)
                            && j["id"].as_u64() == Some(job_id)
                    })
                    .ok_or_else(not_found)?;
                if *action == "play" && job["status"] != "manual" {
                    return Err(invalid("Джоба не ожидает ручного запуска"));
                }
                let status = if *action == "cancel" {
                    "canceled"
                } else {
                    "pending"
                };
                job["status"] = json!(status);
                job["pipeline"]["status"] = json!(status);
                let pipeline_id = job["pipeline"]["id"].as_u64();
                if let Some(pipeline) = self.pipelines.iter_mut().find(|p| {
                    p["project_id"].as_u64() == Some(project_id) && p["id"].as_u64() == pipeline_id
                }) {
                    pipeline["status"] = json!(status);
                }
                Ok(Response::Json(job.clone()))
            }
            _ => Err(not_found()),
        }
    }
}

fn identifier(value: &str) -> Result<u64, ApiError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(not_found)
}
fn not_found() -> ApiError {
    invalid("Демонстрационный ресурс не найден (404)")
}
fn invalid(message: &str) -> ApiError {
    ApiError {
        http_status: None,
        message: message.into(),
        uncertain: false,
    }
}

fn params(query: &str) -> Result<BTreeMap<String, String>, ApiError> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            Ok((decode(key)?, decode(value)?))
        })
        .collect()
}

fn decode(value: &str) -> Result<String, ApiError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                    .map_err(|_| invalid("Некорректный URL"))?;
                output.push(u8::from_str_radix(hex, 16).map_err(|_| invalid("Некорректный URL"))?);
                index += 3;
            }
            b'%' => return Err(invalid("Некорректный URL")),
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(output).map_err(|_| invalid("Некорректный UTF-8 URL"))
}

fn list(mut rows: Vec<Value>, params: &BTreeMap<String, String>) -> Result<Response, ApiError> {
    if let Some(search) = params.get("search").filter(|s| !s.is_empty()) {
        let needle = search.to_lowercase();
        rows.retain(|row| {
            [
                "name",
                "path_with_namespace",
                "full_path",
                "description",
                "title",
                "path",
            ]
            .iter()
            .any(|key| {
                row[*key]
                    .as_str()
                    .is_some_and(|text| text.to_lowercase().contains(&needle))
            })
        });
    }
    let page = params
        .get("page")
        .map(|n| n.parse::<usize>())
        .transpose()
        .map_err(|_| invalid("Некорректная страница"))?
        .unwrap_or(1);
    let per_page = params
        .get("per_page")
        .map(|n| n.parse::<usize>())
        .transpose()
        .map_err(|_| invalid("Некорректный размер страницы"))?
        .unwrap_or(50);
    if page == 0 || !(1..=100).contains(&per_page) {
        return Err(invalid("Некорректная пагинация"));
    }
    let skip = page.saturating_sub(1).saturating_mul(per_page);
    Ok(Response::Json(Value::Array(
        rows.into_iter().skip(skip).take(per_page).collect(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn json(response: Response) -> Value {
        let Response::Json(value) = response else {
            panic!("expected JSON")
        };
        value
    }
    #[test]
    fn routes_isolate_projects_and_support_navigation() {
        let mut demo = Demo::new();
        let projects = json(
            demo.execute(&Request::get("projects?simple=true&per_page=1&page=2"))
                .unwrap(),
        );
        assert_eq!(projects[0]["id"], 102);
        let group = json(
            demo.execute(&Request::get("groups/1/projects?search=api"))
                .unwrap(),
        );
        assert_eq!(group.as_array().unwrap().len(), 1);
        assert_eq!(group[0]["id"], 101);
        assert!(
            demo.execute(&Request::get("projects/102/merge_requests/42"))
                .is_err()
        );
        assert!(
            demo.execute(&Request::get("projects/102/pipelines/1842/jobs"))
                .is_err()
        );
        assert!(
            demo.execute(&Request::text("projects/102/jobs/9271/trace"))
                .is_err()
        );
        assert!(
            demo.execute(&Request::get("projects/999/repository/tree"))
                .is_err()
        );
        assert!(demo.execute(&Request::get("projects/101/unknown")).is_err());
        assert!(demo.execute(&Request::get("projects?page=0")).is_err());
        assert_eq!(
            json(
                demo.execute(&Request::get("projects/101/repository/tree?path=src"))
                    .unwrap()
            )[0]["path"],
            "src/lib.rs"
        );
        assert!(matches!(
            demo.execute(&Request::text(format!(
                "projects/101/repository/blobs/{SOURCE}/raw"
            )))
            .unwrap(),
            Response::Text { .. }
        ));
        assert_eq!(
            json(
                demo.execute(&Request::get("projects/101/merge_requests/42/diffs"))
                    .unwrap()
            )[0]["new_path"],
            "src/lib.rs"
        );
        assert!(
            json(
                demo.execute(&Request::get("projects/101/merge_requests/42/notes"))
                    .unwrap()
            )
            .as_array()
            .unwrap()
            .len()
                > 1
        );
    }
    #[test]
    fn mutations_change_only_selected_field_and_selected_ci_resource() {
        let mut demo = Demo::new();
        let mut before = json(demo.execute(&Request::get("projects/101")).unwrap());
        let result = json(
            demo.execute(&Request::put(
                "projects/101",
                json!({"description":"Новое описание"}),
            ))
            .unwrap(),
        );
        before["description"] = json!("Новое описание");
        assert_eq!(result, before);
        assert_eq!(
            json(demo.execute(&Request::get("projects/101")).unwrap()),
            before
        );
        assert!(
            demo.execute(&Request::put(
                "projects/101",
                json!({"description":"x","name":"y"})
            ))
            .is_err()
        );
        assert!(
            demo.execute(&Request::put("projects/101", json!({"private_token":"x"})))
                .is_err()
        );
        assert!(
            demo.execute(&Request::put(
                "projects/101",
                json!({"build_timeout":"600"})
            ))
            .is_err()
        );
        assert_eq!(
            json(
                demo.execute(&Request::post("projects/101/pipelines/1842/retry"))
                    .unwrap()
            )["status"],
            "success"
        );
        assert_eq!(
            json(
                demo.execute(&Request::post("projects/101/jobs/9273/play"))
                    .unwrap()
            )["status"],
            "pending"
        );
        assert_eq!(
            json(
                demo.execute(&Request::post("projects/103/pipelines/1844/retry"))
                    .unwrap()
            )["status"],
            "pending"
        );
        assert_eq!(
            json(
                demo.execute(&Request::get("projects/102/pipelines"))
                    .unwrap()
            )[0]["status"],
            "running"
        );
        assert_eq!(
            json(
                demo.execute(&Request::post("projects/101/pipelines/1842/cancel"))
                    .unwrap()
            )["status"],
            "canceled"
        );
        assert!(
            demo.execute(&Request::post("projects/101/jobs/9271/play"))
                .is_err()
        );
    }
}
