use crate::http_client::HttpClient;
use crate::queries::SqlError;
use poem::web::RemoteAddr;
use poem_openapi::payload::Json;
use poem_openapi::{ApiResponse, Object, OpenApi};
use rand::{thread_rng, Rng};
use sqlx::SqlitePool;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

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
        let user_id = match queries::register_user(
            &self.database,
            &body.username,
            &body.email,
            &body.password,
        )
        .await
        {
            Ok(i) => i,
            Err(SqlError::UniqueConstraintViolation) => return RegisterResponse::AlreadyRegistered,
            Err(SqlError::Other) => return RegisterResponse::RegistrationFailed,
        };

        RegisterResponse::Registered(Json(RegisterResponseBody { user_id }))
    }

    #[oai(path = "/login", method = "post")]
    pub async fn login(&self, body: Json<LoginBody>) -> LoginResponse {
        let password = queries::get_password(&self.database, &body.identifier, &body.identifier)
            .await
            .unwrap_or_else(|_| String::new());

        if password == body.password {
            LoginResponse::LoggedIn
        } else {
            LoginResponse::WrongCredentials
        }
    }

    #[oai(path = "/weather", method = "get")]
    pub async fn weather(&self, ip: &RemoteAddr) -> WeatherResponse {
        let ip_string = match ip.as_socket_addr() {
            Some(addr) => get_ip_string(addr),
            None => {
                return WeatherResponse::GeolocationQueryFailed(Json(ErrorMessage {
                    message: "Could not obtain remote address".to_owned(),
                }))
            }
        };

        let response = match self.http_client.get_coordinates_for_ip(&ip_string).await {
            Ok(r) => r,
            Err(e) => {
                return WeatherResponse::GeolocationQueryFailed(Json(ErrorMessage {
                    message: e.to_string(),
                }))
            }
        };

        let response = match self
            .http_client
            .get_weather_for_coordinates(response.latitude, response.longitude)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return WeatherResponse::WeatherQueryFailed(Json(ErrorMessage {
                    message: e.to_string(),
                }))
            }
        };

        let response_body = WeatherResponseBody {
            temperature: response.current.temp_c,
            feels_like: response.current.feelslike_c,
            condition: response.current.condition.text,
            last_updated: response.current.last_updated,
        };

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

#[cfg(not(feature = "integration-test"))]
fn get_ip_string(address: &SocketAddr) -> String {
    address.ip().to_string()
}

// In tests, clients are always local, so IP address is always loopback
// The API we are using does not like that, so we make up an IP
#[cfg(feature = "integration-test")]
fn get_ip_string(address: &SocketAddr) -> String {
    let mut ip = address.ip();

    if is_loopback_address(&ip) {
        const IP_BLOCK_SIZE: u32 = 2_097_152;
        let range_start = [78u8, 160u8, 0u8, 0u8];
        let offset: [u8; 4] = thread_rng().gen_range(0..IP_BLOCK_SIZE).to_be_bytes();

        ip = IpAddr::V4(Ipv4Addr::new(
            range_start[0] + offset[0],
            range_start[1] + offset[1],
            range_start[2] + offset[2],
            range_start[3] + offset[3],
        ));
    }

    ip.to_string()
}

pub fn is_loopback_address(ip: &IpAddr) -> bool {
    let loopback_v4 = IpAddr::from_str("127.0.0.1").expect("should be valid IPv4 loopback address");
    let loopback_v6 = IpAddr::from_str("::1").expect("should be valid IPv6 loopback address");

    ip.eq(&loopback_v4) || ip.eq(&loopback_v6)
}
