use crate::presentation::handler::health::health;
use axum::{Router, routing::get};
use std::net::SocketAddr;

pub async fn run(addr: SocketAddr) -> Result<(), std::io::Error> {
    // build our application with a route
    let api_routes = Router::new().route("/health", get(health));
    let app = Router::new().nest("/v1", api_routes);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await
}
