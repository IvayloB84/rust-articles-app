use axum::{
    Router,
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect},
    routing::get,
};
use serde::Deserialize;
use sqlx::{FromRow, SqlitePool, sqlite::SqliteConnectOptions};
use std::{net::SocketAddr, str::FromStr};

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[derive(FromRow)]
struct Article {
    id: i64,
    title: String,
    body: String,
}

#[derive(Deserialize)]
struct CreateArticle {
    title: String,
    body: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:///app/storage/production.sqlite3?mode=rwc".into());

    let opts = SqliteConnectOptions::from_str(&db_url)?
        .pragma("journal_mode", "WAL")
        .pragma("synchronous", "NORMAL")
        .pragma("mmap_size", "134217728")
        .create_if_missing(true);

    let db = SqlitePool::connect_with(opts).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS articles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            body TEXT NOT NULL
        )",
    )
    .execute(&db)
    .await?;

    let state = AppState { db };

    let rust_routes = Router::new()
        .route("/articles", get(list_articles).post(create_article))
        .route("/health", get(|| async { "OK" }));

    let app = Router::new().nest("/rust", rust_routes).with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn list_articles(State(state): State<AppState>) -> impl IntoResponse {
    let articles: Vec<Article> =
        sqlx::query_as("SELECT id, title, body FROM articles ORDER BY id DESC")
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    let mut items_html = String::new();
    for a in &articles {
        items_html.push_str(&format!(
            "<div style='border:1px solid #ddd;padding:12px;margin:10px 0;border-radius:6px'>\
             <h3>#{} — {}</h3><p>{}</p></div>",
            a.id,
            escape_html(&a.title),
            escape_html(&a.body)
        ));
    }

    if items_html.is_empty() {
        items_html = "<p><em>No articles yet. Create one below!</em></p>".to_string();
    }

    Html(format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Rust Articles Engine</title></head>
<body style="font-family:system-ui,sans-serif;max-width:720px;margin:40px auto;padding:0 16px">
  <h1>Rust Articles Engine</h1>
  <p>Backed by SQLite WAL on 2Gi PVC &bull; Total articles: <strong>{}</strong></p>
  <form method="post" action="/rust/articles" style="display:flex;flex-direction:column;gap:8px;margin:20px 0">
    <input name="title" placeholder="Article title" required style="padding:8px;font-size:16px">
    <textarea name="body" placeholder="Write your article..." rows="3" required style="padding:8px;font-size:16px"></textarea>
    <button type="submit" style="padding:10px;background:#e44d26;color:#fff;border:none;border-radius:4px;cursor:pointer">Publish Article</button>
  </form>
  <hr>
  {}
</body>
</html>"#,
        articles.len(),
        items_html
    ))
}

async fn create_article(
    State(state): State<AppState>,
    Form(input): Form<CreateArticle>,
) -> impl IntoResponse {
    let _ = sqlx::query("INSERT INTO articles (title, body) VALUES (?, ?)")
        .bind(input.title.trim())
        .bind(input.body.trim())
        .execute(&state.db)
        .await;

    Redirect::to("/rust/articles")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
