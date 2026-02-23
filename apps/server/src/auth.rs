use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{models::TelegramUser, AppState};

type HmacSha256 = Hmac<Sha256>;

/// Maximum age of initData before it's considered expired (24 hours).
const MAX_AUTH_AGE_SECS: i64 = 86400;

/// Validates Telegram Mini App initData and extracts user info.
/// See: https://core.telegram.org/bots/webapps#validating-data-received-via-the-mini-app
pub fn validate_init_data(init_data: &str, bot_token: &str) -> Option<TelegramUser> {
    let params: BTreeMap<String, String> = url::form_urlencoded::parse(init_data.as_bytes())
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let hash = params.get("hash")?;

    // Verify auth_date is recent (prevent replay attacks)
    if let Some(auth_date_str) = params.get("auth_date") {
        if let Ok(auth_date) = auth_date_str.parse::<i64>() {
            let now = chrono::Utc::now().timestamp();
            if (now - auth_date) > MAX_AUTH_AGE_SECS {
                tracing::warn!(
                    "initData expired: auth_date={}, age={}s",
                    auth_date,
                    now - auth_date
                );
                return None;
            }
        }
    }

    // Build data-check-string (sorted key=value pairs, excluding hash)
    let data_check_string: String = params
        .iter()
        .filter(|(k, _)| k.as_str() != "hash")
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    // secret_key = HMAC-SHA256("WebAppData", bot_token)
    let mut secret_mac =
        HmacSha256::new_from_slice(b"WebAppData").expect("HMAC can take key of any size");
    secret_mac.update(bot_token.as_bytes());
    let secret_key = secret_mac.finalize().into_bytes();

    // computed_hash = HMAC-SHA256(secret_key, data_check_string)
    let mut mac =
        HmacSha256::new_from_slice(&secret_key).expect("HMAC can take key of any size");
    mac.update(data_check_string.as_bytes());
    let computed_hash = hex::encode(mac.finalize().into_bytes());

    if computed_hash != *hash {
        tracing::warn!("initData hash mismatch");
        return None;
    }

    // Parse user JSON
    let user_json = params.get("user")?;
    serde_json::from_str::<TelegramUser>(user_json).ok()
}

/// Extract Telegram user from the Authorization header.
/// Header format: `tma <initData>`
pub fn extract_user_from_header(auth_header: &str, bot_token: &str) -> Option<TelegramUser> {
    let init_data = auth_header.strip_prefix("tma ")?;
    validate_init_data(init_data, bot_token)
}

/// Axum middleware that validates Telegram auth on every request.
/// Stores TelegramUser in request extensions.
#[allow(dead_code)]
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let user = extract_user_from_header(auth_header, &state.bot_token)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

/// Check if the authenticated user is the admin.
pub fn is_admin(user: &TelegramUser, admin_tg_id: i64) -> bool {
    user.id == admin_tg_id
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_BOT_TOKEN: &str = "7777777777:AAFake_Test_Token_For_Unit_Tests";

    /// Build a valid initData string with correct HMAC-SHA256 signature.
    fn build_init_data(bot_token: &str, user_json: &str, auth_date: i64) -> String {
        let mut params = BTreeMap::new();
        params.insert("auth_date".to_string(), auth_date.to_string());
        params.insert("user".to_string(), user_json.to_string());

        let data_check_string: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let mut secret_mac =
            HmacSha256::new_from_slice(b"WebAppData").expect("HMAC");
        secret_mac.update(bot_token.as_bytes());
        let secret_key = secret_mac.finalize().into_bytes();

        let mut mac = HmacSha256::new_from_slice(&secret_key).expect("HMAC");
        mac.update(data_check_string.as_bytes());
        let hash = hex::encode(mac.finalize().into_bytes());

        let mut encoded = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in &params {
            encoded.append_pair(k, v);
        }
        encoded.append_pair("hash", &hash);
        encoded.finish()
    }

    fn make_user(id: i64, first_name: &str, username: Option<&str>) -> TelegramUser {
        TelegramUser {
            id,
            first_name: first_name.to_string(),
            last_name: None,
            username: username.map(|s| s.to_string()),
        }
    }

    fn fresh_auth_date() -> i64 {
        chrono::Utc::now().timestamp() - 60 // 1 minute ago
    }

    fn test_user_json() -> String {
        r#"{"id":12345,"first_name":"Тест","username":"testuser"}"#.to_string()
    }

    // ── validate_init_data ──

    #[test]
    fn test_validate_valid_init_data() {
        let init_data = build_init_data(TEST_BOT_TOKEN, &test_user_json(), fresh_auth_date());
        let user = validate_init_data(&init_data, TEST_BOT_TOKEN);
        assert!(user.is_some());
        let user = user.unwrap();
        assert_eq!(user.id, 12345);
        assert_eq!(user.first_name, "Тест");
        assert_eq!(user.username.as_deref(), Some("testuser"));
    }

    #[test]
    fn test_validate_wrong_token() {
        let init_data = build_init_data(TEST_BOT_TOKEN, &test_user_json(), fresh_auth_date());
        let user = validate_init_data(&init_data, "9999999999:AAWrong_Token");
        assert!(user.is_none());
    }

    #[test]
    fn test_validate_tampered_hash() {
        let mut init_data = build_init_data(TEST_BOT_TOKEN, &test_user_json(), fresh_auth_date());
        // Replace last character of the hash
        let last = init_data.pop().unwrap();
        let replacement = if last == 'a' { 'b' } else { 'a' };
        init_data.push(replacement);
        assert!(validate_init_data(&init_data, TEST_BOT_TOKEN).is_none());
    }

    #[test]
    fn test_validate_expired_auth_date() {
        let old_date = chrono::Utc::now().timestamp() - 90000; // >24h ago
        let init_data = build_init_data(TEST_BOT_TOKEN, &test_user_json(), old_date);
        assert!(validate_init_data(&init_data, TEST_BOT_TOKEN).is_none());
    }

    #[test]
    fn test_validate_barely_fresh() {
        // 1 second before expiry
        let date = chrono::Utc::now().timestamp() - (MAX_AUTH_AGE_SECS - 1);
        let init_data = build_init_data(TEST_BOT_TOKEN, &test_user_json(), date);
        assert!(validate_init_data(&init_data, TEST_BOT_TOKEN).is_some());
    }

    #[test]
    fn test_validate_missing_hash() {
        // Build init_data without hash
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("auth_date", &fresh_auth_date().to_string())
            .append_pair("user", &test_user_json())
            .finish();
        assert!(validate_init_data(&encoded, TEST_BOT_TOKEN).is_none());
    }

    #[test]
    fn test_validate_missing_user() {
        // Build with hash but no user param — hash will be valid for auth_date-only data
        let mut params = BTreeMap::new();
        params.insert("auth_date".to_string(), fresh_auth_date().to_string());

        let data_check_string: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let mut secret_mac = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        secret_mac.update(TEST_BOT_TOKEN.as_bytes());
        let secret_key = secret_mac.finalize().into_bytes();
        let mut mac = HmacSha256::new_from_slice(&secret_key).unwrap();
        mac.update(data_check_string.as_bytes());
        let hash = hex::encode(mac.finalize().into_bytes());

        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("auth_date", &fresh_auth_date().to_string())
            .append_pair("hash", &hash)
            .finish();

        assert!(validate_init_data(&encoded, TEST_BOT_TOKEN).is_none());
    }

    #[test]
    fn test_validate_invalid_user_json() {
        let init_data = build_init_data(TEST_BOT_TOKEN, "not json at all", fresh_auth_date());
        assert!(validate_init_data(&init_data, TEST_BOT_TOKEN).is_none());
    }

    #[test]
    fn test_validate_empty_string() {
        assert!(validate_init_data("", TEST_BOT_TOKEN).is_none());
    }

    // ── extract_user_from_header ──

    #[test]
    fn test_extract_valid_header() {
        let init_data = build_init_data(TEST_BOT_TOKEN, &test_user_json(), fresh_auth_date());
        let header = format!("tma {}", init_data);
        let user = extract_user_from_header(&header, TEST_BOT_TOKEN);
        assert!(user.is_some());
        assert_eq!(user.unwrap().id, 12345);
    }

    #[test]
    fn test_extract_wrong_prefix() {
        let init_data = build_init_data(TEST_BOT_TOKEN, &test_user_json(), fresh_auth_date());
        let header = format!("Bearer {}", init_data);
        assert!(extract_user_from_header(&header, TEST_BOT_TOKEN).is_none());
    }

    #[test]
    fn test_extract_no_prefix() {
        let init_data = build_init_data(TEST_BOT_TOKEN, &test_user_json(), fresh_auth_date());
        assert!(extract_user_from_header(&init_data, TEST_BOT_TOKEN).is_none());
    }

    #[test]
    fn test_extract_empty() {
        assert!(extract_user_from_header("", TEST_BOT_TOKEN).is_none());
    }

    #[test]
    fn test_extract_tma_only() {
        assert!(extract_user_from_header("tma ", TEST_BOT_TOKEN).is_none());
    }

    // ── is_admin ──

    #[test]
    fn test_is_admin_true() {
        let user = make_user(12345, "Admin", None);
        assert!(is_admin(&user, 12345));
    }

    #[test]
    fn test_is_admin_false() {
        let user = make_user(12345, "User", None);
        assert!(!is_admin(&user, 99999));
    }

    #[test]
    fn test_is_admin_zero() {
        let user = make_user(0, "Zero", None);
        assert!(is_admin(&user, 0));
    }
}
