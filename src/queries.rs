use sqlx::error::ErrorKind;
use sqlx::{Executor, Row, SqlitePool};

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

pub async fn get_password(
    database: &SqlitePool,
    username: &str,
    email: &str,
) -> Result<String, SqlError> {
    let query = sqlx::query!(
        r#"
            SELECT password
            FROM user
            WHERE username = ? OR email = ?
        "#,
        username,
        email
    );

    let row = database.fetch_one(query).await.map_err(SqlError::from)?;
    let password = row.get::<String, &str>("password");

    Ok(password)
}

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
