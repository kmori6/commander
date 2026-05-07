use crate::presentation::handler::health_handler::health_handler;
use axum::{Router, routing::get};
use std::net::SocketAddr;

pub async fn run(addr: SocketAddr) -> Result<(), std::io::Error> {
    let api_routes = Router::new().route("/health", get(health_handler));

    let app = Router::new().nest("/v1", api_routes);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
