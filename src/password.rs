use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use tokio::task::spawn_blocking;

/// Hashes the given password with Argon2id version `0x13`-`19` with parameters
/// `m_cost`=15000, `t_cost`=2, `p_cost`=1.
///
/// # Panics
/// `expect`s in the function should not cause any panics with possible inputs of the function.
#[must_use]
pub fn hash(password: &str) -> String {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let params = Params::new(15000, 2, 1, None).expect("provided parameters should not throw");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string()).expect("password hashing should not throw")
}

/// Checks if the given password matches with the hash.
///
/// Designed to do the hash computation regardless if the user was registered or not
/// as a measure against timing attacks.
pub async fn validate(password: String, hash: Option<String>) -> bool {
    let placeholder_hash = "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
        .to_string();
    let hash = hash.unwrap_or(placeholder_hash);

    compare(password, hash).await.is_ok()
}

/// Computes the hash for the password and compares against the hash.
///
/// # Errors
/// Returns error if spawning blocking task fails or password verification fails for any reason.
async fn compare(password: String, hash: String) -> Result<(), anyhow::Error> {
    spawn_blocking(move || {
        let hash = PasswordHash::new(&hash)?;
        Argon2::default().verify_password(password.as_bytes(), &hash)
    })
        .await??;

    Ok(())
}
