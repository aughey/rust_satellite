use axum::{response::Html, routing::get, Router};
use rust_satellite::Result;
use tracing::info;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = Router::new().route("/", get(handler));

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
