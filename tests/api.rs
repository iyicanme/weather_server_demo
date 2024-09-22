use std::time::Duration;

use fake::Fake;
use reqwest::StatusCode;

use weather_server_lib::{RegisterBody, RegisterResponse, RegisterResponseBody, server};

async fn spawn_server() {
    let server = server::setup().await.expect("server initialization failed");

    tokio::spawn(server.serve());

    // poem server does not initialize quickly enough for us to query it immediately
    tokio::time::sleep(Duration::from_secs(1)).await;
}

#[tokio::test]
#[serial_test::serial]
async fn health_check_succeeds() {
    spawn_server().await;

    let client = reqwest::Client::default();
    let response = client.get("http://127.0.0.1:8000/api/health_check")
        .send()
        .await
        .expect("health check failed");

    assert!(response.status().is_success());
}

#[tokio::test]
#[serial_test::serial]
async fn register_succeeds() {
    spawn_server().await;

    let username = fake::faker::internet::en::Username().fake();
    let email = fake::faker::internet::en::SafeEmail().fake();
    let password = fake::faker::internet::en::Password(8..16).fake();

    let request_body = RegisterBody { username, email, password };

    let client = reqwest::Client::default();
    let response = client.post("http://127.0.0.1:8000/api/register")
        .json(&request_body)
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status(), StatusCode::CREATED);

    let response_body = response
        .json::<RegisterResponseBody>()
        .await
        .expect("could not obtain registration response body");

    assert_eq!(response_body.user_id, 0);
}