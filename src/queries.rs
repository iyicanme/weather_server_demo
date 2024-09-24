use sqlx::error::ErrorKind;
use sqlx::{Executor, Row, SqlitePool};

/// Persists a user to the database.
/// 
/// Caller is responsible to hash the password correctly.
/// 
/// # Errors
/// Will return error if any database error occurs
pub async fn register_user(
    database: &SqlitePool,
    username: &str,
    email: &str,
    password: &str,
) -> Result<u64, SqlError> {
    let query = sqlx::query!(
        r#"
            INSERT INTO user (id, username, email, password)
            VALUES (NULL, $1, $2, $3)
            RETURNING id
        "#,
        username,
        email,
        password
    );

    let row = database.fetch_one(query).await.map_err(SqlError::from)?;
    let user_id = row.get::<u64, &str>("id");

    Ok(user_id)
}

/// Returns user ID and password of user matching the given username or email.
///
/// If no user matches, a user ID of 0 and a None in place of a password is returned.
/// This is so caller can use a placeholder password and continue password validation in the case
/// user does not exist.
///
/// # Errors
/// Will return error if any database error occurs
pub async fn get_user_id_and_password_by_username_or_email(
    database: &SqlitePool,
    username: &str,
    email: &str,
) -> (u64, Option<String>) {
    let query = sqlx::query!(
        r#"
            SELECT id, password
            FROM user
            WHERE username = ? OR email = ?
        "#,
        username,
        email
    );

    let Ok(row) = database.fetch_one(query).await.map_err(SqlError::from) else {
        return (0u64, None);
    };

    let id = row.get::<u64, &str>("id");
    let password = row.get::<String, &str>("password");

    (id, Some(password))
}

/// Error derived from `sqlx::Error`, that allows caller of register query function understand user
/// already exists.
#[derive(Debug)]
pub enum SqlError {
    UniqueConstraintViolation,
    Other, // Wrap sqlx::Error inside if more context is needed
}

impl From<sqlx::Error> for SqlError {
    fn from(value: sqlx::Error) -> Self {
        let Some(is_unique_violation) = value
            .as_database_error()
            .map(|e| e.kind() == ErrorKind::UniqueViolation)
        else {
            return Self::Other;
        };

        if is_unique_violation {
            Self::UniqueConstraintViolation
        } else {
            Self::Other
        }
    }
}
