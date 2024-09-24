use crate::api::Api;
use crate::config::Config;
use crate::http_client::HttpClient;
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use poem::listener::TcpListener;
use poem::{Route, Server};
use poem_openapi::OpenApiService;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};
use std::net::IpAddr;
use std::str::FromStr;

pub mod api;
pub mod config;
pub mod http_client;
pub mod queries;

use argon2::password_hash::SaltString;
use argon2::{
    password_hash, Algorithm, Argon2, Params as Argon2Params, PasswordHash, PasswordHasher,
    PasswordVerifier, Version,
};
use std::sync::OnceLock;
use tokio::task::spawn_blocking;

static JWT_KEYS: OnceLock<Keys> = OnceLock::new();

pub async fn setup(config: &Config) -> Result<PendingServer, anyhow::Error> {
    let database = database(&config.database_name).await?;

    let http_client = HttpClient::new()?;
    let api = Api::new(http_client, database.clone());

    let api_service =
        OpenApiService::new(api, "Weather Server Demo", "1.0").server("http://localhost:3000/api");
    let ui = api_service.swagger_ui();

    let routes = Route::new().nest("/api", api_service).nest("/", ui);

    let address = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(address);

    Ok(PendingServer {
        listener,
        routes,
        database,
    })
}

pub struct PendingServer {
    listener: TcpListener<String>,
    routes: Route,
    database: SqlitePool,
}

impl PendingServer {
    pub async fn serve(self) -> Result<(), std::io::Error> {
        Server::new(self.listener).run(self.routes).await
    }

    #[must_use]
    pub fn database(&self) -> SqlitePool {
        self.database.clone()
    }
}

async fn database(database_name: &str) -> Result<SqlitePool, sqlx::Error> {
    let database_url = format!("sqlite://database/{database_name}.db");

    let database_exists = Sqlite::database_exists(&database_url)
        .await
        .unwrap_or(false);
    if !database_exists {
        Sqlite::create_database(&database_url).await?;
    }

    let database = SqlitePool::connect(&database_url).await?;
    sqlx::migrate!("./migrations").run(&database).await?;

    Ok(database)
}

#[must_use]
pub fn is_loopback_address(ip: &IpAddr) -> bool {
    let loopback_v4 = IpAddr::from_str("127.0.0.1").expect("should be valid IPv4 loopback address");
    let loopback_v6 = IpAddr::from_str("::1").expect("should be valid IPv6 loopback address");

    ip.eq(&loopback_v4) || ip.eq(&loopback_v6)
}

pub fn create_token(user_id: u64) -> Result<String, jsonwebtoken::errors::Error> {
    // We should reduce expiration interval so changes in user can be applied sooner
    let expiration = (Utc::now().naive_utc() + chrono::naive::Days::new(1))
        .and_utc()
        .timestamp() as u64;

    let body = TokenBody {
        user_id,
        expiration,
    };
    let header = Header::default();

    jsonwebtoken::encode(&header, &body, &Keys::get().encoding)
}

#[must_use]
pub fn check_token(token: &str) -> bool {
    jsonwebtoken::decode::<TokenBody>(token, &Keys::get().decoding, &Validation::default()).is_ok()
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TokenBody {
    user_id: u64,
    #[serde(rename = "exp")]
    expiration: u64,
}

struct Keys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Keys {
    fn get() -> &'static Self {
        JWT_KEYS.get_or_init(|| {
            let secret = Self::read_secret();
            Self::new(&secret)
        })
    }

    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }

    fn read_secret() -> Vec<u8> {
        std::env::var("JWT_SECRET")
            .expect("no JWT secret in environment variables, please define 'JWT_SECRET'")
            .into_bytes()
    }
}

pub fn hash_password(password: &str) -> Result<String, password_hash::Error> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let params = Argon2Params::new(15000, 2, 1, None)?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
}

pub async fn validate_password(password: String, hash: Option<String>) -> bool {
    let placeholder_hash = "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
        .to_string();
    let hash = hash.unwrap_or(placeholder_hash);

    compute_password(password, hash).await.is_ok()
}

// This is a separate password so all functions that can fail is encapsulated 
// and we can just call is_ok() to convert all results to bool
async fn compute_password(password: String, hash: String) -> Result<(), anyhow::Error> {
    spawn_blocking(move || {
        let hash = PasswordHash::new(&hash)?;
        Argon2::default().verify_password(password.as_bytes(), &hash)
    })
    .await??;

    Ok(())
}
