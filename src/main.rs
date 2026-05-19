use askama::Template;
use axum::{
    Router,
    extract::{Form, Path},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use pulldown_cmark::{Parser, html};
use serde::Deserialize;
use std::process::Command;
use std::{fs, path::PathBuf};

/* templates */
#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    users: Vec<String>,
}

#[derive(Template)]
#[template(path = "profile.html")]
struct ProfileTemplate {
    user: String,
    repos: Vec<String>,
}

#[derive(Template)]
#[template(path = "repo.html")]
struct RepoTemplate {
    user: String,
    repo: String,
    readme: String,
}

#[derive(Template)]
#[template(path = "commits.html")]
struct CommitsTemplate {
    user: String,
    repo: String,
    commits: Vec<String>,
}

#[derive(Template)]
#[template(path = "tree.html")]
struct TreeTemplate {
    user: String,
    repo: String,
    path: String,
    files: Vec<String>,
}

#[derive(Template)]
#[template(path = "blob.html")]
struct BlobTemplate {
    user: String,
    repo: String,
    path: String,
    content: String,
}

#[derive(Template)]
#[template(
    source = "<li><a href=\"/{{ name }}\">👤 <strong>{{ name }}</strong></a></li>",
    ext = "html"
)]
struct UserItemFragment {
    name: String,
}

#[derive(Template)]
#[template(
    source = "<li><a href=\"/{{ user }}/{{ name }}\">📦 <strong>{{ name }}</strong></a></li>",
    ext = "html"
)]
struct RepoItemFragment {
    user: String,
    name: String,
}

/* forms */
#[derive(Deserialize)]
struct CreateUserForm {
    username: String,
}

#[derive(Deserialize)]
struct CreateRepoForm {
    reponame: String,
}

fn run_git(repo_dir: &PathBuf, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo_dir)
        .args(args)
        .output()
        .unwrap_or_else(|_| panic!("Failed to run git command in {:?}", repo_dir));

    String::from_utf8_lossy(&output.stdout).to_string()
}

fn render<T: askama::Template>(template: T) -> axum::response::Html<String> {
    axum::response::Html(template.render().unwrap())
}

/* route handlers */
async fn home() -> impl IntoResponse {
    let mut users = vec![];
    if let Ok(entries) = fs::read_dir("repos") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                users.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    render(IndexTemplate { users })
}

async fn create_user(Form(form): Form<CreateUserForm>) -> impl IntoResponse {
    fs::create_dir_all(format!("repos/{}", form.username)).unwrap();
    render(UserItemFragment {
        name: form.username,
    })
}

async fn profile(Path(user): Path<String>) -> impl IntoResponse {
    let mut repos = vec![];
    if let Ok(entries) = fs::read_dir(format!("repos/{}", user)) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                repos.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    render(ProfileTemplate { user, repos })
}

async fn create_repo(
    Path(user): Path<String>,
    Form(form): Form<CreateRepoForm>,
) -> impl IntoResponse {
    let repo_path = PathBuf::from(format!("repos/{}/{}", user, form.reponame));
    fs::create_dir_all(&repo_path).unwrap();

    run_git(&repo_path, &["init"]);
    run_git(
        &repo_path,
        &["commit", "--allow-empty", "-m", "Initial commit"],
    );

    fs::write(
        repo_path.join("README.md"),
        format!("# {}\n\nWelcome to your new repo!", form.reponame),
    )
    .unwrap();
    run_git(&repo_path, &["add", "README.md"]);
    run_git(&repo_path, &["commit", "-m", "Add README.md"]);

    render(RepoItemFragment {
        user,
        name: form.reponame,
    })
}

async fn repo(Path((user, repo)): Path<(String, String)>) -> impl IntoResponse {
    let repo_path = PathBuf::from(format!("repos/{}/{}", user, repo));
    let readme_md = run_git(&repo_path, &["show", "HEAD:README.md"]);

    let parser = Parser::new(&readme_md);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    render(RepoTemplate {
        user,
        repo,
        readme: html_output,
    })
}

async fn commits(Path((user, repo)): Path<(String, String)>) -> impl IntoResponse {
    let repo_path = PathBuf::from(format!("repos/{}/{}", user, repo));
    let log_output = run_git(&repo_path, &["log", "--oneline"]);
    let commits: Vec<String> = log_output.lines().map(|s| s.to_string()).collect();

    render(CommitsTemplate {
        user,
        repo,
        commits,
    })
}

async fn tree(Path((user, repo, path)): Path<(String, String, String)>) -> impl IntoResponse {
    let repo_path = PathBuf::from(format!("repos/{}/{}", user, repo));
    /* ls-tree output format: <mode> <type> <object> <file> */
    let tree_output = run_git(&repo_path, &["ls-tree", "HEAD", &path]);
    let files: Vec<String> = tree_output
        .lines()
        .map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            parts.get(1).unwrap_or(&"").to_string()
        })
        .collect();

    render(TreeTemplate {
        user,
        repo,
        path,
        files,
    })
}

async fn blob(Path((user, repo, path)): Path<(String, String, String)>) -> impl IntoResponse {
    let repo_path = PathBuf::from(format!("repos/{}/{}", user, repo));
    let content = run_git(&repo_path, &["show", &format!("HEAD:{}", path)]);

    render(BlobTemplate {
        user,
        repo,
        path,
        content,
    })
}

/* entry point */
#[tokio::main]
async fn main() {
    fs::create_dir_all("repos").unwrap();

    let app = Router::new()
        .route("/", get(home))
        .route("/create-user", post(create_user))
        .route("/{user}", get(profile))
        .route("/{user}/create-repo", post(create_repo))
        .route("/{user}/{repo}", get(repo))
        .route("/{user}/{repo}/commits", get(commits))
        .route("/{user}/{repo}/tree/{*path}", get(tree))
        .route("/{user}/{repo}/blob/{*path}", get(blob));

    println!("🚀 RetroGit running on http://127.0.0.1:3000");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
