//! Tests for payment integration with Central API.
//!
//! NOTE: Credentials now only contain auth fields (access_token, refresh_token,
//! expires_at, user_id). Subscription state is managed server-side.
//!
//! These tests verify the payment-related API client methods including
//! checkout session creation, payment status polling, and subscription management.

use spoq::auth::central_api::{
    CentralApiClient, CheckoutSessionResponse, PaymentStatusResponse, SubscriptionStatus,
};

/// Test CheckoutSessionResponse deserialization
#[test]
fn test_checkout_session_response_deserialization() {
    let json = r#"{
        "checkout_url": "https://checkout.stripe.com/session123",
        "session_id": "cs_test_abc123",
        "customer_email": "test@example.com"
    }"#;

    let response: CheckoutSessionResponse = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(
        response.checkout_url,
        "https://checkout.stripe.com/session123"
    );
    assert_eq!(response.session_id, "cs_test_abc123");
    assert_eq!(response.customer_email, "test@example.com");
}

/// Test PaymentStatusResponse deserialization with completed payment
#[test]
fn test_payment_status_response_completed() {
    let json = r#"{
        "status": "complete",
        "subscription_id": "sub_1234567890",
        "customer_id": "cus_abc123"
    }"#;

    let response: PaymentStatusResponse = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(response.status, "complete");
    assert_eq!(response.subscription_id, Some("sub_1234567890".to_string()));
    assert_eq!(response.customer_id, Some("cus_abc123".to_string()));
}

/// Test PaymentStatusResponse deserialization with pending payment
#[test]
fn test_payment_status_response_pending() {
    let json = r#"{"status": "pending"}"#;

    let response: PaymentStatusResponse = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(response.status, "pending");
    assert_eq!(response.subscription_id, None);
    assert_eq!(response.customer_id, None);
}

/// Test PaymentStatusResponse with different status values
#[test]
fn test_payment_status_different_values() {
    let statuses = vec!["pending", "complete", "failed", "canceled"];

    for status in statuses {
        let json = format!(r#"{{"status": "{}"}}"#, status);
        let response: PaymentStatusResponse =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(response.status, status);

        // Only complete should have subscription_id in production
        if status == "complete" {
            // In this test, we're not providing subscription_id, so it should be None
            assert!(response.subscription_id.is_none());
        }
    }
}

/// Test SubscriptionStatus deserialization with active subscription
#[test]
fn test_subscription_status_active() {
    let json = r#"{
        "status": "active",
        "plan": "plan_enterprise",
        "current_period_end": "2026-12-31T23:59:59Z",
        "cancel_at_period_end": false,
        "customer_portal_url": "https://billing.stripe.com/portal123"
    }"#;

    let status: SubscriptionStatus = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(status.status, "active");
    assert_eq!(status.plan, Some("plan_enterprise".to_string()));
    assert_eq!(
        status.current_period_end,
        Some("2026-12-31T23:59:59Z".to_string())
    );
    assert_eq!(status.cancel_at_period_end, Some(false));
    assert!(status.customer_portal_url.is_some());
}

/// Test SubscriptionStatus deserialization with inactive subscription
#[test]
fn test_subscription_status_inactive() {
    let json = r#"{"status": "inactive"}"#;

    let status: SubscriptionStatus = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(status.status, "inactive");
    assert_eq!(status.plan, None);
    assert_eq!(status.current_period_end, None);
    assert_eq!(status.cancel_at_period_end, None);
    assert_eq!(status.customer_portal_url, None);
}

/// Test CentralApiClient can be created
#[test]
fn test_central_api_client_creation() {
    let api = CentralApiClient::new();
    // Just verify it was created successfully
    assert_eq!(api.base_url, "https://spoq.dev");
}

/// Test payment status polling workflow simulation
#[test]
fn test_payment_status_polling_workflow() {
    // Simulate first poll: pending
    let pending_json = r#"{"status": "pending"}"#;
    let pending_response: PaymentStatusResponse =
        serde_json::from_str(pending_json).expect("Should deserialize");

    assert_eq!(pending_response.status, "pending");
    assert!(pending_response.subscription_id.is_none());

    // Simulate second poll: complete with subscription
    let complete_json = r#"{
        "status": "complete",
        "subscription_id": "sub_success123",
        "customer_id": "cus_xyz789"
    }"#;
    let complete_response: PaymentStatusResponse =
        serde_json::from_str(complete_json).expect("Should deserialize");

    assert_eq!(complete_response.status, "complete");
    assert_eq!(
        complete_response.subscription_id,
        Some("sub_success123".to_string())
    );
}

/// Test subscription_id extraction from payment status
#[test]
fn test_subscription_id_extraction() {
    let json = r#"{
        "status": "complete",
        "subscription_id": "sub_extracted123"
    }"#;

    let response: PaymentStatusResponse = serde_json::from_str(json).expect("Should deserialize");

    if let Some(sub_id) = response.subscription_id {
        assert_eq!(sub_id, "sub_extracted123");
    } else {
        panic!("Expected subscription_id to be present");
    }
}

/// Test backward compatibility - PaymentStatusResponse without subscription_id
#[test]
fn test_payment_status_backward_compatibility() {
    let json = r#"{"status": "complete"}"#;

    let response: PaymentStatusResponse =
        serde_json::from_str(json).expect("Should deserialize old format");

    assert_eq!(response.status, "complete");
    assert_eq!(response.subscription_id, None); // Should default to None
    assert_eq!(response.customer_id, None); // Should default to None
}

/// Test backward compatibility - SubscriptionStatus without optional fields
#[test]
fn test_subscription_status_backward_compatibility() {
    let json = r#"{"status": "active"}"#;

    let status: SubscriptionStatus =
        serde_json::from_str(json).expect("Should deserialize old format");

    assert_eq!(status.status, "active");
    assert_eq!(status.plan, None); // Should default to None
    assert_eq!(status.current_period_end, None); // Should default to None
}

/// Test CheckoutSessionResponse with minimal fields
#[test]
fn test_checkout_session_minimal() {
    let json = r#"{
        "checkout_url": "https://checkout.stripe.com/minimal",
        "session_id": "cs_minimal",
        "customer_email": "user@example.com"
    }"#;

    let response: CheckoutSessionResponse = serde_json::from_str(json).expect("Should deserialize");

    assert!(!response.checkout_url.is_empty());
    assert!(!response.session_id.is_empty());
    assert!(!response.customer_email.is_empty());
}

/// Test subscription status with canceled state
#[test]
fn test_subscription_status_canceled() {
    let json = r#"{
        "status": "canceled",
        "plan": "plan_basic",
        "current_period_end": "2026-01-31T23:59:59Z",
        "cancel_at_period_end": true
    }"#;

    let status: SubscriptionStatus = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(status.status, "canceled");
    assert_eq!(status.plan, Some("plan_basic".to_string()));
    assert_eq!(status.cancel_at_period_end, Some(true));
}

/// Test payment status with failed state
#[test]
fn test_payment_status_failed() {
    let json = r#"{
        "status": "failed",
        "customer_id": "cus_failed123"
    }"#;

    let response: PaymentStatusResponse = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(response.status, "failed");
    assert_eq!(response.subscription_id, None);
    assert_eq!(response.customer_id, Some("cus_failed123".to_string()));
}
