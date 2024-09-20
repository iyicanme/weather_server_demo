use std::fmt::{Display, Formatter};

use poem::web::RemoteAddr;
use poem_openapi::{ApiResponse, Object, OpenApi};
use poem_openapi::payload::Json;

use crate::http_client::HttpClient;

pub mod config;
pub mod http_client;

pub struct Api {
    http_client: HttpClient,
}

impl Api {
    #[must_use]
    pub fn new(http_client: HttpClient) -> Self {
        Self {
            http_client,
        }
    }
}

#[OpenApi]
impl Api {
    #[oai(path = "/register", method = "post")]
    pub async fn register(&self, body: Json<RegisterBody>) -> RegisterResponse {
        RegisterResponse::UserCreated
    }

    #[oai(path = "/login", method = "post")]
    pub async fn login(&self, body: Json<LoginBody>) -> LoginResponse {
        LoginResponse::LoggedIn
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

        let response = match self.http_client.get_weather_for_coordinates(latitude, longitude).await {
            Ok(r) => r,
            Err(e) => return WeatherResponse::WeatherQueryFailed(Json(ErrorMessage { message: e.to_string() }))
        };

        let response_body = WeatherResponseBody { temperature: 0.0, feels_like: 0.0, condition: String::new(), last_updated: String::new() };

        WeatherResponse::Success(Json(response_body))
    }
}

#[derive(Object)]
pub struct RegisterBody {
    username: String,
    email: String,
    password: String,
}

#[derive(Object)]
pub struct LoginBody {}

#[derive(ApiResponse)]
pub enum RegisterResponse {
    #[oai(status = 201)]
    UserCreated,
    #[oai(status = 409)]
    AlreadyRegistered,
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

#[derive(Object)]
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