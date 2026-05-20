use askama::Template;
use axum::{
    Router,
    extract::{Form, Path},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use pulldown_cmark::{Parser, html};
use serde::Deserialize;
use std::process::Command;
use std::{fs, path::PathBuf};

struct UserInfo {
    name: String,
    has_avatar: bool,
}

/* templates */
#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    users: Vec<UserInfo>,
}

#[derive(Template)]
#[template(path = "profile.html")]
struct ProfileTemplate {
    user: String,
    repos: Vec<String>,
    has_avatar: bool,
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
#[template(path = "blob.html")]
struct BlobTemplate {
    user: String,
    repo: String,
    path: String,
    content: String,
}

#[derive(Template)]
#[template(
    source = "<li><i class=\"ri-user-line\"></i> <a href=\"/{{ name }}\"><strong>{{ name }}</strong></a></li>",
    ext = "html"
)]
struct UserItemFragment {
    name: String,
}

#[derive(Template)]
#[template(
    source = "<li><i class=\"ri-git-repository-line\"></i> <a href=\"/{{ user }}/{{ name }}\"><strong>{{ name }}</strong></a></li>",
    ext = "html"
)]
struct RepoItemFragment {
    user: String,
    name: String,
}

#[derive(Debug)]
struct GitFile {
    name: String,
    mode: String,
    is_dir: bool,
}

#[derive(Template)]
#[template(path = "tree.html")]
struct TreeTemplate {
    user: String,
    repo: String,
    path: String,
    files: Vec<GitFile>,
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

    if !output.stderr.is_empty() {
        println!("GIT ERROR: {}", String::from_utf8_lossy(&output.stderr));
    }

    String::from_utf8_lossy(&output.stdout).to_string()
}

fn render<T: askama::Template>(template: T) -> axum::response::Html<String> {
    axum::response::Html(template.render().unwrap())
}

fn check_avatar(user: &str) -> bool {
    PathBuf::from(format!("repos/{}/avatar.png", user)).exists()
}

/* route handlers */
async fn home() -> impl IntoResponse {
    let mut users = vec![];
    if let Ok(entries) = fs::read_dir("repos") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let has_avatar = check_avatar(&name);

                users.push(UserInfo { name, has_avatar });
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
                let repo_name = entry.file_name().to_string_lossy().to_string();
                // Пропускаем файл avatar.png, чтобы он не отображался как репозиторий!
                if repo_name != "avatar.png" {
                    repos.push(repo_name);
                }
            }
        }
    }

    let has_avatar = check_avatar(&user);
    render(ProfileTemplate {
        user,
        repos,
        has_avatar,
    })
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

async fn render_tree(user: String, repo: String, path: String) -> impl IntoResponse {
    let repo_path = PathBuf::from(format!("repos/{}/{}", user, repo));
    let clean_path = path.trim_start_matches('/').trim_end_matches('/');

    let dir_target = if clean_path.is_empty() {
        String::new()
    } else {
        format!("{}/", clean_path)
    };

    let git_args = if clean_path.is_empty() {
        vec!["ls-tree", "HEAD"]
    } else {
        vec!["ls-tree", "HEAD", &dir_target]
    };

    let tree_output = run_git(&repo_path, &git_args);

    let mut files = Vec::new();
    for line in tree_output.lines() {
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        let metadata_part = parts.get(0).unwrap_or(&"");

        let metadata: Vec<&str> = metadata_part.split_whitespace().collect();
        let mode = metadata.get(0).unwrap_or(&"").to_string();

        if let Some(&full_path) = parts.get(1) {
            let display_name = full_path.split('/').last().unwrap_or(full_path).to_string();

            if !display_name.is_empty() {
                let is_dir = mode.starts_with("04");

                files.push(GitFile {
                    name: display_name,
                    mode,
                    is_dir,
                });
            }
        }
    }

    render(TreeTemplate {
        user,
        repo,
        path: clean_path.to_string(),
        files,
    })
}

async fn tree_root(Path((user, repo)): Path<(String, String)>) -> impl IntoResponse {
    render_tree(user, repo, String::new()).await
}

async fn tree(Path((user, repo, path)): Path<(String, String, String)>) -> impl IntoResponse {
    render_tree(user, repo, path).await
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

async fn get_avatar(Path(user): Path<String>) -> impl IntoResponse {
    let avatar_path = format!("repos/{}/avatar.png", user);

    match fs::read(&avatar_path) {
        Ok(content) => ([(header::CONTENT_TYPE, "image/png")], content).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/* entry point */
#[tokio::main]
async fn main() {
    fs::create_dir_all("repos").unwrap();

    let app = Router::new()
        .route("/", get(home))
        .route("/create-user", post(create_user))
        .route("/.avatars/{user}", get(get_avatar))
        .route("/{user}", get(profile))
        .route("/{user}/create-repo", post(create_repo))
        .route("/{user}/{repo}", get(repo))
        .route("/{user}/{repo}/commits", get(commits))
        .route("/{user}/{repo}/tree", get(tree_root)) // Ловит ровно /tree
        .route("/{user}/{repo}/tree/", get(tree_root)) // Ловит /tree/ (вот она, твоя 404!)
        .route("/{user}/{repo}/tree/{*path}", get(tree))
        .route("/{user}/{repo}/blob/{*path}", get(blob));

    println!("🚀 RetroGit running on http://127.0.0.1:3000");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
