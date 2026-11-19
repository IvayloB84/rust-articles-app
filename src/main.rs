use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{net::SocketAddr, str::FromStr};

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize Prometheus /metrics recorder
    let prom_handle = PrometheusBuilder::new().install_recorder()?;

    // 2. Connect to SQLite on your 10Gi PVC with WAL & mmap pragmas
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:///app/storage/production.sqlite3?mode=rwc".into());

    let opts = SqliteConnectOptions::from_str(&db_url)?
        .pragma("journal_mode", "WAL")
        .pragma("synchronous", "NORMAL")
        .pragma("mmap_size", "134217728")
        .create_if_missing(true);

    let db = SqlitePool::connect_with(opts).await?;

    // Create table automatically if missing
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

    // 3. Match your existing /rails Traefik path & readiness probe (/rails/articles)
    let rails_routes = Router::new()
        .route("/articles", get(list_articles))
        .route("/health", get(|| async { "OK" }));

    let app = Router::new()
        .nest("/rails", rails_routes)
        .route("/metrics", get(move || async move { prom_handle.render() }))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn list_articles(State(state): State<AppState>) -> impl IntoResponse {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM articles")
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    Html(format!(
        "<h1>Rust Articles Engine</h1><p>Articles in SQLite WAL DB: {}</p>",
        count.0
    ))
}
