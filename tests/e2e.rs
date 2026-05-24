mod support;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn message_submission_creates_queued_task() {
    let app = support::test_app().await;

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/sessions",
            json!({ "title": "TDD" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let session = response_json(response).await;
    let session_id = session["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/sessions/{session_id}/messages"),
            json!({ "text": "hello" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = response_json(response).await;
    let task_id = body["task_id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(empty_request("GET", &format!("/v1/tasks/{task_id}")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let task = response_json(response).await;

    assert_eq!(task["status"], "queued");
    assert_eq!(task["session_id"], session_id);
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn empty_request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
