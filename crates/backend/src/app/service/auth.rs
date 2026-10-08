//! Auth use-cases: register (argon2), login (argon2 verify + jwt issue).

use crate::domain::entities::User;
use crate::domain::value_objects::{Email, PasswordHash, Role, new_id};

use crate::app::service::use_cases::AppServices;
use crate::port::{PasswordHasher, TokenIssuer};

#[derive(Debug)]
pub enum AuthError {
    EmailTaken,
    BadCredentials,
    Storage(String),
}

impl From<String> for AuthError {
    fn from(e: String) -> Self {
        AuthError::Storage(e)
    }
}

pub const DEFAULT_USER_ROLE: Role = Role::Employee;

pub async fn register(
    svc: &AppServices,
    hasher: &dyn PasswordHasher,
    email: &str,
    password: &str,
) -> Result<User, AuthError> {
    let email = Email::parse(email).map_err(|e| AuthError::Storage(e.to_string()))?;
    if svc.users.find_by_email(email.as_str()).await?.is_some() {
        return Err(AuthError::EmailTaken);
    }
    tracing::info!(email = email.as_str(), "registering user");
    let hashed = hasher.hash(password).await?;
    let user = User {
        id: new_id(),
        email,
        password: PasswordHash::from_hash(hashed),
        role: DEFAULT_USER_ROLE,
        created_at: chrono::Utc::now(),
    };
    svc.users.insert(&user).await?;
    Ok(user)
}

pub async fn login(
    svc: &AppServices,
    hasher: &dyn PasswordHasher,
    tokens: &dyn TokenIssuer,
    email: &str,
    password: &str,
) -> Result<(uuid::Uuid, String), AuthError> {
    let email = Email::parse(email).map_err(|e| AuthError::Storage(e.to_string()))?;
    let Some(user) = svc.users.find_by_email(email.as_str()).await? else {
        tracing::warn!(email = email.as_str(), "login failed: unknown email");
        return Err(AuthError::BadCredentials);
    };
    let ok = hasher.verify(password, user.password.as_str()).await?;
    if !ok {
        tracing::warn!(user_id = %user.id, "login failed: bad password");
        return Err(AuthError::BadCredentials);
    }
    tracing::info!(user_id = %user.id, "login ok");
    let token = tokens.issue(user.id, user.role).await?;
    Ok((user.id, token))
}
