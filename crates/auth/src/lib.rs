use avn_hub_core::{AppError, AppResult, Database};
use chrono::{Duration, Utc};
use uuid::Uuid;

pub fn hash_password(password: &str) -> AppResult<String> {
    use argon2::{
        password_hash::{PasswordHasher, SaltString},
        Argon2,
    };
    use rand::rngs::OsRng;

    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Other(format!("password hash failed: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Argon2,
    };

    let parsed = PasswordHash::new(hash)
        .map_err(|e| AppError::Other(format!("invalid password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub struct AuthService;

impl AuthService {
    pub fn is_configured(db: &Database) -> AppResult<bool> {
        Ok(db
            .get_setting("app_password_hash")?
            .map(|h| !h.is_empty())
            .unwrap_or(false))
    }

    pub fn set_password(db: &Database, password: &str) -> AppResult<()> {
        if password.len() < 4 {
            return Err(AppError::BadRequest(
                "Password must be at least 4 characters".into(),
            ));
        }
        let hash = hash_password(password)?;
        db.set_setting("app_password_hash", &hash)?;
        Ok(())
    }

    pub fn remove_password(db: &Database) -> AppResult<()> {
        db.delete_setting("app_password_hash")?;
        Ok(())
    }

    pub fn login(db: &Database, password: &str) -> AppResult<String> {
        let hash = db
            .get_setting("app_password_hash")?
            .ok_or(AppError::BadRequest(
                "App password is not configured".into(),
            ))?;
        if !verify_password(password, &hash)? {
            return Err(AppError::Unauthorized);
        }
        db.purge_expired_sessions()?;
        let token = Uuid::new_v4().to_string();
        let expires = (Utc::now() + Duration::days(30)).to_rfc3339();
        db.create_session(&token, &expires)?;
        Ok(token)
    }

    pub fn logout(db: &Database, token: &str) -> AppResult<()> {
        db.delete_session(token)?;
        Ok(())
    }

    pub fn validate(db: &Database, token: &str) -> AppResult<bool> {
        if !Self::is_configured(db)? {
            return Ok(true);
        }
        db.session_valid(token)
    }

    /// When no password is set, auth is open. When set, require a valid token.
    pub fn require(db: &Database, token: Option<&str>) -> AppResult<()> {
        if !Self::is_configured(db)? {
            return Ok(());
        }
        let Some(token) = token.filter(|t| !t.is_empty()) else {
            return Err(AppError::Unauthorized);
        };
        if db.session_valid(token)? {
            Ok(())
        } else {
            Err(AppError::Unauthorized)
        }
    }
}
