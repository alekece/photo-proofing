use std::time::Duration;

use strum::Display;

use crate::auth::session::SessionToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum SameSite {
    Lax,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieMode {
    Production,
    /// Plain-HTTP local development: drops `Secure` and the `__Host-` prefix,
    /// which browsers refuse over HTTP.
    Development,
}

#[derive(Debug, Clone, Copy)]
pub struct SessionCookie {
    pub mode: CookieMode,
    pub same_site: SameSite,
}

impl Default for SessionCookie {
    fn default() -> Self {
        Self {
            mode: CookieMode::Production,
            same_site: SameSite::Lax,
        }
    }
}

impl SessionCookie {
    pub fn name(&self) -> &'static str {
        match self.mode {
            CookieMode::Production => "__Host-session",
            CookieMode::Development => "session",
        }
    }

    /// `Set-Cookie` header value carrying the token. The token is base64url,
    /// so it needs no escaping.
    pub fn set_header(&self, token: &SessionToken, max_age: Duration) -> String {
        self.render(token.expose(), max_age)
    }

    /// `Set-Cookie` header value instructing the browser to drop the cookie.
    pub fn clear_header(&self) -> String {
        self.render("", Duration::ZERO)
    }

    fn render(&self, value: &str, max_age: Duration) -> String {
        let secure = match self.mode {
            CookieMode::Production => "; Secure",
            CookieMode::Development => "",
        };

        format!(
            "{}={value}; Path=/; Max-Age={}; HttpOnly{secure}; SameSite={}",
            self.name(),
            max_age.as_secs(),
            self.same_site,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token() -> SessionToken {
        SessionToken::generate()
    }

    #[test]
    fn production_cookie_has_all_hardening_attributes() {
        let token = token();
        let header = SessionCookie::default().set_header(&token, Duration::from_secs(3600));

        assert!(header.starts_with(&format!("__Host-session={}; ", token.expose())));
        for attribute in [
            "Path=/",
            "Max-Age=3600",
            "HttpOnly",
            "Secure",
            "SameSite=Lax",
        ] {
            assert!(header.contains(attribute), "missing {attribute}: {header}");
        }
        assert!(!header.contains("Domain"));
    }

    #[test]
    fn strict_same_site_is_supported() {
        let cookie = SessionCookie {
            same_site: SameSite::Strict,
            ..SessionCookie::default()
        };

        assert!(
            cookie
                .set_header(&token(), Duration::from_secs(1))
                .contains("SameSite=Strict")
        );
    }

    #[test]
    fn development_mode_relaxes_secure_and_prefix_only() {
        let cookie = SessionCookie {
            mode: CookieMode::Development,
            ..SessionCookie::default()
        };
        let header = cookie.set_header(&token(), Duration::from_secs(60));

        assert!(header.starts_with("session="));
        assert!(!header.contains("Secure"));
        assert!(header.contains("HttpOnly"));
        assert!(header.contains("SameSite=Lax"));
    }

    #[test]
    fn clear_header_expires_immediately() {
        let header = SessionCookie::default().clear_header();

        assert!(header.starts_with("__Host-session=; "));
        assert!(header.contains("Max-Age=0"));
        assert!(header.contains("Secure"));
    }
}
