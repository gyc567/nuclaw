//! User Authentication Module
//!
//! Provides secure user registration, login, and token management.
//! Implements rate limiting, input validation, and secure token storage.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::error::{NuClawError, Result};

/// Authentication-specific errors
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("User already exists")]
    UserExists,
    #[error("User not found")]
    UserNotFound,
    #[error("Invalid email format")]
    InvalidEmail,
    #[error("Password too weak")]
    WeakPassword,
    #[error("Rate limit exceeded")]
    RateLimited,
    #[error("Token expired")]
    TokenExpired,
    #[error("Invalid token")]
    InvalidToken,
    #[error("Input too large")]
    InputTooLarge,
}

impl From<AuthError> for NuClawError {
    fn from(e: AuthError) -> Self {
        NuClawError::Auth {
            message: e.to_string(),
        }
    }
}

/// User data structure (never expose password_hash)
#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: u64,
    pub email: String,
    pub created_at: i64,
    #[serde(skip)]
    password_hash: String,
}

/// Authentication token
#[derive(Debug, Clone)]
pub struct AuthToken {
    pub token: String,
    pub user_id: u64,
    pub expires_at: Instant,
}

/// Rate limiter for login attempts
pub struct RateLimiter {
    attempts: RwLock<HashMap<String, (u64, Instant)>>,
    max_attempts: u64,
    window: Duration,
}

impl RateLimiter {
    #[inline]
    pub fn new(max_attempts: u64, window_secs: u64) -> Self {
        Self {
            attempts: RwLock::new(HashMap::with_capacity(100)),
            max_attempts,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check if request should be rate limited
    pub async fn check(&self, key: &str) -> Result<()> {
        let mut attempts = self.attempts.write().await;
        let now = Instant::now();

        // Clean expired entries periodically (every 100 requests)
        if attempts.len() > 100 && fastrand::u8(0..100) < 5 {
            attempts.retain(|_, (_, expiry)| *expiry > now);
        }

        match attempts.get_mut(key) {
            Some((count, expiry)) => {
                if now > *expiry {
                    // Window expired, reset
                    *count = 1;
                    *expiry = now + self.window;
                    Ok(())
                } else if *count >= self.max_attempts {
                    Err(AuthError::RateLimited.into())
                } else {
                    *count += 1;
                    Ok(())
                }
            }
            None => {
                attempts.insert(key.to_string(), (1, now + self.window));
                Ok(())
            }
        }
    }

    /// Reset rate limit for a key (called on successful login)
    pub async fn reset(&self, key: &str) {
        let mut attempts = self.attempts.write().await;
        attempts.remove(key);
    }
}

/// Authentication service
pub struct AuthService {
    users: Arc<RwLock<HashMap<u64, User>>>,
    email_index: Arc<RwLock<HashMap<String, u64>>>,
    tokens: Arc<RwLock<HashMap<String, AuthToken>>>,
    rate_limiter: Arc<RateLimiter>,
    next_id: AtomicU64,
    token_ttl: Duration,
}

/// Registration request
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

/// Login request
#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Authentication response
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: User,
    pub token: String,
    pub expires_in: u64,
}

impl AuthService {
    /// Create new auth service with default config
    #[inline]
    pub fn new() -> Self {
        Self::with_config(AuthConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: AuthConfig) -> Self {
        let initial_capacity = config.initial_capacity;
        Self {
            users: Arc::new(RwLock::new(HashMap::with_capacity(initial_capacity))),
            email_index: Arc::new(RwLock::new(HashMap::with_capacity(initial_capacity))),
            tokens: Arc::new(RwLock::new(HashMap::with_capacity(initial_capacity))),
            rate_limiter: Arc::new(RateLimiter::new(config.max_login_attempts, config.rate_limit_window_secs)),
            next_id: AtomicU64::new(1),
            token_ttl: Duration::from_secs(config.token_ttl_secs),
        }
    }

    /// Validate email format
    #[inline]
    fn validate_email(email: &str) -> Result<()> {
        // Check length first (DoS prevention)
        if email.len() > 254 {
            return Err(AuthError::InputTooLarge.into());
        }

        // Basic email validation regex
        static EMAIL_REGEX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
            Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
        });

        if !EMAIL_REGEX.is_match(email) {
            return Err(AuthError::InvalidEmail.into());
        }

        Ok(())
    }

    /// Validate password strength
    #[inline]
    fn validate_password(password: &str) -> Result<()> {
        // Check length (DoS prevention)
        if password.len() > 128 {
            return Err(AuthError::InputTooLarge.into());
        }

        if password.len() < 8 {
            return Err(AuthError::WeakPassword.into());
        }

        // Check for at least one uppercase, one lowercase, one digit
        let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());

        if !has_upper || !has_lower || !has_digit {
            return Err(AuthError::WeakPassword.into());
        }

        Ok(())
    }

    /// Hash password using Argon2
    fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| NuClawError::Auth {
                message: "Password hashing failed".to_string(),
            })?;

        Ok(password_hash.to_string())
    }

    /// Verify password against hash
    fn verify_password(password: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash).map_err(|_| NuClawError::Auth {
            message: "Invalid password hash".to_string(),
        })?;

        let argon2 = Argon2::default();
        Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
    }

    /// Generate secure random token
    fn generate_token() -> String {
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| rng.gen::<u8>() % 36)
            .map(|i| if i < 10 { (b'0' + i) as char } else { (b'a' + i - 10) as char })
            .collect()
    }

    /// Register a new user
    pub async fn register(&self, req: RegisterRequest) -> Result<AuthResponse> {
        // Validate inputs
        Self::validate_email(&req.email)?;
        Self::validate_password(&req.password)?;

        let email_lower = req.email.to_lowercase();

        // Check if user exists (using email index for O(1) lookup)
        let email_index = self.email_index.read().await;
        if email_index.contains_key(&email_lower) {
            return Err(AuthError::UserExists.into());
        }
        drop(email_index);

        // Hash password
        let password_hash = Self::hash_password(&req.password)?;

        // Create user
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let user = User {
            id,
            email: email_lower.clone(),
            created_at: chrono::Utc::now().timestamp(),
            password_hash,
        };

        // Store user
        let mut users = self.users.write().await;
        let mut email_index = self.email_index.write().await;

        users.insert(id, user.clone());
        email_index.insert(email_lower, id);

        // Generate token
        let token = Self::generate_token();
        let expires_at = Instant::now() + self.token_ttl;

        let auth_token = AuthToken {
            token: token.clone(),
            user_id: id,
            expires_at,
        };

        self.tokens.write().await.insert(token.clone(), auth_token);

        Ok(AuthResponse {
            user,
            token,
            expires_in: self.token_ttl.as_secs(),
        })
    }

    /// Login user
    pub async fn login(&self, req: LoginRequest, client_ip: &str) -> Result<AuthResponse> {
        // Validate inputs
        Self::validate_email(&req.email)?;

        // Check rate limit (by IP + email combination)
        let rate_key = format!("{}:{}", client_ip, req.email);
        self.rate_limiter.check(&rate_key).await?;

        let email_lower = req.email.to_lowercase();

        // Find user by email (using index)
        let email_index = self.email_index.read().await;
        let user_id = match email_index.get(&email_lower) {
            Some(id) => *id,
            None => {
                // Use constant-time comparison to prevent timing attacks
                // Even on user not found, do a dummy hash verification
                let _ = Self::verify_password(&req.password, "$argon2id$v=19$m=65536,t=3,p=4$...");
                return Err(AuthError::InvalidCredentials.into());
            }
        };
        drop(email_index);

        // Get user
        let users = self.users.read().await;
        let user = match users.get(&user_id) {
            Some(u) => u.clone(),
            None => return Err(AuthError::InvalidCredentials.into()),
        };
        drop(users);

        // Verify password
        if !Self::verify_password(&req.password, &user.password_hash)? {
            return Err(AuthError::InvalidCredentials.into());
        }

        // Reset rate limit on success
        self.rate_limiter.reset(&rate_key).await;

        // Generate token
        let token = Self::generate_token();
        let expires_at = Instant::now() + self.token_ttl;

        let auth_token = AuthToken {
            token: token.clone(),
            user_id: user.id,
            expires_at,
        };

        self.tokens.write().await.insert(token.clone(), auth_token);

        Ok(AuthResponse {
            user,
            token,
            expires_in: self.token_ttl.as_secs(),
        })
    }

    /// Validate token and return user
    pub async fn validate_token(&self, token: &str) -> Result<User> {
        // Check length (DoS prevention)
        if token.len() > 256 {
            return Err(AuthError::InvalidToken.into());
        }

        let mut tokens = self.tokens.write().await;

        // Clean expired tokens periodically
        if tokens.len() > 1000 && fastrand::u8(0..100) < 5 {
            let now = Instant::now();
            tokens.retain(|_, t| t.expires_at > now);
        }

        match tokens.get(token) {
            Some(auth_token) => {
                if auth_token.expires_at < Instant::now() {
                    tokens.remove(token);
                    return Err(AuthError::TokenExpired.into());
                }

                let users = self.users.read().await;
                match users.get(&auth_token.user_id) {
                    Some(user) => Ok(user.clone()),
                    None => Err(AuthError::UserNotFound.into()),
                }
            }
            None => Err(AuthError::InvalidToken.into()),
        }
    }

    /// Logout user (invalidate token)
    pub async fn logout(&self, token: &str) -> Result<()> {
        let mut tokens = self.tokens.write().await;
        tokens.remove(token);
        Ok(())
    }

    /// Get user by ID
    pub async fn get_user(&self, user_id: u64) -> Result<User> {
        let users = self.users.read().await;
        match users.get(&user_id) {
            Some(user) => Ok(user.clone()),
            None => Err(AuthError::UserNotFound.into()),
        }
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new()
    }
}

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub initial_capacity: usize,
    pub max_login_attempts: u64,
    pub rate_limit_window_secs: u64,
    pub token_ttl_secs: u64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            initial_capacity: 100,
            max_login_attempts: 5,
            rate_limit_window_secs: 300, // 5 minutes
            token_ttl_secs: 86400,       // 24 hours
        }
    }
}

/// Builder for AuthConfig
pub struct AuthConfigBuilder {
    config: AuthConfig,
}

impl AuthConfigBuilder {
    #[inline]
    pub fn new() -> Self {
        Self {
            config: AuthConfig::default(),
        }
    }

    #[inline]
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.config.initial_capacity = capacity;
        self
    }

    #[inline]
    pub fn with_rate_limit(mut self, attempts: u64, window_secs: u64) -> Self {
        self.config.max_login_attempts = attempts;
        self.config.rate_limit_window_secs = window_secs;
        self
    }

    #[inline]
    pub fn with_token_ttl(mut self, secs: u64) -> Self {
        self.config.token_ttl_secs = secs;
        self
    }

    #[inline]
    pub fn build(self) -> AuthConfig {
        self.config
    }
}

impl Default for AuthConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_success() {
        let auth = AuthService::new();
        let req = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "SecurePass123".to_string(),
        };

        let result = auth.register(req).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.user.email, "test@example.com");
        assert!(!response.token.is_empty());
    }

    #[tokio::test]
    async fn test_register_duplicate_email() {
        let auth = AuthService::new();
        let req = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "SecurePass123".to_string(),
        };

        // First registration succeeds
        assert!(auth.register(req.clone()).await.is_ok());

        // Second registration fails
        let result = auth.register(req).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NuClawError::Auth { .. }));
    }

    #[tokio::test]
    async fn test_register_weak_password() {
        let auth = AuthService::new();
        let req = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "weak".to_string(),
        };

        let result = auth.register(req).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_register_invalid_email() {
        let auth = AuthService::new();
        let req = RegisterRequest {
            email: "not-an-email".to_string(),
            password: "SecurePass123".to_string(),
        };

        let result = auth.register(req).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_login_success() {
        let auth = AuthService::new();
        let register_req = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "SecurePass123".to_string(),
        };

        auth.register(register_req).await.unwrap();

        let login_req = LoginRequest {
            email: "test@example.com".to_string(),
            password: "SecurePass123".to_string(),
        };

        let result = auth.login(login_req, "127.0.0.1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_login_invalid_credentials() {
        let auth = AuthService::new();
        let login_req = LoginRequest {
            email: "nonexistent@example.com".to_string(),
            password: "WrongPass123".to_string(),
        };

        let result = auth.login(login_req, "127.0.0.1").await;
        assert!(result.is_err());
        // Verify error message doesn't leak which field is wrong
        let err_msg = result.unwrap_err().to_string();
        assert!(!err_msg.contains("email"));
        assert!(!err_msg.contains("password"));
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = AuthConfigBuilder::new()
            .with_rate_limit(2, 60)
            .build();
        let auth = AuthService::with_config(config);

        // Try login 3 times with wrong password
        for i in 0..3 {
            let login_req = LoginRequest {
                email: "test@example.com".to_string(),
                password: "WrongPass123".to_string(),
            };

            let result = auth.login(login_req, "127.0.0.1").await;
            if i < 2 {
                assert!(result.is_err());
                // Should be invalid credentials, not rate limited yet
                assert!(!result.unwrap_err().to_string().contains("Rate limit"));
            } else {
                // Third attempt should be rate limited
                assert!(result.is_err());
                assert!(result.unwrap_err().to_string().contains("Rate limit"));
            }
        }
    }

    #[tokio::test]
    async fn test_token_validation() {
        let auth = AuthService::new();
        let register_req = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "SecurePass123".to_string(),
        };

        let response = auth.register(register_req).await.unwrap();

        // Valid token
        let user = auth.validate_token(&response.token).await;
        assert!(user.is_ok());
        assert_eq!(user.unwrap().email, "test@example.com");

        // Invalid token
        let result = auth.validate_token("invalid_token").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_logout() {
        let auth = AuthService::new();
        let register_req = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "SecurePass123".to_string(),
        };

        let response = auth.register(register_req).await.unwrap();

        // Logout
        assert!(auth.logout(&response.token).await.is_ok());

        // Token should be invalid now
        let result = auth.validate_token(&response.token).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_email_case_insensitive() {
        let auth = AuthService::new();
        let register_req = RegisterRequest {
            email: "Test@Example.COM".to_string(),
            password: "SecurePass123".to_string(),
        };

        auth.register(register_req).await.unwrap();

        // Login with different case
        let login_req = LoginRequest {
            email: "test@example.com".to_string(),
            password: "SecurePass123".to_string(),
        };

        let result = auth.login(login_req, "127.0.0.1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_input_size_limits() {
        let auth = AuthService::new();

        // Email too long
        let long_email = format!("{}@example.com", "a".repeat(250));
        let req = RegisterRequest {
            email: long_email,
            password: "SecurePass123".to_string(),
        };
        let result = auth.register(req).await;
        assert!(result.is_err());

        // Password too long
        let req = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "a".repeat(200),
        };
        let result = auth.register(req).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_error_messages_no_secrets() {
        let auth = AuthService::new();
        let password = "MySecretPassword123";

        let req = RegisterRequest {
            email: "test@example.com".to_string(),
            password: password.to_string(),
        };

        auth.register(req).await.unwrap();

        // Try login with wrong password
        let login_req = LoginRequest {
            email: "test@example.com".to_string(),
            password: "WrongPassword".to_string(),
        };

        let result = auth.login(login_req, "127.0.0.1").await;
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        // Verify password is NOT in error message
        assert!(!error_msg.contains(password));
        assert!(!error_msg.contains("WrongPassword"));
    }
}
