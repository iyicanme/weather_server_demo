use std::sync::OnceLock;
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};

/// Static storage for JWT keys.
static JWT_KEYS: OnceLock<Keys> = OnceLock::new();

/// Creates a JWT token containing given user ID.
///
/// # Errors
/// Function returns error if JWT encryption fails
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
/// Checks if the given token is issued with this server's key.
pub fn check_token(token: &str) -> bool {
    jsonwebtoken::decode::<TokenBody>(token, &Keys::get().decoding, &Validation::default()).is_ok()
}

/// Represents the claim section of JWT token.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TokenBody {
    user_id: u64,
    #[serde(rename = "exp")]
    expiration: u64,
}

/// For static storage of JWT keys.
struct Keys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Keys {
    /// Returns the JWT keys if they are previously initialized, or initializes them.
    fn get() -> &'static Self {
        JWT_KEYS.get_or_init(|| {
            let secret = Self::read_secret();
            Self::new(&secret)
        })
    }

    /// Initializes JWT tokens from the secret.
    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }

    /// Reads JWT secret from environment files.
    ///
    /// # Panics
    /// Will panic if the environment variable `JWT_SECRET` is not set
    fn read_secret() -> Vec<u8> {
        std::env::var("JWT_SECRET")
            .expect("no JWT secret in environment variables, please define 'JWT_SECRET'")
            .into_bytes()
    }
}
