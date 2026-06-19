mod support;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

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
    assert!(body.get("message").is_none());

    let response = app
        .clone()
        .oneshot(empty_request("GET", &format!("/v1/tasks/{task_id}")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let task = response_json(response).await;

    assert_eq!(task["status"], "queued");
    assert_eq!(task["session_id"], session_id);

    let response = app
        .oneshot(empty_request(
            "GET",
            &format!("/v1/sessions/{session_id}/messages"),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json(response).await;
    let messages = body["messages"].as_array().unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["task_id"], task_id);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"][0]["text"], "hello");
}

#[tokio::test]
async fn message_submission_to_missing_session_returns_not_found() {
    let app = support::test_app().await;
    let session_id = Uuid::new_v4();

    let response = app
        .oneshot(json_request(
            "POST",
            &format!("/v1/sessions/{session_id}/messages"),
            json!({ "text": "hello" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
