use std::fmt::{Display, Formatter};

use crate::http_client::{HttpClient, WeatherApiResponse};
use crate::queries::SqlError;
use poem::web::RemoteAddr;
use poem_openapi::payload::Json;
use poem_openapi::{ApiResponse, Object, OpenApi};
use sqlx::SqlitePool;

pub mod config;
pub mod http_client;
pub mod queries;
pub mod server;

pub struct Api {
    http_client: HttpClient,
    database: SqlitePool,
}

impl Api {
    #[must_use]
    pub const fn new(http_client: HttpClient, database: SqlitePool) -> Self {
        Self {
            http_client,
            database,
        }
    }
}

#[OpenApi]
impl Api {
    #[oai(path = "/health_check", method = "get")]
    pub async fn health_check(&self) -> HealthResponse {
        HealthResponse::Alive
    }

    #[oai(path = "/register", method = "post")]
    pub async fn register(&self, body: Json<RegisterBody>) -> RegisterResponse {
        let user_id = match queries::register_user(&self.database, &body.username, &body.email, &body.password).await {
            Ok(i) => i,
            Err(SqlError::UniqueConstraintViolation) => return RegisterResponse::AlreadyRegistered,
            Err(SqlError::Other) => return RegisterResponse::RegistrationFailed,
        };

        RegisterResponse::Registered(Json(RegisterResponseBody { user_id }))
    }

    #[oai(path = "/login", method = "post")]
    pub async fn login(&self, body: Json<LoginBody>) -> LoginResponse {
        let password = queries::get_password(&self.database, &body.identifier, &body.identifier).await.unwrap_or_else(|_| String::new());

        if password == body.password {
            LoginResponse::LoggedIn
        } else {
            LoginResponse::WrongCredentials
        }
    }

    #[oai(path = "/weather", method = "get")]
    pub async fn weather(&self, ip: &RemoteAddr) -> WeatherResponse {
        let ip_string = match ip.as_socket_addr() {
            Some(addr) => addr.ip().to_string(),
            None => return WeatherResponse::GeolocationQueryFailed(Json(ErrorMessage { message: "Could not obtain remote address".to_owned() })),
        };

        let response = match self.http_client.get_coordinates_for_ip(&ip_string).await {
            Ok(r) => r,
            Err(e) => return WeatherResponse::GeolocationQueryFailed(Json(ErrorMessage { message: e.to_string() }))
        };

        let latitude = response.latitude;
        let longitude = response.longitude;

        let WeatherApiResponse { temperature, feels_like, condition, last_updated } = match self.http_client.get_weather_for_coordinates(latitude, longitude).await {
            Ok(r) => r,
            Err(e) => return WeatherResponse::WeatherQueryFailed(Json(ErrorMessage { message: e.to_string() }))
        };

        let response_body = WeatherResponseBody { temperature, feels_like, condition, last_updated };

        WeatherResponse::Success(Json(response_body))
    }
}

#[derive(serde::Serialize, Object)]
pub struct RegisterBody {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(serde::Serialize, Object)]
pub struct LoginBody {
    pub identifier: String,
    pub password: String,
}

#[derive(ApiResponse)]
pub enum HealthResponse {
    #[oai(status = 200)]
    Alive,
}

#[derive(ApiResponse)]
pub enum RegisterResponse {
    #[oai(status = 201)]
    Registered(Json<RegisterResponseBody>),
    #[oai(status = 409)]
    AlreadyRegistered,
    #[oai(status = 500)]
    RegistrationFailed,
}

#[derive(serde::Deserialize, Object)]
pub struct RegisterResponseBody {
    pub user_id: u64,
}

#[derive(ApiResponse)]
pub enum LoginResponse {
    #[oai(status = 200)]
    LoggedIn,
    #[oai(status = 404)]
    WrongCredentials,
}

#[derive(ApiResponse)]
pub enum WeatherResponse {
    #[oai(status = 200)]
    Success(Json<WeatherResponseBody>),
    #[oai(status = 500)]
    GeolocationQueryFailed(Json<ErrorMessage>),
    #[oai(status = 500)]
    WeatherQueryFailed(Json<ErrorMessage>),
}

#[derive(serde::Deserialize, Object)]
pub struct WeatherResponseBody {
    temperature: f64,
    feels_like: f64,
    condition: String,
    last_updated: String,
}

#[derive(Object)]
pub struct ErrorMessage {
    message: String,
}

impl Display for ErrorMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}