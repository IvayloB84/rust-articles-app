use axum::{
    Json, Router,
    extract::{Form, Path, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, sqlite::SqliteConnectOptions};
use std::{net::SocketAddr, str::FromStr};

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[derive(FromRow, Serialize)]
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

    let app = build_router(db.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Checkpoint and close SQLite WAL cleanly on SIGTERM
    db.close().await;
    println!("SQLite pool closed cleanly.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    println!("Shutdown signal received...");
}

async fn list_articles(State(state): State<AppState>) -> impl IntoResponse {
    let articles: Vec<Article> =
        sqlx::query_as("SELECT id, title, body FROM articles ORDER BY id DESC")
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    let mut cards = String::new();
    for a in &articles {
        cards.push_str(&format!(
            r#"<article class="card">
                <div class="card-header">
                    <span class="badge">#{}</span>
                    <h2>{}</h2>
                    <form method="post" action="/rust/articles/{}/delete" class="delete-form">
                        <button type="submit" class="btn-delete" title="Delete article">Delete</button>
                    </form>
                </div>
                <p>{}</p>
            </article>"#,
            a.id,
            escape_html(&a.title),
            a.id,
            escape_html(&a.body)
        ));
    }

    if cards.is_empty() {
        cards =
            r#"<div class="empty">No articles published yet. Write your first post above!</div>"#
                .to_string();
    }

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Rust Articles Engine</title>
<style>
  :root {{
    --bg: #0f172a; --surface: #1e293b; --border: #334155;
    --text: #f8fafc; --muted: #94a3b8; --accent: #f97316; --accent-hover: #ea580c;
  }}
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: var(--bg); color: var(--text); line-height: 1.6;
    max-width: 780px; margin: 0 auto; padding: 40px 20px;
  }}
  header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 28px; border-bottom: 1px solid var(--border); padding-bottom: 16px; }}
  header h1 {{ font-size: 1.6rem; display: flex; align-items: center; gap: 10px; }}
  .pill {{ background: rgba(249,115,22,0.15); color: var(--accent); border: 1px solid rgba(249,115,22,0.4); padding: 4px 12px; border-radius: 999px; font-size: 0.85rem; font-weight: 600; }}
  .composer {{ background: var(--surface); border: 1px solid var(--border); border-radius: 12px; padding: 20px; margin-bottom: 32px; box-shadow: 0 4px 20px rgba(0,0,0,0.25); }}
  input, textarea {{
    width: 100%; background: var(--bg); color: var(--text); border: 1px solid var(--border);
    border-radius: 8px; padding: 12px; font-size: 0.95rem; margin-bottom: 12px; font-family: inherit;
  }}
  input:focus, textarea:focus {{ outline: none; border-color: var(--accent); }}
  .btn-primary {{
    background: var(--accent); color: #fff; border: none; padding: 10px 20px;
    border-radius: 8px; font-weight: 600; font-size: 0.95rem; cursor: pointer; transition: background 0.15s;
  }}
  .btn-primary:hover {{ background: var(--accent-hover); }}
  .card {{ background: var(--surface); border: 1px solid var(--border); border-radius: 12px; padding: 20px; margin-bottom: 16px; }}
  .card-header {{ display: flex; align-items: center; gap: 10px; margin-bottom: 10px; }}
  .card-header h2 {{ font-size: 1.15rem; flex: 1; }}
  .badge {{ background: var(--bg); color: var(--muted); border: 1px solid var(--border); padding: 2px 8px; border-radius: 6px; font-size: 0.8rem; font-family: monospace; }}
  .card p {{ color: #cbd5e1; white-space: pre-wrap; }}
  .btn-delete {{ background: transparent; color: var(--muted); border: 1px solid var(--border); padding: 4px 10px; border-radius: 6px; font-size: 0.8rem; cursor: pointer; }}
  .btn-delete:hover {{ color: #ef4444; border-color: #ef4444; }}
  .empty {{ text-align: center; color: var(--muted); padding: 40px; border: 1px dashed var(--border); border-radius: 12px; }}
</style>
</head>
<body>
  <header>
    <h1>Rust Articles Engine</h1>
    <span class="pill">SQLite WAL &bull; {} Posts</span>
  </header>
  <form method="post" action="/rust/articles" class="composer">
    <input name="title" placeholder="Article title..." required>
    <textarea name="body" placeholder="Write your thoughts in Rust..." rows="4" required></textarea>
    <button type="submit" class="btn-primary">Publish Article</button>
  </form>
  <main>{}</main>
</body>
</html>"#,
        articles.len(),
        cards
    ))
}

async fn list_articles_json(State(state): State<AppState>) -> Json<Vec<Article>> {
    let articles = sqlx::query_as("SELECT id, title, body FROM articles ORDER BY id DESC")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(articles)
}

async fn create_article(
    State(state): State<AppState>,
    Form(input): Form<CreateArticle>,
) -> impl IntoResponse {
    if let Err(err) = sqlx::query("INSERT INTO articles (title, body) VALUES (?, ?)")
        .bind(input.title.trim())
        .bind(input.body.trim())
        .execute(&state.db)
        .await
    {
        eprintln!("Failed to insert article: {err}");
    }

    Redirect::to("/rust/articles")
}

async fn delete_article(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    if let Err(err) = sqlx::query("DELETE FROM articles WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
    {
        eprintln!("Failed to delete article {id}: {err}");
    }

    Redirect::to("/rust/articles")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn build_router(db: SqlitePool) -> Router {
    let state = AppState { db };

    let rust_routes = Router::new()
        .route("/articles", get(list_articles).post(create_article))
        .route("/api/articles", get(list_articles_json))
        .route("/articles/{id}/delete", post(delete_article))
        .route("/health", get(|| async { "OK" }));

    Router::new().nest("/rust", rust_routes).with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt; // provides `.oneshot(...)`

    async fn setup_test_app() -> Router {
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE articles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                body TEXT NOT NULL
            )",
        )
        .execute(&db)
        .await
        .unwrap();

        build_router(db)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/rust/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"OK");
    }

    #[tokio::test]
    async fn test_create_and_fetch_json_articles() {
        let app = setup_test_app().await;

        // 1. Submit a POST form to create an article
        let create_req = Request::builder()
            .method("POST")
            .uri("/rust/articles")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("title=CI+Test+Post&body=Verified+in+memory"))
            .unwrap();

        let create_res = app.clone().oneshot(create_req).await.unwrap();
        assert_eq!(create_res.status(), StatusCode::SEE_OTHER);

        // 2. Fetch the JSON API endpoint and verify the created article is returned
        let api_req = Request::builder()
            .uri("/rust/api/articles")
            .body(Body::empty())
            .unwrap();

        let api_res = app.oneshot(api_req).await.unwrap();
        assert_eq!(api_res.status(), StatusCode::OK);

        let body = api_res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["title"], "CI Test Post");
        assert_eq!(json[0]["body"], "Verified in memory");
    }
}
