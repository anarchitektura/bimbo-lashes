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
