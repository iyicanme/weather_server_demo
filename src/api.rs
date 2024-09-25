use crate::authorization::{check_token, create_token};
use crate::http_client::HttpClient;
use crate::queries::SqlError;
use crate::{password, queries};
use poem::web::RemoteAddr;
use poem_openapi::auth::Bearer;
use poem_openapi::payload::Json;
use poem_openapi::{ApiResponse, Object, OpenApi, SecurityScheme};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::str::FromStr;
#[cfg(feature = "integration-test")]
use {
    rand::Rng,
    std::net::{IpAddr, Ipv4Addr},
};

/// Holds the state and defines the handlers of the API.
pub struct Api {
    /// HTTP client wrapping the foreing geolocation and the weather APIs.
    http_client: HttpClient,
    /// Database connection.
    database: SqlitePool,
}

impl Api {
    /// Creates an instance of the API with given HTTP client and the database connection.
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
    /// A handler that always returns success.
    ///
    /// Used to check if server is alive.
    ///
    /// # Returns
    /// `200 Success` on every call
    #[oai(path = "/health_check", method = "get")]
    pub async fn health_check(&self) -> HealthResponse {
        HealthResponse::Alive
    }

    /// Registers a user.
    ///
    /// Password is hashed with Argon2 before getting persisted.
    ///
    /// # Returns
    /// `201 Created` with the created user's ID on success.
    ///
    /// `409 Conflict` if user already exists.
    ///
    /// `500 Internal Server Error` if the database operation fails.
    #[oai(path = "/register", method = "post")]
    pub async fn register(&self, body: Json<RegisterBody>) -> RegisterResponse {
        let credentials = match RegisterCredentials::try_from(body.0) {
            Ok(c) => c,
            Err(e) => return RegisterResponse::InvalidCredentials(
                ResponseMessage::new(&format!("Invalid credentials: {e}")).into_json()
            ),
        };
        
        let password_hash = password::hash(&credentials.password);
        let user_id = match queries::register_user(
            &self.database,
            &credentials.username,
            &credentials.email,
            &password_hash,
        )
        .await
        {
            Ok(i) => i,
            Err(SqlError::UniqueConstraintViolation) => return RegisterResponse::AlreadyRegistered(
                ResponseMessage::new("A user with given credentials already exists.")
                    .into_json()
            ),
            Err(SqlError::Other) => return RegisterResponse::RegistrationFailed(
                ResponseMessage::new("Registration failed . Try again.")
                    .into_json()
            ),
        };

        RegisterResponse::Registered(Json(RegisterResponseBody { user_id }))
    }

    /// Logs in the user with given credentials.
    /// User identifier can either be username or email.
    ///
    /// If the user does not exist with given identifier, the password is still hashed and
    /// compared against a placeholder hash as a measure against timing attacks.
    ///
    /// # Returns
    /// `200 Success` and a JWT token if passwords match.
    ///
    /// `404 Not Found` if such user does not exist or password do not match.
    ///
    /// `500 Internal Server Error` if JWT token creation fails.
    #[oai(path = "/login", method = "post")]
    pub async fn login(&self, body: Json<LoginBody>) -> LoginResponse {
        let (user_id, password_hash) =
            queries::get_user_id_and_password_by_username_or_email(&self.database, &body.identifier, &body.identifier).await;

        let password_match = password::validate(body.password.clone(), password_hash).await;
        let Ok(token) = create_token(user_id) else {
            return LoginResponse::CouldNotCreateToken(
                ResponseMessage::new("Login failed.").into_json()
            );
        };

        if password_match {
            LoginResponse::LoggedIn(Json(LoginResponseBody { token }))
        } else {
            LoginResponse::WrongCredentials(
                ResponseMessage::new("Username/email or password is wrong.").into_json()
            )
        }
    }

    /// Returns weather information for the caller.
    /// Location of the user is determined with their IP address.
    ///
    /// An HTTP call to a geolocation API with the caller's IP is made to get their coordinates.
    /// Then the weather information for that coordinate is obtained
    /// with an HTTP call to a weather API.
    ///
    /// Requires a valid JWT token.
    ///
    /// # Returns
    /// `200 Success` with the weather information on success.
    ///
    /// `401 Unauthorized` if no JWT token is attached or attached token is invalid.
    ///
    /// `500 Internal Server Error` if the call to foreign APIs fail.
    #[oai(path = "/weather", method = "get")]
    pub async fn weather(
        &self,
        authorization: JwtAuthorization,
        ip: &RemoteAddr,
    ) -> WeatherResponse {
        if !check_token(&authorization.0.token) {
            return WeatherResponse::Unauthorized(
                ResponseMessage::new("Unauthorized access.").into_json()
            );
        }

        let ip_string = match ip.as_socket_addr() {
            Some(addr) => get_ip_string(addr),
            None => return WeatherResponse::GeolocationQueryFailed(
                ResponseMessage::new("Could not fetch user IP.").into_json()
            ),
        };

        let Ok(response) = self.http_client.get_coordinates_for_ip(&ip_string).await else {
            return WeatherResponse::GeolocationQueryFailed(
                ResponseMessage::new("Could not fetch user location.").into_json()
            );
        };

        let Ok(response) = self
            .http_client
            .get_weather_for_coordinates(response.latitude, response.longitude)
            .await
        else {
            return WeatherResponse::WeatherQueryFailed(
                ResponseMessage::new("Could not fetch weather information.").into_json()
            );
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

/// Information used in `register` request body.
#[derive(serde::Serialize, Object)]
pub struct RegisterBody {
    /// User's username. Has to be unique.
    pub username: String,
    /// User's email. Has to be unique.
    pub email: String,
    /// User's password.
    pub password: String,
}

struct RegisterCredentials {
    /// User's username. Has to be unique.
    username: String,
    /// User's email. Has to be unique.
    email: String,
    /// User's password.
    password: String,
}

impl TryFrom<RegisterBody> for RegisterCredentials {
    type Error = String;

    fn try_from(RegisterBody { username, email, password }: RegisterBody) -> Result<Self, Self::Error> {
        if !(6usize..=24usize).contains(&username.len()) {
            let error_message = "Username needs to be at least 6 and at most 24 characters".to_owned();
            return Err(error_message);
        }

        if username.chars().any(|c| !c.is_alphanumeric() && !['.', '_'].contains(&c)) {
            let error_message = "Username can only contain letters, numbers, dots and underscores".to_owned();
            return Err(error_message);
        }

        let Ok(email) = email_address::EmailAddress::from_str(&email) else {
            let error_message = "Username can only contain letters, numbers, dots and underscores".to_owned();
            return Err(error_message);
        };

        if !(8usize..=32usize).contains(&password.len()) {
            let error_message = "Password needs to be at least 8 and at most 32 characters".to_owned();
            return Err(error_message);
        }

        let allowed_chars = "~!@$%^&*()_-+={[}]|:',.?/";
        if password.chars().any(|c| !c.is_alphanumeric() && !allowed_chars.chars().any(|symbol| symbol.eq(&c))) {
            let error_message = format!("Username can only contain letters, numbers and symbols {allowed_chars}");
            return Err(error_message);
        }

        let credentials = RegisterCredentials { username, email: email.email(), password };

        Ok(credentials)
    }
}

/// Information used in `login` request body.
#[derive(serde::Serialize, Object)]
pub struct LoginBody {
    /// Can either be user's `username` or `email`.
    pub identifier: String,
    /// User's password
    pub password: String,
}

/// Describes authorization used in `weather` request.
#[derive(SecurityScheme)]
#[oai(ty = "bearer")]
pub struct JwtAuthorization(Bearer);

/// Response of `health_check` call.
#[derive(ApiResponse)]
pub enum HealthResponse {
    /// Returned on all calls
    #[oai(status = 200)]
    Alive,
}

/// Response of `register` call.
#[derive(ApiResponse)]
pub enum RegisterResponse {
    /// Returned when registration succeeds.
    #[oai(status = 201)]
    Registered(Json<RegisterResponseBody>),
    /// Returned when registration credentials are not valid.
    #[oai(status = 400)]
    InvalidCredentials(ResponseBody),
    /// Returned when user with same credentials exists.
    #[oai(status = 409)]
    AlreadyRegistered(ResponseBody),
    /// Returned when persisting the user fails.
    #[oai(status = 500)]
    RegistrationFailed(ResponseBody),
}

/// Body of `register` call success response.
#[derive(serde::Deserialize, Object)]
pub struct RegisterResponseBody {
    /// ID of registered user.
    pub user_id: u64,
}

/// Response of `login` call.
#[derive(ApiResponse)]
pub enum LoginResponse {
    /// Returned when user successfully logs in.
    #[oai(status = 200)]
    LoggedIn(Json<LoginResponseBody>),
    /// Returned when such user does not exist or password does not match.
    #[oai(status = 404)]
    WrongCredentials(ResponseBody),
    /// Returned when JWT token creation fails.
    #[oai(status = 500)]
    CouldNotCreateToken(ResponseBody),
}

/// Body of `login` call success response.
#[derive(serde::Deserialize, Object)]
pub struct LoginResponseBody {
    /// Created JWT token.
    pub token: String,
}

/// Response of `weather` call.
#[derive(ApiResponse)]
pub enum WeatherResponse {
    /// Returned when weather information is successfully obtained.
    #[oai(status = 200)]
    Success(Json<WeatherResponseBody>),
    /// Returned when no token is provided or provided token is invalid.
    #[oai(status = 401)]
    Unauthorized(ResponseBody),
    /// Returned when call to geolocation API fails.
    #[oai(status = 500)]
    GeolocationQueryFailed(ResponseBody),
    /// Returned when call to weather API fails.
    #[oai(status = 500)]
    WeatherQueryFailed(ResponseBody),
}

/// Body of `weather` call success response.
#[derive(serde::Deserialize, Object)]
pub struct WeatherResponseBody {
    temperature: f64,
    feels_like: f64,
    condition: String,
    last_updated: String,
}

/// A response body serializable to JSON by poem-openapi
pub type ResponseBody = Json<ResponseMessage>;

/// A response body that used to return generic messages to the caller, usually errors
#[derive(Object)]
pub struct ResponseMessage {
    message: String,
}

impl ResponseMessage {
    /// Creates error message from given string
    fn new(message: &str) -> Self {
        Self {
            message: message.to_owned()
        }
    }

    /// Converts message into a poem-openapi JSON serializable type
    const fn into_json(self) -> ResponseBody {
        Json(self)
    }
}

/// Returns IP string for given `SocketAddr`.
///
/// Only exist so it can be overridden in tests with a version that returns a random IP string
/// from a range that does not belong to local network.
#[cfg(not(feature = "integration-test"))]
fn get_ip_string(address: &SocketAddr) -> String {
    address.ip().to_string()
}

/// In tests, clients are always local, so IP address is always loopback
/// The API we are using does not like that, so we make up an IP
#[cfg(feature = "integration-test")]
fn get_ip_string(address: &SocketAddr) -> String {
    let mut ip = address.ip();

    if crate::helpers::is_loopback_address(&ip) {
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
