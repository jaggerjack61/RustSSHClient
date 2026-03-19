use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AuthType {
    #[default]
    Password,
    Key,
}

impl fmt::Display for AuthType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password => write!(f, "Password"),
            Self::Key => write!(f, "SSH Key"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SaveLifetime {
    OneHour,
    OneDay,
    OneWeek,
    ThirtyDays,
    #[default]
    Forever,
}

impl SaveLifetime {
    pub const ALL: [Self; 5] = [
        Self::OneHour,
        Self::OneDay,
        Self::OneWeek,
        Self::ThirtyDays,
        Self::Forever,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::OneHour => "1 Hour",
            Self::OneDay => "1 Day",
            Self::OneWeek => "1 Week",
            Self::ThirtyDays => "30 Days",
            Self::Forever => "Forever",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::OneHour => "Use for temporary maintenance access and one-off jumps.",
            Self::OneDay => "Keep it through the workday, then let it self-prune.",
            Self::OneWeek => "A balanced default for active project environments.",
            Self::ThirtyDays => "Good for recurring infrastructure you revisit monthly.",
            Self::Forever => "Keep this credential until you explicitly delete it.",
        }
    }

    pub fn expiration_from(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::OneHour => Some(now + chrono::Duration::hours(1)),
            Self::OneDay => Some(now + chrono::Duration::days(1)),
            Self::OneWeek => Some(now + chrono::Duration::weeks(1)),
            Self::ThirtyDays => Some(now + chrono::Duration::days(30)),
            Self::Forever => None,
        }
    }
}

impl fmt::Display for SaveLifetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostSort {
    #[default]
    Label,
    Host,
    Recent,
}

impl fmt::Display for HostSort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Label => write!(f, "Label"),
            Self::Host => write!(f, "Host"),
            Self::Recent => write!(f, "Recent"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostRecord {
    pub id: Uuid,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    pub password: Option<String>,
    pub key_reference: Option<Uuid>,
    #[serde(default)]
    pub save_lifetime: SaveLifetime,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl fmt::Debug for HostRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostRecord")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("auth_type", &self.auth_type)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("key_reference", &self.key_reference)
            .field("save_lifetime", &self.save_lifetime)
            .field("expires_at", &self.expires_at)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl HostRecord {
    pub fn new(request: &LoginRequest) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            label: request.effective_label(),
            host: request.host.trim().to_string(),
            port: request.port,
            username: request.username.trim().to_string(),
            auth_type: request.auth_type,
            password: request.password.clone(),
            key_reference: request.key_reference,
            save_lifetime: request.save_lifetime,
            expires_at: request.save_lifetime.expiration_from(now),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn apply_request(&mut self, request: &LoginRequest) {
        self.label = request.effective_label();
        self.host = request.host.trim().to_string();
        self.port = request.port;
        self.username = request.username.trim().to_string();
        self.auth_type = request.auth_type;
        self.password = request.password.clone();
        self.key_reference = request.key_reference;
        self.save_lifetime = request.save_lifetime;
        self.updated_at = Utc::now();
        self.expires_at = request.save_lifetime.expiration_from(self.updated_at);
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.map(|expires_at| expires_at <= now).unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginRequest {
    pub label: Option<String>,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub auth_type: AuthType,
    pub key_reference: Option<Uuid>,
    pub save_host: bool,
    pub save_lifetime: SaveLifetime,
}

impl LoginRequest {
    pub fn validate(&self) -> AppResult<()> {
        if self.host.trim().is_empty() {
            return Err(AppError::Validation("Host is required.".into()));
        }

        if self.username.trim().is_empty() {
            return Err(AppError::Validation("Username is required.".into()));
        }

        if self.port == 0 {
            return Err(AppError::Validation("Port must be greater than 0.".into()));
        }

        match self.auth_type {
            AuthType::Password => {
                if self
                    .password
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
                {
                    return Err(AppError::Validation(
                        "Password is required for password authentication.".into(),
                    ));
                }
            }
            AuthType::Key => {
                if self.key_reference.is_none() {
                    return Err(AppError::Validation(
                        "Select an SSH key for key-based authentication.".into(),
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn effective_label(&self) -> String {
        let requested = self.label.as_deref().unwrap_or_default().trim();

        if requested.is_empty() {
            format!("{}@{}", self.username.trim(), self.host.trim())
        } else {
            requested.to_string()
        }
    }

    pub fn socket_address(&self) -> String {
        format!("{}:{}", self.host.trim(), self.port)
    }
}

impl Drop for LoginRequest {
    fn drop(&mut self) {
        if let Some(password) = &mut self.password {
            password.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::{AuthType, LoginRequest, SaveLifetime};

    #[test]
    fn validates_password_requests() {
        let request = LoginRequest {
            label: None,
            host: "example.com".into(),
            port: 22,
            username: "root".into(),
            password: Some("secret".into()),
            auth_type: AuthType::Password,
            key_reference: None,
            save_host: true,
            save_lifetime: SaveLifetime::Forever,
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn rejects_missing_key_authentication() {
        let request = LoginRequest {
            label: Some("prod".into()),
            host: "example.com".into(),
            port: 22,
            username: "deploy".into(),
            password: None,
            auth_type: AuthType::Key,
            key_reference: None,
            save_host: true,
            save_lifetime: SaveLifetime::Forever,
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn falls_back_to_derived_label() {
        let request = LoginRequest {
            label: Some("  ".into()),
            host: "box.local".into(),
            port: 22,
            username: "deploy".into(),
            password: Some("secret".into()),
            auth_type: AuthType::Password,
            key_reference: None,
            save_host: false,
            save_lifetime: SaveLifetime::Forever,
        };

        assert_eq!(request.effective_label(), "deploy@box.local");
    }

    #[test]
    fn host_records_can_expire() {
        let request = LoginRequest {
            label: Some("expiring".into()),
            host: "example.com".into(),
            port: 22,
            username: "root".into(),
            password: Some("secret".into()),
            auth_type: AuthType::Password,
            key_reference: None,
            save_host: true,
            save_lifetime: super::SaveLifetime::OneDay,
        };

        let mut host = super::HostRecord::new(&request);
        host.expires_at = Some(Utc::now() - Duration::hours(1));

        assert!(host.is_expired(Utc::now()));
    }
}
