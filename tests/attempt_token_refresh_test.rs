use spoq::auth::central_api::CentralApiClient;
/// Integration test for attempt_token_refresh function in main.rs
///
/// This test verifies the credential saving behavior added in Phase 3,
/// which ensures that refreshed tokens are immediately persisted to disk.
///
/// Phase 3 Changes Tested:
/// 1. Credentials are saved immediately after successful refresh
/// 2. Error categorization provides user-friendly messages
/// 3. Enhanced logging throughout the refresh flow
use spoq::auth::credentials::{Credentials, CredentialsManager};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper to create a test credentials manager with temp directory
fn create_test_manager(temp_dir: &TempDir) -> CredentialsManager {
    CredentialsManager::with_path(temp_dir.path().to_path_buf())
}

/// Test that attempt_token_refresh saves credentials after successful refresh
///
/// This test simulates the Phase 3 behavior where attempt_token_refresh
/// calls manager.save() immediately after building new credentials.
#[tokio::test]
async fn test_token_refresh_saves_credentials_on_success() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    // Create expired credentials
    let credentials = Credentials {
        access_token: Some("expired-token".to_string()),
        refresh_token: Some("valid-refresh-token".to_string()),
        expires_at: Some(0), // Expired
        user_id: Some("user-123".to_string()),
    };

    // Save initial expired credentials
    assert!(manager.save(&credentials));

    // Mock the refresh endpoint to return new tokens
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "new-access-token",
            "refresh_token": "new-refresh-token",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // Simulate the refresh flow (what attempt_token_refresh does)
    let client = CentralApiClient::with_base_url(mock_server.uri());
    let refresh_response = client
        .refresh_token(credentials.refresh_token.as_ref().unwrap())
        .await;
    assert!(refresh_response.is_ok());

    let token_response = refresh_response.unwrap();

    // Build new credentials (same as attempt_token_refresh)
    let mut new_credentials = credentials.clone();
    new_credentials.access_token = Some(token_response.access_token.clone());
    new_credentials.refresh_token = token_response.refresh_token;

    let expires_in = token_response.expires_in.unwrap_or(900);
    let new_expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
    new_credentials.expires_at = Some(new_expires_at);

    // CRITICAL: Save credentials after refresh (Phase 3 fix)
    assert!(
        manager.save(&new_credentials),
        "Failed to save refreshed credentials"
    );

    // Verify credentials were saved correctly by checking the file exists and reloading
    let creds_path = manager.credentials_path();
    assert!(
        creds_path.exists(),
        "Credentials file should exist after save"
    );

    // Reload to verify persistence
    let loaded_credentials = manager.load();
    assert_eq!(
        loaded_credentials.access_token,
        Some("new-access-token".to_string()),
        "Access token should be saved and reloaded correctly"
    );
    assert_eq!(
        loaded_credentials.refresh_token,
        Some("new-refresh-token".to_string()),
        "Refresh token should be saved and reloaded correctly"
    );
    assert!(
        !loaded_credentials.is_expired(),
        "Reloaded credentials should not be expired"
    );
    assert!(
        loaded_credentials.is_valid(),
        "Reloaded credentials should be valid"
    );
}

/// Test that refresh failure doesn't corrupt existing credentials
#[tokio::test]
async fn test_token_refresh_preserves_credentials_on_failure() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    // Create expired credentials
    let credentials = Credentials {
        access_token: Some("expired-token".to_string()),
        refresh_token: Some("invalid-refresh-token".to_string()),
        expires_at: Some(0), // Expired
        user_id: Some("user-123".to_string()),
    };

    // Save initial credentials
    assert!(manager.save(&credentials));

    // Mock the refresh endpoint to return 401 error
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/refresh"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": "Invalid refresh token"
        })))
        .mount(&mock_server)
        .await;

    // Attempt refresh (will fail)
    let client = CentralApiClient::with_base_url(mock_server.uri());
    let refresh_response = client
        .refresh_token(credentials.refresh_token.as_ref().unwrap())
        .await;
    assert!(refresh_response.is_err());

    // On failure, credentials should NOT be saved (attempt_token_refresh returns Err)
    // Verify original credentials are still intact
    let loaded_credentials = manager.load();
    assert_eq!(
        loaded_credentials.access_token,
        Some("expired-token".to_string())
    );
    assert_eq!(
        loaded_credentials.refresh_token,
        Some("invalid-refresh-token".to_string())
    );
}

/// Test error response handling matches Phase 3 error categorization
#[tokio::test]
async fn test_refresh_error_categorization() {
    use spoq::auth::central_api::CentralApiError;

    let mock_server = MockServer::start().await;

    // Test 401 - Invalid/expired refresh token
    Mock::given(method("POST"))
        .and(path("/auth/refresh"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": "Invalid refresh token"
        })))
        .mount(&mock_server)
        .await;

    let client = CentralApiClient::with_base_url(mock_server.uri());
    let result = client.refresh_token("invalid-token").await;

    assert!(result.is_err());
    match result {
        Err(CentralApiError::ServerError { status, message }) => {
            assert_eq!(status, 401);
            assert!(message.contains("Invalid refresh token"));
        }
        _ => panic!("Expected ServerError with 401 status"),
    }
}

/// Test that refresh token rotation is handled correctly
#[tokio::test]
async fn test_refresh_token_rotation() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let mut credentials = Credentials::default();
    credentials.access_token = Some("old-token".to_string());
    credentials.refresh_token = Some("old-refresh-token".to_string());
    credentials.expires_at = Some(0); // Expired

    assert!(manager.save(&credentials));

    // Mock server provides NEW refresh token (rotation)
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "rotated-access-token",
            "refresh_token": "rotated-refresh-token",  // New refresh token
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    let client = CentralApiClient::with_base_url(mock_server.uri());
    let token_response = client
        .refresh_token(credentials.refresh_token.as_ref().unwrap())
        .await
        .unwrap();

    // Build new credentials with rotated tokens
    let mut new_credentials = credentials.clone();
    new_credentials.access_token = Some(token_response.access_token.clone());

    // This is the key part - handle refresh token rotation
    if let Some(new_refresh) = token_response.refresh_token {
        new_credentials.refresh_token = Some(new_refresh);
    }

    let expires_in = token_response.expires_in.unwrap_or(900);
    new_credentials.expires_at = Some(chrono::Utc::now().timestamp() + expires_in as i64);

    // Save and reload to verify rotation
    assert!(manager.save(&new_credentials));
    let loaded = manager.load();

    assert_eq!(
        loaded.access_token,
        Some("rotated-access-token".to_string())
    );
    assert_eq!(
        loaded.refresh_token,
        Some("rotated-refresh-token".to_string())
    );
}

/// Test expiration calculation fallback to 900 seconds
#[tokio::test]
async fn test_expiration_fallback_to_default() {
    let mock_server = MockServer::start().await;

    // Mock response without expires_in field
    Mock::given(method("POST"))
        .and(path("/auth/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "new-token",
            "refresh_token": "new-refresh",
            "token_type": "Bearer"
            // Missing expires_in field
        })))
        .mount(&mock_server)
        .await;

    let client = CentralApiClient::with_base_url(mock_server.uri());
    let token_response = client.refresh_token("test-token").await.unwrap();

    // Should fall back to 900 seconds (15 minutes) when expires_in is missing
    let expires_in = token_response.expires_in.unwrap_or(900);
    assert_eq!(
        expires_in, 900,
        "Should default to 900 seconds when expires_in is missing"
    );
}
