use axum::{
    body::{to_bytes, Body},
    extract::connect_info::ConnectInfo,
    http::{Request, StatusCode},
};
use scytale_node::{http_gateway::router, Node, NodeConfig};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::util::ServiceExt;

fn setup_node() -> Arc<Node> {
    let mut node = Node::open(NodeConfig::in_memory()).unwrap();
    node.start().unwrap();
    Arc::new(node)
}

async fn bind(app: &axum::Router, passbook_id: &str, candidate: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::post("/api/v1/alias/bind")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "passbook_id": passbook_id,
                        "candidate": candidate,
                        "signature": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn alias_bind_resolve_conflict_and_validation() {
    let app = router(setup_node());

    let response = bind(&app, "scy1_alice", "SCY-100001").await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "Accepted");

    let response = bind(&app, "scy1_alice", "SCY-100001").await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = bind(&app, "scy1_bob", "SCY-100001").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/alias/resolve/SCY-100001")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["passbook_id"], "scy1_alice");

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/alias/resolve/SCY-999999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/alias/resolve/INVALID")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn write_routes_are_rate_limited_and_do_not_allow_wildcard_cors() {
    let app = router(setup_node());

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/health")
                .header("origin", "https://untrusted.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get("access-control-allow-origin")
        .is_none());

    for _ in 0..30 {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/alias/bind")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/alias/bind")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let mut request = Request::post("/api/v1/alias/bind")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "192.0.2.10:9000".parse::<SocketAddr>().unwrap(),
    ));
    let response = app.oneshot(request).await.unwrap();
    assert_ne!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}
