use std::time::Duration;

use fake::Fake;
use rand::{thread_rng, Rng};
use rand_distr::Alphanumeric;
use reqwest::StatusCode;
use sqlx::SqlitePool;
use weather_server_lib::api::{LoginBody, RegisterBody, RegisterResponseBody, WeatherResponseBody};
use weather_server_lib::config::Config;
use weather_server_lib::{create_token, hash_password, queries};

#[tokio::test]
#[serial_test::serial]
async fn health_check_succeeds() {
    let database = spawn_server().await;

    let client = reqwest::Client::default();
    let response = client
        .get("http://127.0.0.1:8000/api/health_check")
        .send()
        .await
        .expect("health check failed");

    assert!(response.status().is_success());

    database.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn register_succeeds() {
    let database = spawn_server().await;

    let user = User::random();
    let request_body = RegisterBody {
        username: user.username,
        email: user.email,
        password: user.password,
    };

    let client = reqwest::Client::default();
    let response = client
        .post("http://127.0.0.1:8000/api/register")
        .json(&request_body)
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status(), StatusCode::CREATED);

    let response_body = response
        .json::<RegisterResponseBody>()
        .await
        .expect("could not obtain registration response body");

    assert_eq!(response_body.user_id, 1);

    database.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn login_with_username_succeeds() {
    let database = spawn_server().await;

    let user = User::random();
    let password_hash = hash_password(&user.password).expect("password hashing failed");
    queries::register_user(
        &database.connection,
        &user.username,
        &user.email,
        &password_hash,
    )
    .await
    .expect("user persisting failed");

    let request_body = LoginBody {
        identifier: user.username,
        password: user.password,
    };

    let client = reqwest::Client::default();
    let response = client
        .post("http://127.0.0.1:8000/api/login")
        .json(&request_body)
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status(), StatusCode::OK);

    database.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn login_with_email_succeeds() {
    let database = spawn_server().await;

    let user = User::random();
    let password_hash = hash_password(&user.password).expect("password hashing failed");
    queries::register_user(
        &database.connection,
        &user.username,
        &user.email,
        &password_hash,
    )
    .await
    .expect("user persisting failed");

    let request_body = LoginBody {
        identifier: user.email,
        password: user.password,
    };

    let client = reqwest::Client::default();
    let response = client
        .post("http://127.0.0.1:8000/api/login")
        .json(&request_body)
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status(), StatusCode::OK);

    database.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn get_weather_with_logged_in_user_succeeds() {
    let database = spawn_server().await;

    let token = create_token(0).expect("token creation failed");
    let authorization = format!("Bearer {}", token);

    let client = reqwest::Client::default();
    let response = client
        .get("http://127.0.0.1:8000/api/weather")
        .header("Authorization", authorization)
        .send()
        .await
        .expect("weather request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let _ = response
        .json::<WeatherResponseBody>()
        .await
        .expect("could not obtain weather data");

    database.close().await
}

#[must_use]
async fn spawn_server() -> Database {
    let mut config = Config::read().unwrap();

    config.database_name = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(|x| x as char)
        .collect();

    let server = weather_server_lib::setup(&config)
        .await
        .expect("server initialization failed");

    let database = server.database();
    tokio::spawn(server.serve());

    // Poem server does not initialize quickly enough for us to query it immediately
    tokio::time::sleep(Duration::from_secs(1)).await;

    Database::new(&config.database_name, &database)
}

struct Database {
    name: String,
    connection: SqlitePool,
}

impl Database {
    fn new(name: &str, connection: &SqlitePool) -> Self {
        Self {
            name: name.to_owned(),
            connection: connection.clone(),
        }
    }

    async fn close(self) {
        // We need to close the connection or some changes are not flushed
        // Possibly not important as we will be deleting those files
        self.connection.close().await;

        // Cleanup files SQLite files created for this test
        std::env::current_dir()
            .and_then(std::fs::read_dir)
            .into_iter() // Turns Result<ReadDir> to Iterator<Item=ReadDir> with one item
            .flatten() // Removes the previously introduced Iterator and gives us the Iterator<Item=Result<DirEntry>> inside ReadDir
            .flatten() // Removes the Results so the iterator becomes Iterator<Item=DirEntry>
            .filter(|f| f.file_name().to_string_lossy().contains(&self.name))
            .for_each(|f| {
                let _ = std::fs::remove_file(f.path());
            });
    }
}

struct User {
    username: String,
    email: String,
    password: String,
}

impl User {
    fn random() -> Self {
        Self {
            username: fake::faker::internet::en::Username().fake(),
            email: fake::faker::internet::en::SafeEmail().fake(),
            password: fake::faker::internet::en::Password(8..16).fake(),
        }
    }
}
