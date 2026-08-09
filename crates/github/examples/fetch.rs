use stellr_core::{Provider, RepoRef};
use stellr_github::{auth, sync::GithubProvider};

#[tokio::main]
async fn main() {
    let token = auth::resolve_token().expect("GitHub token is required");
    let provider = GithubProvider::new(token).expect("GitHub provider initialization failed");
    let repo = RepoRef {
        owner: "teloverge".into(),
        name: "stellr".into(),
    };
    let issues = provider
        .fetch(&repo)
        .await
        .expect("GitHub fetch failed")
        .issues;
    println!("{}", issues.len());
}
