use crate::http_client::HttpClient;
use crate::queries::SqlError;
use crate::{check_token, create_token, hash_password, queries, validate_password};
use poem::web::RemoteAddr;
use poem_openapi::auth::Bearer;
use poem_openapi::payload::Json;
use poem_openapi::{ApiResponse, Object, OpenApi, SecurityScheme};
use sqlx::SqlitePool;
use std::net::SocketAddr;

#[cfg(feature = "integration-test")]
use {
    rand::Rng,
    std::net::{IpAddr, Ipv4Addr},
};

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
        let password_hash = match hash_password(&body.password) {
            Ok(h) => h,
            Err(_) => return RegisterResponse::RegistrationFailed,
        };

        let user_id = match queries::register_user(
            &self.database,
            &body.username,
            &body.email,
            &password_hash,
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
        let (user_id, password_hash) =
            queries::get_password(&self.database, &body.identifier, &body.identifier).await;

        let password_match = validate_password(body.password.clone(), password_hash).await;
        let Ok(token) = create_token(user_id) else {
            return LoginResponse::CouldNotCreateToken;
        };

        if password_match {
            LoginResponse::LoggedIn(Json(LoginResponseBody { token }))
        } else {
            LoginResponse::WrongCredentials
        }
    }

    #[oai(path = "/weather", method = "get")]
    pub async fn weather(
        &self,
        authorization: JwtAuthorization,
        ip: &RemoteAddr,
    ) -> WeatherResponse {
        if !check_token(&authorization.0.token) {
            return WeatherResponse::Unauthorized;
        }

        let ip_string = match ip.as_socket_addr() {
            Some(addr) => get_ip_string(addr),
            None => return WeatherResponse::GeolocationQueryFailed,
        };

        let Ok(response) = self.http_client.get_coordinates_for_ip(&ip_string).await else {
            return WeatherResponse::GeolocationQueryFailed;
        };

        let Ok(response) = self
            .http_client
            .get_weather_for_coordinates(response.latitude, response.longitude)
            .await
        else {
            return WeatherResponse::WeatherQueryFailed;
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

#[derive(SecurityScheme)]
#[oai(ty = "bearer")]
pub struct JwtAuthorization(Bearer);

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
    LoggedIn(Json<LoginResponseBody>),
    #[oai(status = 404)]
    WrongCredentials,
    #[oai(status = 500)]
    CouldNotCreateToken,
}

#[derive(serde::Deserialize, Object)]
pub struct LoginResponseBody {
    pub token: String,
}

#[derive(ApiResponse)]
pub enum WeatherResponse {
    #[oai(status = 200)]
    Success(Json<WeatherResponseBody>),
    #[oai(status = 500)]
    GeolocationQueryFailed,
    #[oai(status = 500)]
    WeatherQueryFailed,
    #[oai(status = 401)]
    Unauthorized,
}

#[derive(serde::Deserialize, Object)]
pub struct WeatherResponseBody {
    temperature: f64,
    feels_like: f64,
    condition: String,
    last_updated: String,
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

    if crate::is_loopback_address(&ip) {
        const IP_BLOCK_SIZE: u32 = 2_097_152;
        let range_start = [78u8, 160u8, 0u8, 0u8];
        let offset: [u8; 4] = rand::thread_rng().gen_range(0..IP_BLOCK_SIZE).to_be_bytes();

        ip = IpAddr::V4(Ipv4Addr::new(
            range_start[0] + offset[0],
            range_start[1] + offset[1],
            range_start[2] + offset[2],
            range_start[3] + offset[3],
        ));
    }

    ip.to_string()
}
