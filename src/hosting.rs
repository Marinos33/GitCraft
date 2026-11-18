//! Integrations (Preferences > Integrations): GitHub, GitHub Enterprise Server, GitLab,
//! GitLab Self-Managed, Bitbucket, Bitbucket Data Center, Azure DevOps (hosting services) and Jira Cloud,
//! Jira Data Center, Trello (issue trackers). This file: connections, sign-in, tokens for Git, remotes.
//! hosting_api.rs: pull requests, issues, Launchpad, repositories to clone, SSH keys.
//! Tokens live in the operating system's credential store, never in GitCraft's settings file; hosts, user
//! names and organisations are ordinary settings passed in with `set_config`.

pub use crate::hosting_api::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::RwLock;

const SERVICE: &str = "GitCraft";

pub fn secret_get(name: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, name).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

pub fn secret_set(name: &str, value: &str) -> Result<(), String> {
    let e = keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())?;
    if value.is_empty() {
        match e.delete_credential() { Ok(()) | Err(keyring::Error::NoEntry) => Ok(()), Err(error) => Err(error.to_string()) }
    } else {
        e.set_password(value).map_err(|e| e.to_string())
    }
}

/// The supported integrations, in display order.
pub const SERVICES: [&str; 10] = ["github", "github-enterprise", "gitlab", "gitlab-self", "bitbucket", "bitbucket-server", "azure", "jira", "jira-server", "trello"];

pub fn label(id: &str) -> &'static str {
    match id {
        "github" => "GitHub",
        "github-enterprise" => "GitHub Enterprise Server",
        "gitlab" => "GitLab",
        "gitlab-self" => "GitLab Self-Managed",
        "bitbucket" => "Bitbucket",
        "bitbucket-server" => "Bitbucket Data Center",
        "azure" => "Azure DevOps",
        "jira" => "Jira Cloud",
        "jira-server" => "Jira Data Center",
        "trello" => "Trello",
        _ => "this service",
    }
}

/// Name of a service's token in the credential store ("github-token" and "gitlab-token" as before).
pub fn secret_name(id: &str) -> String {
    format!("{id}-token")
}

/// Hosts, user names and organisations for the services (not secret).
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub github_enterprise_host: String,
    pub gitlab_host: String,
    /// Bitbucket user name for an app password / API token (empty: the token is an OAuth / access token).
    pub bitbucket_user: String,
    pub bitbucket_server_host: String,
    pub bitbucket_server_user: String,
    pub azure_org: String,
    /// Jira Cloud site, e.g. mycompany.atlassian.net, and the account e-mail its API token belongs to.
    pub jira_site: String,
    pub jira_email: String,
    pub jira_server_host: String,
    /// Trello API key (the token is the secret).
    pub trello_key: String,
}

static CONFIG: RwLock<Option<Config>> = RwLock::new(None);

pub fn set_config(c: Config) {
    if let Ok(mut w) = CONFIG.write() {
        *w = Some(c);
    }
}

pub fn config() -> Config {
    CONFIG.read().ok().and_then(|c| c.clone()).unwrap_or_default()
}

fn clean_host(h: &str) -> String {
    h.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string()
}

/// The web host of a service ("" when it isn't set up).
pub fn host_of(id: &str) -> String {
    let c = config();
    match id {
        "github" => "github.com".into(),
        "github-enterprise" => clean_host(&c.github_enterprise_host),
        "gitlab" => "gitlab.com".into(),
        "gitlab-self" => clean_host(&c.gitlab_host),
        "bitbucket" => "bitbucket.org".into(),
        "bitbucket-server" => clean_host(&c.bitbucket_server_host),
        "azure" => "dev.azure.com".into(),
        "jira" => clean_host(&c.jira_site),
        "jira-server" => clean_host(&c.jira_server_host),
        "trello" => "trello.com".into(),
        _ => String::new(),
    }
}

pub fn api_base(id: &str) -> String {
    let h = host_of(id);
    match id {
        "github" => "https://api.github.com".into(),
        "github-enterprise" => format!("https://{h}/api/v3"),
        "gitlab" | "gitlab-self" => format!("https://{h}/api/v4"),
        "bitbucket" => "https://api.bitbucket.org/2.0".into(),
        "bitbucket-server" => format!("https://{h}/rest/api/1.0"),
        "azure" => format!("https://dev.azure.com/{}", config().azure_org.trim()),
        "jira" => format!("https://{h}/rest/api/3"),
        "jira-server" => format!("https://{h}/rest/api/2"),
        "trello" => "https://api.trello.com/1".into(),
        _ => String::new(),
    }
}

pub enum Auth {
    Bearer(String),
    Header(&'static str, String),
    Basic(String, String),
    /// Trello: key and token travel as query parameters.
    Query(String, String),
}

pub fn auth(id: &str) -> Result<Auth, String> {
    let token = secret_get(&secret_name(id)).ok_or_else(|| format!("Connect {} first (Preferences > Integrations).", label(id)))?;
    let c = config();
    Ok(match id {
        "github" | "github-enterprise" | "bitbucket-server" | "jira-server" => Auth::Bearer(token),
        "gitlab" | "gitlab-self" => Auth::Header("PRIVATE-TOKEN", token),
        "bitbucket" if c.bitbucket_user.trim().is_empty() => Auth::Bearer(token),
        "bitbucket" => Auth::Basic(c.bitbucket_user.trim().to_string(), token),
        // A personal access token, or (after a browser sign-in through Git Credential Manager) an OAuth JWT.
        "azure" if token.starts_with("eyJ") => Auth::Bearer(token),
        "azure" => Auth::Basic(String::new(), token),
        "jira" => Auth::Basic(c.jira_email.trim().to_string(), token),
        "trello" => Auth::Query(c.trello_key.trim().to_string(), token),
        _ => return Err("Unknown integration".into()),
    })
}

fn b64(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(s)
}

/// One HTTPS request to a service's REST API; JSON in and out.
pub struct HttpPage { pub value: Value, pub link: Option<String>, pub next_page: Option<String>, pub continuation: Option<String> }

pub fn http(method: &str, url: &str, auth: &Auth, body: Option<Value>) -> Result<Value, String> {
    http_page(method, url, auth, body).map(|page| page.value)
}

pub fn http_page(method: &str, url: &str, auth: &Auth, body: Option<Value>) -> Result<HttpPage, String> {
    http_page_request(method, url, auth, body).map_err(|mut error| {
        let secrets = match auth {
            Auth::Bearer(value) | Auth::Header(_, value) => vec![value.clone()],
            Auth::Basic(user, password) => vec![password.clone(), b64(&format!("{user}:{password}"))],
            Auth::Query(key, token) => vec![key.clone(), token.clone()],
        };
        for secret in secrets.into_iter().filter(|s| !s.is_empty()) {
            error = error.replace(&crate::hosting_api::q(&secret), "[redacted]").replace(&secret, "[redacted]");
        }
        error
    })
}

fn http_page_request(method: &str, url: &str, auth: &Auth, body: Option<Value>) -> Result<HttpPage, String> {
    crate::read_jobs::check()?;
    let context = crate::read_jobs::context();
    let url = match auth {
        Auth::Query(k, t) => format!("{url}{}key={}&token={}", if url.contains('?') { "&" } else { "?" }, crate::hosting_api::q(k), crate::hosting_api::q(t)),
        _ => url.to_string(),
    };
    static SLOTS: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    let mut request = crate::net::client().request(method.parse().map_err(|_| "Invalid HTTP method")?, &url);
    request = match auth {
        Auth::Bearer(token) => request.bearer_auth(token),
        Auth::Header(name, token) => request.header(*name, token),
        Auth::Basic(user, password) => request.basic_auth(user, Some(password)),
        Auth::Query(..) => request,
    };
    request = if url.contains("api.github.com") || url.contains("/api/v3/") {
        request.header("Accept", "application/vnd.github+json").header("X-GitHub-Api-Version", "2022-11-28")
    } else { request.header("Accept", "application/json") };
    if let Some(body) = body { request = request.json(&body); }
    let limits = crate::net::Limits {
        timeout: std::time::Duration::from_secs(30),
        max_bytes: 10 * 1024 * 1024,
        slots: Some(SLOTS.get_or_init(|| tokio::sync::Semaphore::new(8))),
    };
    let response = crate::net::send(request, limits, &|| crate::read_jobs::check_context(&context)).map_err(|error| match error {
        crate::net::Error::Unreachable => "Could not contact the hosting service. Check its address and connection.".to_string(),
        crate::net::Error::Unreadable => "Could not read the hosting response.".to_string(),
        crate::net::Error::TooLarge => "The hosting response exceeded its 10 MB budget; narrow the query.".to_string(),
        crate::net::Error::TimedOut => "The hosting request timed out.".to_string(),
        crate::net::Error::Cancelled(message) => message,
    })?;
    let status = response.status;
    if response.is_redirect() { return Err("The hosting service redirected this authenticated request. Update its configured address; credentials were not forwarded.".into()); }
    let link = response.header("Link"); let next_page = response.header("X-Next-Page"); let continuation = response.header("X-MS-ContinuationToken");
    let text = response.text();
    let value = if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap_or(Value::String(text.clone())) };
    if status >= 400 {
        let message = value["message"].as_str().or(value["error"]["message"].as_str()).or(value["errorMessages"][0].as_str()).or(value["errors"][0]["message"].as_str()).unwrap_or(&text);
        let hint = match status { 401 => " - the token was not accepted; reconnect in Preferences > Integrations", 403 => " - the token may be missing a permission", _ => "" };
        return Err(format!("{status}: {}{hint}", message.chars().take(300).collect::<String>()));
    }
    Ok(HttpPage { value, link, next_page, continuation })
}

fn s(v: &Value) -> String {
    v.as_str().map(|x| x.to_string()).unwrap_or_default()
}

fn no_window(c: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let _ = c;
}

#[derive(Serialize, Default, Clone)]
pub struct ServiceStatus {
    pub id: String,
    pub label: String,
    pub host: String,
    pub connected: bool,
    pub account: Option<String>,
    pub error: Option<String>,
}

/// Who the saved token belongs to ("Connected as ...").
pub fn whoami_id(id: &str) -> Result<String, String> {
    let get = |path: &str| http("GET", &format!("{}{path}", api_base(id)), &auth(id)?, None);
    let v = match id {
        "github" | "github-enterprise" => return get("/user").map(|v| s(&v["login"])),
        "gitlab" | "gitlab-self" => return get("/user").map(|v| s(&v["username"])),
        "bitbucket" => get("/user")?,
        "bitbucket-server" => {
            get("/application-properties")?;
            let u = config().bitbucket_server_user;
            return Ok(if u.trim().is_empty() { "token accepted".into() } else { u });
        }
        "azure" => http("GET", "https://app.vssps.visualstudio.com/_apis/profile/profiles/me?api-version=7.1", &auth(id)?, None)?,
        "jira" | "jira-server" => get("/myself")?,
        "trello" => get("/members/me")?,
        _ => return Err("Unknown integration".into()),
    };
    let name = [&v["display_name"], &v["displayName"], &v["fullName"], &v["username"], &v["emailAddress"], &v["name"]].into_iter().map(s).find(|x| !x.is_empty());
    Ok(name.unwrap_or_else(|| "connected".into()))
}

pub fn status(id: &str) -> ServiceStatus {
    let mut st = ServiceStatus { id: id.into(), label: label(id).into(), host: host_of(id), ..Default::default() };
    if secret_get(&secret_name(id)).is_none() {
        return st;
    }
    st.connected = true;
    match whoami_id(id) {
        Ok(a) => st.account = Some(a),
        Err(e) => st.error = Some(e),
    }
    st
}

fn gcm(host: &str, op: &str, input: &str) -> Result<std::process::Output, String> {
    use std::io::Write;
    // Plain git (not git_cmd): no GitCraft token helper, and prompts allowed so GCM may open its sign-in window.
    let mut c = std::process::Command::new(crate::prefs::git_program());
    c.args(["-c", &format!("credential.https://{host}.helper="), "-c", &format!("credential.https://{host}.helper=manager"), "credential", op]);
    c.env_remove("GIT_CONFIG_COUNT").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    no_window(&mut c);
    let mut ch = c.spawn().map_err(|e| format!("Could not start Git: {e}"))?;
    ch.stdin.take().ok_or("no stdin")?.write_all(input.as_bytes()).map_err(|e| e.to_string())?;
    ch.wait_with_output().map_err(|e| e.to_string())
}

/// Sign in through the browser with Git Credential Manager (the sign-in Git itself uses) and return the token
/// it hands out - the counterpart of the "Connect" button. github.com, gitlab.com, bitbucket.org,
/// dev.azure.com.
pub fn browser_sign_in(id: &str) -> Result<String, String> {
    let host = match id {
        "github" | "gitlab" | "bitbucket" | "azure" => host_of(id),
        _ => return Err(format!("{} doesn't support browser sign-in here; paste a personal access token instead.", label(id))),
    };
    let o = gcm(&host, "fill", &format!("protocol=https\nhost={host}\n\n"))?;
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    let field = |k: &str| out.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).unwrap_or_default().to_string();
    let password = field("password");
    if !o.status.success() || password.is_empty() {
        let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
        return Err(if err.contains("not a git command") || err.contains("credential-manager") {
            "Git Credential Manager isn't installed. Install Git for Windows (it includes it), or paste a token instead.".into()
        } else if err.is_empty() {
            "The sign-in was cancelled.".into()
        } else {
            err
        });
    }
    // Let GCM keep it, so Git uses the same sign-in.
    let _ = gcm(&host, "approve", &format!("protocol=https\nhost={host}\nusername={}\npassword={password}\n\n", field("username")));
    Ok(password)
}

/// The GitHub CLI's sign-in (`gh auth token`), for people already logged in with `gh`.
pub fn gh_cli_token() -> Result<String, String> {
    let mut c = std::process::Command::new("gh");
    c.args(["auth", "token"]);
    no_window(&mut c);
    let o = c.output().map_err(|_| "The GitHub CLI (gh) isn't installed.".to_string())?;
    let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if o.status.success() && !t.is_empty() {
        Ok(t)
    } else {
        Err("The GitHub CLI isn't signed in (run `gh auth login`).".into())
    }
}

/// Services whose token Git may use for HTTPS remotes.
const GIT_SERVICES: [&str; 7] = ["github", "github-enterprise", "gitlab", "gitlab-self", "bitbucket", "bitbucket-server", "azure"];

/// The user name Git sends with a service's token.
fn git_user(id: &str, c: &Config) -> String {
    match id {
        "github" | "github-enterprise" => "x-access-token".to_string(),
        "gitlab" | "gitlab-self" => "oauth2".to_string(),
        "bitbucket" if c.bitbucket_user.trim().is_empty() => "x-token-auth".to_string(),
        "bitbucket" => c.bitbucket_user.trim().to_string(),
        "bitbucket-server" => c.bitbucket_server_user.trim().to_string(),
        _ => "gitcraft".to_string(),
    }
}

/// The sign-in (user name, token) Git should use for `host`, as Git names it (host[:port]).
pub fn git_login(host: &str) -> Option<(String, String)> {
    let c = config();
    let id = GIT_SERVICES.into_iter().find(|id| {
        let configured = host_of(id);
        let authority = configured.split('/').next().unwrap_or_default();
        !authority.is_empty() && authority.eq_ignore_ascii_case(host)
    })?;
    Some((git_user(id, &c), secret_get(&secret_name(id))?))
}

/// Tell Git which hosts have a connected service. The tokens stay in the credential store; Git
/// asks GitCraft's credential helper for them when it needs them.
pub fn sync_git_credentials(gitlab_host: &str) {
    if !gitlab_host.trim().is_empty() {
        let mut c = config();
        if c.gitlab_host.trim().is_empty() {
            c.gitlab_host = gitlab_host.trim().to_string();
            set_config(c);
        }
    }
    let hosts = GIT_SERVICES.into_iter()
        .filter(|id| secret_get(&secret_name(id)).is_some())
        .map(host_of)
        .filter(|host| !host.is_empty())
        .collect();
    crate::git::set_credential_hosts(hosts);
}

/// `gitcraft --git-credential <operation>`: Git's credential helper protocol on stdin/stdout.
/// Runs before the window opens and only answers `get`.
pub fn git_credential_main(operation: &str) -> i32 {
    use std::io::{Read, Write};
    let mut input = String::new();
    if std::io::stdin().take(64 * 1024).read_to_string(&mut input).is_err() { return 1; }
    if operation == "get" { load_saved_config(); }
    let reply = crate::git::credential_reply(operation, &input, git_login);
    let mut out = std::io::stdout();
    if out.write_all(reply.as_bytes()).and_then(|_| out.flush()).is_err() { return 1; }
    0
}

/// Read the integrations section of the saved preferences, for the credential helper process.
fn load_saved_config() {
    let path = crate::sample::data_dir().join("settings.json");
    let Some(saved) = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { return };
    let settings = &saved["settings"];
    let mut c: Config = serde_json::from_value(settings["integrations"].clone()).unwrap_or_default();
    if c.gitlab_host.trim().is_empty() { c.gitlab_host = s(&settings["gitlabHost"]); }
    set_config(c);
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Github,
    Gitlab,
    Bitbucket,
    Azure,
}

/// Where a remote lives: provider, the integration it belongs to, host and project path.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RemoteRepo {
    pub provider: Provider,
    /// Integration id (github, github-enterprise, gitlab, gitlab-self, bitbucket, bitbucket-server, azure).
    pub service: String,
    pub host: String,
    /// "owner/repo", "group/sub/project", Bitbucket Data Center "PROJECT/repo", Azure "org/project/repo".
    pub path: String,
    pub web: String,
}

/// Understands https://host/owner/repo(.git), git@host:owner/repo(.git), ssh://git@host[:port]/owner/repo, and the
/// Azure DevOps and Bitbucket Data Center address styles.
pub fn parse_remote(url: &str, gitlab_host: &str) -> Option<RemoteRepo> {
    let u = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let (mut host, path) = if let Some(rest) = u.strip_prefix("git@") {
        let (h, p) = rest.split_once(':')?;
        (h.to_string(), p.to_string())
    } else {
        let rest = u.split_once("://").map(|(_, r)| r).unwrap_or(u);
        let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
        let (h, p) = rest.split_once('/')?;
        (if u.starts_with("ssh://") { h.split(':').next().unwrap_or(h).to_string() } else { h.to_string() }, p.to_string())
    };
    let c = config();
    let gl_self = if gitlab_host.trim().is_empty() { clean_host(&c.gitlab_host) } else { clean_host(gitlab_host) };
    let mk = |provider: Provider, service: &str, host: &str, path: String, web: String| Some(RemoteRepo { provider, service: service.into(), host: host.into(), path, web });
    // Azure DevOps: dev.azure.com/{org}/{project}/_git/{repo}, ssh.dev.azure.com:v3/{org}/{project}/{repo},
    // {org}.visualstudio.com/{project}/_git/{repo}.
    if host == "dev.azure.com" || host == "ssh.dev.azure.com" || host.ends_with(".visualstudio.com") {
        let parts: Vec<&str> = path.trim_start_matches("v3/").split('/').filter(|p| *p != "_git").collect();
        let (org, project, repo) = if host.ends_with(".visualstudio.com") {
            (host.trim_end_matches(".visualstudio.com").to_string(), parts.first()?.to_string(), parts.get(1)?.to_string())
        } else {
            (parts.first()?.to_string(), parts.get(1)?.to_string(), parts.get(2)?.to_string())
        };
        let web = format!("https://dev.azure.com/{org}/{project}/_git/{repo}");
        return mk(Provider::Azure, "azure", "dev.azure.com", format!("{org}/{project}/{repo}"), web);
    }
    if u.starts_with("git@") || u.starts_with("ssh://") {
        let hostname = |authority: &str| authority.split('/').next().unwrap_or(authority).split(':').next().unwrap_or(authority).to_ascii_lowercase();
        for authority in [&gl_self, &clean_host(&c.github_enterprise_host), &clean_host(&c.bitbucket_server_host)] {
            if !authority.is_empty() && hostname(authority) == hostname(&host) { host = authority.split('/').next().unwrap_or(authority).to_owned(); break; }
        }
    }
    let simple = |provider: Provider, service: &str| mk(provider, service, &host, path.clone(), format!("https://{host}/{path}"));
    match host.as_str() {
        "github.com" => return simple(Provider::Github, "github"),
        "gitlab.com" => return simple(Provider::Gitlab, "gitlab"),
        "bitbucket.org" => return simple(Provider::Bitbucket, "bitbucket"),
        _ => {}
    }
    if !gl_self.is_empty() && host == gl_self {
        return simple(Provider::Gitlab, "gitlab-self");
    }
    let ghe = clean_host(&c.github_enterprise_host);
    if !ghe.is_empty() && host == ghe {
        return simple(Provider::Github, "github-enterprise");
    }
    let bbs = clean_host(&c.bitbucket_server_host);
    if !bbs.is_empty() && host == bbs.split('/').next().unwrap_or(&bbs) {
        // https://host/scm/PROJECT/repo or ssh://git@host:7999/PROJECT/repo
        let p = path.trim_start_matches("scm/").to_string();
        let (proj, repo) = p.split_once('/')?;
        let web = format!("https://{bbs}/projects/{}/repos/{repo}", proj.to_uppercase());
        return mk(Provider::Bitbucket, "bitbucket-server", &bbs, format!("{proj}/{repo}"), web);
    }
    None
}

/// The remote a push / pull talks to, and whether it signs in with SSH.
#[derive(Serialize, Default, Debug)]
pub struct RemoteAuth {
    pub remote: String,
    pub url: String,
    pub ssh: bool,
    pub host: String,
    /// The same repository's HTTPS address (for "Use HTTPS instead").
    pub https_url: Option<String>,
    /// GitCraft has a token for this host from Preferences > Integrations.
    pub connected: bool,
    /// The integration this remote belongs to, if any.
    pub service: String,
}

pub fn https_url(url: &str) -> Option<String> {
    let u = url.trim();
    let (host, path) = if let Some(rest) = u.strip_prefix("git@") {
        rest.split_once(':')?
    } else if let Some(rest) = u.strip_prefix("ssh://") {
        let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
        let (h, p) = rest.split_once('/')?;
        (h.split(':').next().unwrap_or(h), p)
    } else {
        return None;
    };
    let path = path.trim_matches('/').trim_end_matches(".git");
    if host == "ssh.dev.azure.com" {
        let p: Vec<&str> = path.trim_start_matches("v3/").split('/').collect();
        if p.len() == 3 {
            return Some(format!("https://dev.azure.com/{}/{}/_git/{}", p[0], p[1], p[2]));
        }
    }
    (!host.is_empty() && !path.is_empty()).then(|| format!("https://{host}/{path}.git"))
}

pub fn remote_auth(dir: &std::path::Path, gitlab_host: &str) -> RemoteAuth {
    let up = crate::git::run(dir, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
    let remotes: Vec<String> = crate::git::run(dir, &["remote"]).stdout.lines().map(|s| s.to_string()).collect();
    let remote = up
        .ok
        .then(|| up.stdout.trim().split('/').next().unwrap_or_default().to_string())
        .filter(|r| remotes.contains(r))
        .or_else(|| remotes.iter().find(|r| *r == "origin").cloned())
        .or_else(|| remotes.first().cloned())
        .unwrap_or_default();
    if remote.is_empty() {
        return RemoteAuth::default();
    }
    let url = crate::git::run(dir, &["remote", "get-url", "--push", &remote]).stdout.trim().to_string();
    let ssh = url.starts_with("git@") || url.starts_with("ssh://");
    let r = parse_remote(&url, gitlab_host);
    let service = r.as_ref().map(|r| r.service.clone()).unwrap_or_default();
    let connected = !service.is_empty() && secret_get(&secret_name(&service)).is_some();
    RemoteAuth { https_url: https_url(&url), remote, url, ssh, host: r.map(|r| r.host).unwrap_or_default(), connected, service }
}

/// Who the token belongs to, for the provider of a remote (kept for the older Accounts screen).
pub fn whoami(provider: &Provider, gitlab_host: &str) -> Result<String, String> {
    match provider {
        Provider::Github => whoami_id("github"),
        Provider::Gitlab if gitlab_host.trim().is_empty() || gitlab_host.trim() == "gitlab.com" => whoami_id("gitlab"),
        Provider::Gitlab => whoami_id("gitlab-self"),
        Provider::Bitbucket => whoami_id("bitbucket"),
        Provider::Azure => whoami_id("azure"),
    }
}

/// "Create branch from issue": issue-123-short-title.
pub fn branch_for_issue(number: u64, title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    format!("issue-{number}-{slug}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_addresses_become_https() {
        assert_eq!(https_url("git@github.com:MystifiedSky/GitCraft.git").as_deref(), Some("https://github.com/MystifiedSky/GitCraft.git"));
        assert_eq!(https_url("ssh://git@gitlab.example.com:2222/team/app").as_deref(), Some("https://gitlab.example.com/team/app.git"));
        assert_eq!(https_url("git@ssh.dev.azure.com:v3/org/proj/repo").as_deref(), Some("https://dev.azure.com/org/proj/_git/repo"));
        assert_eq!(https_url("https://github.com/a/b.git"), None);
    }

    #[cfg(unix)]
    #[test]
    fn git_asks_the_helper_and_gets_the_reply_verbatim() {
        // A stand-in for `gitcraft --git-credential`, configured exactly as git_cmd configures it.
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("helper.sh");
        std::fs::write(&script, "#!/bin/sh\ntest \"$2\" = get || exit 0\nprintf 'username=x-access-token\\npassword=secret123\\n'\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let helper = crate::git::credential_helper(&script);
        let env = crate::git::credential_env(&["github.com".into()], &helper);
        assert!(!env.iter().any(|(_, v)| v.contains("secret123")));
        let mut c = std::process::Command::new("git");
        c.args(["credential", "fill"]).envs(env.iter().cloned()).env("GIT_TERMINAL_PROMPT", "0");
        c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        let mut ch = c.spawn().unwrap();
        use std::io::Write;
        ch.stdin.take().unwrap().write_all(b"protocol=https\nhost=github.com\npath=x/y.git\n\n").unwrap();
        let out = String::from_utf8_lossy(&ch.wait_with_output().unwrap().stdout).into_owned();
        assert!(out.contains("username=x-access-token") && out.contains("password=secret123"), "{out}");
    }

    #[test]
    fn git_logins_match_configured_hosts_and_users() {
        let previous = config();
        set_config(Config { bitbucket_user: " team-user ".into(), github_enterprise_host: "https://GHE.example.test/".into(), ..Default::default() });
        assert_eq!(git_user("bitbucket", &config()), "team-user");
        assert_eq!(git_user("github-enterprise", &config()), "x-access-token");
        assert_eq!(git_user("gitlab-self", &config()), "oauth2");
        // No token in the store for these made-up hosts, so nothing is answered.
        assert_eq!(git_login("unknown.example.test"), None);
        set_config(previous);
    }

    #[test]
    fn understands_remote_urls_and_services() {
        let gh = parse_remote("https://github.com/octo/hello.git", "").unwrap();
        assert_eq!((gh.provider.clone(), gh.path.as_str(), gh.web.as_str()), (Provider::Github, "octo/hello", "https://github.com/octo/hello"));
        assert_eq!(parse_remote("git@github.com:octo/hello.git", "").unwrap().path, "octo/hello");
        let gl = parse_remote("ssh://git@gitlab.com/group/sub/project.git", "").unwrap();
        assert_eq!((gl.provider, gl.path.as_str()), (Provider::Gitlab, "group/sub/project"));
        let own = parse_remote("https://git.example.org/team/app", "git.example.org").unwrap();
        assert_eq!((own.provider, own.service.as_str()), (Provider::Gitlab, "gitlab-self"));
        let bb = parse_remote("git@bitbucket.org:team/app.git", "").unwrap();
        assert_eq!((bb.provider, bb.service.as_str(), bb.path.as_str()), (Provider::Bitbucket, "bitbucket", "team/app"));
        for url in ["https://dev.azure.com/org/Proj/_git/repo", "https://org@dev.azure.com/org/Proj/_git/repo", "git@ssh.dev.azure.com:v3/org/Proj/repo", "https://org.visualstudio.com/Proj/_git/repo"] {
            let az = parse_remote(url, "").unwrap();
            assert_eq!((az.service.as_str(), az.path.as_str()), ("azure", "org/Proj/repo"), "{url}");
        }
        assert!(parse_remote("C:/repos/local.git", "").is_none());
        assert!(parse_remote("https://example.com/a/b", "").is_none());
        // Self-hosted services come from the Integrations settings.
        set_config(Config { github_enterprise_host: "https://git.corp.example/".into(), jira_site: "acme.atlassian.net".into(), azure_org: "acme".into(), ..Default::default() });
        assert_eq!(host_of("github-enterprise"), "git.corp.example");
        assert_eq!(api_base("github-enterprise"), "https://git.corp.example/api/v3");
        assert_eq!(api_base("jira"), "https://acme.atlassian.net/rest/api/3");
        assert_eq!(api_base("azure"), "https://dev.azure.com/acme");
        let ghe = parse_remote("git@git.corp.example:team/app.git", "").unwrap();
        assert_eq!((ghe.provider, ghe.service.as_str()), (Provider::Github, "github-enterprise"));
        assert_eq!(ssh_keys_page("github"), "https://github.com/settings/ssh/new");
        set_config(Config::default());
        assert_eq!(secret_name("github"), "github-token");
    }

    #[test]
    fn issue_branch_names() {
        assert_eq!(branch_for_issue(42, "Crash when saving: file > 2 GB!"), "issue-42-crash-when-saving-file-2-gb");
    }
}

/// Match the configured push destination, including renamed upstream branches.
pub fn pull_request_destination(dir: &std::path::Path, branch: &str, gitlab_host: &str) -> Result<(RemoteRepo, String), String> {
    let remotes = crate::git::run(dir, &["remote"]).stdout;
    let configured = crate::git::run(dir, &["config", "--get", &format!("branch.{branch}.remote")]).stdout.trim().to_owned();
    let remote = if !configured.is_empty() { configured } else if remotes.lines().any(|r| r == "origin") { "origin".into() } else { remotes.lines().next().unwrap_or_default().into() };
    if remote.is_empty() || !remotes.lines().any(|r| r == remote) { return Err("Choose a server remote for this branch before starting a pull request.".into()); }
    let merge = crate::git::run(dir, &["config", "--get", &format!("branch.{branch}.merge")]).stdout.trim().to_owned();
    let destination = if merge.is_empty() { branch.to_owned() } else { merge.strip_prefix("refs/heads/").ok_or("The upstream must point to a branch.")?.to_owned() };
    let url = crate::git::run(dir, &["remote", "get-url", "--push", &remote]);
    if !url.ok { return Err(url.message()); }
    let repository = parse_remote(url.stdout.trim(), gitlab_host).ok_or("The push remote is not a supported hosting service.")?;
    Ok((repository, destination))
}


#[cfg(test)]
mod http_safety_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    #[test]
    fn cancelled_http_body_releases_the_connection_promptly() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap(); let address = server.local_addr().unwrap();
        let (ready, wait) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap(); stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut bytes = [0;8192]; let _ = stream.read(&mut bytes);
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 99999\r\n\r\n{").unwrap(); ready.send(()).unwrap();
            let started = std::time::Instant::now(); let closed = stream.read(&mut bytes).map(|n| n == 0).unwrap_or(true);
            assert!(closed && started.elapsed() < std::time::Duration::from_secs(2));
        });
        let request = std::thread::spawn(move || crate::read_jobs::scope(Some("stalled-http".into()), || http_page("GET", &format!("http://{address}/stalled"), &Auth::Bearer("fixture".into()), None)));
        wait.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        let started = std::time::Instant::now(); crate::read_jobs::cancel("stalled-http");
        assert!(request.join().unwrap().err().unwrap().contains("cancelled"));
        assert!(started.elapsed() < std::time::Duration::from_secs(2)); worker.join().unwrap();
    }

    #[test]
    fn transport_reuses_connections_without_reusing_credentials_and_reconnects() {
        use std::io::BufRead;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap(); let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for count in [5, 1] {
                let (stream, _) = listener.accept().unwrap(); stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
                let mut reader = std::io::BufReader::new(stream);
                for _ in 0..count {
                    let mut request = String::new();
                    loop { let mut line = String::new(); assert!(reader.read_line(&mut line).unwrap() > 0); if line == "\r\n" { break; } request.push_str(&line); }
                    requests.push(request);
                    reader.get_mut().write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").unwrap();
                }
            }
            requests
        });
        for index in 0..6 {
            let token = format!("fixture-account-{index}");
            assert!(http_page("GET", &format!("http://{address}/collection"), &Auth::Bearer(token), None).is_ok());
        }
        for (index, request) in worker.join().unwrap().iter().enumerate() {
            assert!(request.contains(&format!("Bearer fixture-account-{index}")));
            assert_eq!(request.to_ascii_lowercase().matches("authorization:").count(), 1);
        }
    }

    #[test]
    fn credentials_do_not_follow_redirects_or_appear_in_errors() {
        let destination=TcpListener::bind("127.0.0.1:0").unwrap();destination.set_nonblocking(true).unwrap();
        let destination_port=destination.local_addr().unwrap().port();
        for auth in [Auth::Bearer("synthetic-secret".into()),Auth::Header("PRIVATE-TOKEN","synthetic-secret".into()),Auth::Basic("synthetic-user".into(),"synthetic-secret".into()),Auth::Query("synthetic-key".into(),"synthetic-secret".into())] {
            let server=TcpListener::bind("127.0.0.1:0").unwrap();let address=server.local_addr().unwrap();
            let worker=std::thread::spawn(move || {let (mut stream,_)=server.accept().unwrap();let mut bytes=[0u8;8192];let _=stream.read(&mut bytes);write!(stream,"HTTP/1.1 302 Found\r\nLocation: http://localhost:{destination_port}/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();});
            let result=http_page("GET",&format!("http://{address}/cards"),&auth,None);
            assert!(result.is_err());worker.join().unwrap();assert!(destination.accept().is_err());
            let unavailable=TcpListener::bind("127.0.0.1:0").unwrap();let address=unavailable.local_addr().unwrap();drop(unavailable);
            let error=http_page("GET",&format!("http://{address}/cards"),&auth,None).err().unwrap();
            assert!(!error.contains("synthetic-secret"));assert!(!error.contains("synthetic-key"));
        }
        let server=TcpListener::bind("127.0.0.1:0").unwrap();let address=server.local_addr().unwrap();
        let worker=std::thread::spawn(move || {let(mut stream,_)=server.accept().unwrap();let mut bytes=[0u8;8192];let _=stream.read(&mut bytes);let body=r#"{"message":"synthetic-key synthetic-secret"}"#;write!(stream,"HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();});
        let error=http_page("GET",&format!("http://{address}/cards"),&Auth::Query("synthetic-key".into(),"synthetic-secret".into()),None).err().unwrap();worker.join().unwrap();
        assert!(!error.contains("synthetic-"));assert!(error.contains("400"));
    }
}
