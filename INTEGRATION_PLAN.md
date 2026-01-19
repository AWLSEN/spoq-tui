# tui_spoq + spoq-web-apis Integration Plan

## Overview

Integrate device authorization flow (RFC 8628) and VPS provisioning into tui_spoq CLI, connecting it to the spoq-web-apis central backend.

**Central Backend URL (hardcoded):** `https://spoq-api-production.up.railway.app`

---

## 1. New Files to Create

### 1.1 `src/auth/mod.rs`
```rust
//! Authentication module for spoq-web-apis integration.

pub mod central_api;
pub mod credentials;
pub mod device_flow;

pub use central_api::CentralApiClient;
pub use credentials::{Credentials, CredentialsManager};
pub use device_flow::{DeviceFlowState, DeviceGrant};
```

### 1.2 `src/auth/credentials.rs`
```rust
//! Credential storage and management.
//! Stores credentials in ~/.spoq/credentials.json

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Credentials {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,  // Unix timestamp
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub vps_url: Option<String>,
    pub vps_status: Option<String>,
}

pub struct CredentialsManager {
    path: PathBuf,
}

impl CredentialsManager {
    pub fn new() -> std::io::Result<Self> {
        let spoq_dir = dirs::home_dir()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Home directory not found"
            ))?
            .join(".spoq");

        if !spoq_dir.exists() {
            std::fs::create_dir_all(&spoq_dir)?;
        }

        Ok(Self {
            path: spoq_dir.join("credentials.json"),
        })
    }

    pub fn load(&self) -> std::io::Result<Credentials> {
        if !self.path.exists() {
            return Ok(Credentials::default());
        }
        let content = std::fs::read_to_string(&self.path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, creds: &Credentials) -> std::io::Result<()> {
        let content = serde_json::to_string_pretty(creds)?;
        std::fs::write(&self.path, content)
    }

    pub fn clear(&self) -> std::io::Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    pub fn is_logged_in(&self) -> bool {
        self.load()
            .map(|c| c.access_token.is_some())
            .unwrap_or(false)
    }

    pub fn has_vps(&self) -> bool {
        self.load()
            .map(|c| c.vps_url.is_some() && c.vps_status.as_deref() == Some("ready"))
            .unwrap_or(false)
    }
}
```

### 1.3 `src/auth/central_api.rs`
```rust
//! HTTP client for spoq-web-apis central backend.

use reqwest::Client;
use serde::{Deserialize, Serialize};

pub const CENTRAL_API_URL: &str = "https://spoq-api-production.up.railway.app";

#[derive(Debug, Clone)]
pub struct CentralApiClient {
    client: Client,
    base_url: String,
}

// Device flow types
#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DeviceTokenResponse {
    Success {
        access_token: String,
        refresh_token: String,
        token_type: String,
        expires_in: i64,
    },
    Error {
        error: String,
        error_description: Option<String>,
    },
}

// VPS types
#[derive(Debug, Serialize)]
pub struct ProvisionVpsRequest {
    pub ssh_password: String,
    pub plan_id: Option<i64>,
    pub data_center_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VpsStatusResponse {
    pub id: String,
    pub hostname: Option<String>,
    pub status: String,
    pub ip_address: Option<String>,
    pub ssh_username: Option<String>,
    pub provider: String,
    pub plan_id: Option<i64>,
    pub created_at: String,
    pub ready_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VpsPlansResponse {
    pub plans: Vec<VpsPlan>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VpsPlan {
    pub id: i64,
    pub name: String,
    pub vcpus: i32,
    pub ram_mb: i64,
    pub disk_gb: i64,
    pub bandwidth_tb: i64,
    pub price_monthly: f64,
    pub data_centers: Vec<DataCenter>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DataCenter {
    pub id: String,
    pub name: String,
    pub country: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub expires_in: i64,
}

#[derive(Debug)]
pub enum ApiError {
    Network(reqwest::Error),
    AuthorizationPending,
    AccessDenied,
    ExpiredToken,
    InvalidGrant,
    Unauthorized,
    ServerError(String),
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::Network(err)
    }
}

impl CentralApiClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: CENTRAL_API_URL.to_string(),
        }
    }

    /// Initiate device authorization flow
    pub async fn device_authorize(&self) -> Result<DeviceCodeResponse, ApiError> {
        let resp = self.client
            .post(format!("{}/auth/device", self.base_url))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(ApiError::ServerError(resp.text().await.unwrap_or_default()))
        }
    }

    /// Poll for device token
    pub async fn device_token(&self, device_code: &str) -> Result<DeviceTokenResponse, ApiError> {
        let resp = self.client
            .post(format!("{}/auth/device/token", self.base_url))
            .json(&serde_json::json!({
                "device_code": device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
            }))
            .send()
            .await?;

        Ok(resp.json().await?)
    }

    /// Refresh access token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<RefreshTokenResponse, ApiError> {
        let resp = self.client
            .post(format!("{}/auth/refresh", self.base_url))
            .json(&serde_json::json!({
                "refresh_token": refresh_token
            }))
            .send()
            .await?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized);
        }

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(ApiError::ServerError(resp.text().await.unwrap_or_default()))
        }
    }

    /// Get VPS plans
    pub async fn get_vps_plans(&self, access_token: &str) -> Result<VpsPlansResponse, ApiError> {
        let resp = self.client
            .get(format!("{}/api/vps/plans", self.base_url))
            .bearer_auth(access_token)
            .send()
            .await?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized);
        }

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(ApiError::ServerError(resp.text().await.unwrap_or_default()))
        }
    }

    /// Provision a new VPS
    pub async fn provision_vps(
        &self,
        access_token: &str,
        request: &ProvisionVpsRequest,
    ) -> Result<VpsStatusResponse, ApiError> {
        let resp = self.client
            .post(format!("{}/api/vps/provision", self.base_url))
            .bearer_auth(access_token)
            .json(request)
            .send()
            .await?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized);
        }

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(ApiError::ServerError(resp.text().await.unwrap_or_default()))
        }
    }

    /// Get VPS status
    pub async fn get_vps_status(&self, access_token: &str) -> Result<Option<VpsStatusResponse>, ApiError> {
        let resp = self.client
            .get(format!("{}/api/vps/status", self.base_url))
            .bearer_auth(access_token)
            .send()
            .await?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized);
        }

        if resp.status() == 404 {
            return Ok(None);
        }

        if resp.status().is_success() {
            Ok(Some(resp.json().await?))
        } else {
            Err(ApiError::ServerError(resp.text().await.unwrap_or_default()))
        }
    }
}
```

### 1.4 `src/auth/device_flow.rs`
```rust
//! Device authorization flow state machine.

use super::central_api::{CentralApiClient, DeviceCodeResponse, DeviceTokenResponse, ApiError};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct DeviceGrant {
    pub device_code: String,
    pub verification_uri: String,
    pub expires_at: Instant,
    pub interval: Duration,
}

#[derive(Debug, Clone)]
pub enum DeviceFlowState {
    /// Initial state - not started
    NotStarted,
    /// Waiting for user to visit URL and authorize
    WaitingForUser {
        grant: DeviceGrant,
        last_poll: Option<Instant>,
    },
    /// Successfully authorized
    Authorized {
        access_token: String,
        refresh_token: String,
        expires_in: i64,
    },
    /// User denied authorization
    Denied,
    /// Device code expired
    Expired,
    /// Error occurred
    Error(String),
}

impl DeviceFlowState {
    pub fn is_terminal(&self) -> bool {
        matches!(self,
            DeviceFlowState::Authorized { .. } |
            DeviceFlowState::Denied |
            DeviceFlowState::Expired |
            DeviceFlowState::Error(_)
        )
    }
}

pub struct DeviceFlowManager {
    client: CentralApiClient,
    pub state: DeviceFlowState,
}

impl DeviceFlowManager {
    pub fn new() -> Self {
        Self {
            client: CentralApiClient::new(),
            state: DeviceFlowState::NotStarted,
        }
    }

    /// Start the device authorization flow
    pub async fn start(&mut self) -> Result<&DeviceGrant, ApiError> {
        let resp = self.client.device_authorize().await?;

        let grant = DeviceGrant {
            device_code: resp.device_code,
            verification_uri: resp.verification_uri,
            expires_at: Instant::now() + Duration::from_secs(resp.expires_in as u64),
            interval: Duration::from_secs(resp.interval as u64),
        };

        self.state = DeviceFlowState::WaitingForUser {
            grant: grant.clone(),
            last_poll: None,
        };

        if let DeviceFlowState::WaitingForUser { grant, .. } = &self.state {
            Ok(grant)
        } else {
            unreachable!()
        }
    }

    /// Poll for token - call this repeatedly while WaitingForUser
    pub async fn poll(&mut self) -> &DeviceFlowState {
        let (grant, last_poll) = match &self.state {
            DeviceFlowState::WaitingForUser { grant, last_poll } => {
                (grant.clone(), *last_poll)
            }
            _ => return &self.state,
        };

        // Check if expired
        if Instant::now() >= grant.expires_at {
            self.state = DeviceFlowState::Expired;
            return &self.state;
        }

        // Respect polling interval
        if let Some(last) = last_poll {
            if last.elapsed() < grant.interval {
                return &self.state;
            }
        }

        // Poll for token
        match self.client.device_token(&grant.device_code).await {
            Ok(DeviceTokenResponse::Success { access_token, refresh_token, expires_in, .. }) => {
                self.state = DeviceFlowState::Authorized {
                    access_token,
                    refresh_token,
                    expires_in,
                };
            }
            Ok(DeviceTokenResponse::Error { error, .. }) => {
                match error.as_str() {
                    "authorization_pending" => {
                        self.state = DeviceFlowState::WaitingForUser {
                            grant,
                            last_poll: Some(Instant::now()),
                        };
                    }
                    "access_denied" => {
                        self.state = DeviceFlowState::Denied;
                    }
                    "expired_token" => {
                        self.state = DeviceFlowState::Expired;
                    }
                    _ => {
                        self.state = DeviceFlowState::Error(error);
                    }
                }
            }
            Err(e) => {
                self.state = DeviceFlowState::Error(format!("{:?}", e));
            }
        }

        &self.state
    }
}
```

---

## 2. Files to Modify

### 2.1 `src/lib.rs` (line ~11)

**Add:**
```rust
pub mod auth;
```

### 2.2 `src/app/types.rs` - Screen enum (line ~17)

**Change from:**
```rust
pub enum Screen {
    #[default]
    CommandDeck,
    Conversation,
}
```

**To:**
```rust
pub enum Screen {
    /// Login screen - device authorization flow
    Login,
    /// VPS provisioning screen - shown when logged in but no VPS
    Provisioning,
    #[default]
    CommandDeck,
    Conversation,
}
```

### 2.3 `src/app/mod.rs` - App struct (around line 156)

**Add fields after `client: Arc<ConductorClient>`:**
```rust
    /// Central API client for spoq-web-apis
    pub central_api: Arc<CentralApiClient>,
    /// Credentials manager
    pub credentials_manager: CredentialsManager,
    /// Current credentials (cached)
    pub credentials: Credentials,
    /// Device flow manager (only during login)
    pub device_flow: Option<DeviceFlowManager>,
    /// VPS plans (loaded during provisioning)
    pub vps_plans: Vec<VpsPlan>,
    /// Selected VPS plan index
    pub selected_plan_idx: usize,
    /// SSH password input during provisioning
    pub ssh_password_input: String,
    /// Is password being entered
    pub entering_ssh_password: bool,
```

**Add imports at top:**
```rust
use crate::auth::{
    CentralApiClient, Credentials, CredentialsManager,
    DeviceFlowManager, DeviceFlowState,
    central_api::VpsPlan,
};
```

**Modify constructors to initialize new fields:**
```rust
// In App::new() and other constructors, add:
let credentials_manager = CredentialsManager::new()?;
let credentials = credentials_manager.load().unwrap_or_default();
let central_api = Arc::new(CentralApiClient::new());

// Then in the struct initialization:
central_api,
credentials_manager,
credentials,
device_flow: None,
vps_plans: Vec::new(),
selected_plan_idx: 0,
ssh_password_input: String::new(),
entering_ssh_password: false,
```

### 2.4 `src/conductor.rs` - Dynamic URL + Auth Headers (line 16+)

**Change from:**
```rust
pub const CONDUCTOR_BASE_URL: &str = "http://100.80.115.93:8000";
```

**To:**
```rust
// Default fallback URL - should be replaced by VPS URL from credentials
pub const DEFAULT_CONDUCTOR_URL: &str = "http://100.80.115.93:8000";
```

**Modify ConductorClient to accept dynamic URL:**
```rust
impl ConductorClient {
    pub fn new() -> Self {
        Self::with_url(DEFAULT_CONDUCTOR_URL)
    }

    pub fn with_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    // Modify all request methods to add auth header when token is provided
    // Example for stream():
    pub async fn stream(&self, request: &StreamRequest, auth_token: Option<&str>) -> Result<...> {
        let mut req = self.client
            .post(format!("{}/v1/stream", self.base_url))
            .json(request);

        if let Some(token) = auth_token {
            req = req.bearer_auth(token);
        }

        req.send().await?...
    }
}
```

### 2.5 `src/main.rs` - Startup Auth Check (around line 65)

**Add before App creation:**
```rust
use tui_spoq::auth::{CredentialsManager, CentralApiClient, DeviceFlowManager};

// In main(), before creating App:
let credentials_manager = CredentialsManager::new()
    .expect("Failed to initialize credentials manager");
let credentials = credentials_manager.load().unwrap_or_default();

// Determine initial screen
let initial_screen = if credentials.access_token.is_none() {
    Screen::Login
} else if credentials.vps_url.is_none() || credentials.vps_status.as_deref() != Some("ready") {
    Screen::Provisioning
} else {
    Screen::CommandDeck
};

// Create app with appropriate initial state
let mut app = App::with_debug(debug_tx)?;
app.current_screen = initial_screen;

// If we have a VPS URL, use it for the conductor client
if let Some(vps_url) = &credentials.vps_url {
    app.client = Arc::new(ConductorClient::with_url(vps_url));
}
```

### 2.6 `src/websocket/client.rs` - Add Auth Support (line 54+)

**Modify WsClientConfig:**
```rust
pub struct WsClientConfig {
    pub host: String,
    pub max_retries: u32,
    pub max_backoff_secs: u64,
    pub auth_token: Option<String>,  // ADD THIS
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            host: std::env::var("SPOQ_WS_HOST")
                .unwrap_or_else(|_| "100.80.115.93:8000".to_string()),
            max_retries: 5,
            max_backoff_secs: 32,
            auth_token: None,  // ADD THIS
        }
    }
}
```

**Modify connect() to use auth token in URL or headers** (depends on Conductor WebSocket auth implementation).

---

## 3. New UI Components

### 3.1 `src/ui/login.rs` (NEW FILE)

```rust
//! Login screen UI - shows device authorization flow.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::auth::DeviceFlowState;

pub fn render_login_screen(f: &mut Frame, state: &DeviceFlowState) {
    let area = f.area();

    // Center the content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Min(12),
            Constraint::Percentage(30),
        ])
        .split(area);

    let content_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Min(60),
            Constraint::Percentage(20),
        ])
        .split(chunks[1])[1];

    let block = Block::default()
        .title(" SPOQ Login ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(content_area);
    f.render_widget(block, content_area);

    match state {
        DeviceFlowState::NotStarted => {
            let text = Paragraph::new("Initializing...")
                .alignment(Alignment::Center);
            f.render_widget(text, inner);
        }
        DeviceFlowState::WaitingForUser { grant, .. } => {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "To sign in, visit:",
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    &grant.verification_uri,
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Waiting for authorization...",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "[Press Q to cancel]",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let text = Paragraph::new(lines)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            f.render_widget(text, inner);
        }
        DeviceFlowState::Authorized { .. } => {
            let text = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "✓ Successfully logged in!",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )),
            ])
            .alignment(Alignment::Center);
            f.render_widget(text, inner);
        }
        DeviceFlowState::Denied => {
            let text = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "✗ Authorization denied",
                    Style::default().fg(Color::Red),
                )),
                Line::from(""),
                Line::from("[Press any key to retry]"),
            ])
            .alignment(Alignment::Center);
            f.render_widget(text, inner);
        }
        DeviceFlowState::Expired => {
            let text = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "✗ Authorization expired",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(""),
                Line::from("[Press any key to retry]"),
            ])
            .alignment(Alignment::Center);
            f.render_widget(text, inner);
        }
        DeviceFlowState::Error(msg) => {
            let text = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("Error: {}", msg),
                    Style::default().fg(Color::Red),
                )),
                Line::from(""),
                Line::from("[Press any key to retry]"),
            ])
            .alignment(Alignment::Center);
            f.render_widget(text, inner);
        }
    }
}
```

### 3.2 `src/ui/provisioning.rs` (NEW FILE)

```rust
//! VPS provisioning screen UI.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::auth::central_api::VpsPlan;

pub struct ProvisioningState<'a> {
    pub plans: &'a [VpsPlan],
    pub selected_idx: usize,
    pub ssh_password: &'a str,
    pub entering_password: bool,
    pub provisioning_status: Option<&'a str>,
    pub error: Option<&'a str>,
}

pub fn render_provisioning_screen(f: &mut Frame, state: &ProvisioningState) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(10),    // Plans list
            Constraint::Length(5),  // Password input
            Constraint::Length(3),  // Actions
        ])
        .margin(2)
        .split(area);

    // Title
    let title = Paragraph::new("VPS Provisioning")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // Plans list or status
    if let Some(status) = state.provisioning_status {
        let status_text = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("Provisioning: {}", status),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from("This may take a few minutes..."),
        ])
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Status "));
        f.render_widget(status_text, chunks[1]);
    } else if state.plans.is_empty() {
        let loading = Paragraph::new("Loading plans...")
            .alignment(Alignment::Center);
        f.render_widget(loading, chunks[1]);
    } else {
        let items: Vec<ListItem> = state.plans.iter().enumerate().map(|(i, plan)| {
            let style = if i == state.selected_idx {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let marker = if i == state.selected_idx { "▶ " } else { "  " };
            ListItem::new(format!(
                "{}{} - {} vCPU, {} MB RAM, {} GB - ${:.2}/mo",
                marker, plan.name, plan.vcpus, plan.ram_mb, plan.disk_gb, plan.price_monthly
            ))
            .style(style)
        }).collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Select Plan (↑/↓) "));
        f.render_widget(list, chunks[1]);
    }

    // Password input
    let password_display = if state.entering_password {
        format!("SSH Password: {}", "*".repeat(state.ssh_password.len()))
    } else {
        "SSH Password: [Press P to enter]".to_string()
    };
    let password_style = if state.entering_password {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let password = Paragraph::new(password_display)
        .style(password_style)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(password, chunks[2]);

    // Actions
    let can_provision = !state.plans.is_empty()
        && state.ssh_password.len() >= 12
        && state.provisioning_status.is_none();

    let actions = if can_provision {
        "[Enter] Provision  [Q] Skip (use default)"
    } else if state.ssh_password.len() < 12 && !state.ssh_password.is_empty() {
        "Password must be at least 12 characters"
    } else {
        "[Q] Skip provisioning"
    };

    let actions_text = Paragraph::new(actions)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(actions_text, chunks[3]);

    // Error display
    if let Some(err) = state.error {
        // Render error overlay
    }
}
```

### 3.3 `src/ui/mod.rs` - Add new modules

**Add:**
```rust
pub mod login;
pub mod provisioning;

pub use login::render_login_screen;
pub use provisioning::{render_provisioning_screen, ProvisioningState};
```

---

## 4. Event Loop Modifications

### 4.1 `src/main.rs` - Handle Login/Provisioning Screens

In the main event loop's screen rendering section, add cases for new screens:

```rust
match app.current_screen {
    Screen::Login => {
        // Render login screen
        terminal.draw(|f| {
            ui::render_login_screen(f,
                app.device_flow.as_ref()
                    .map(|df| &df.state)
                    .unwrap_or(&DeviceFlowState::NotStarted)
            );
        })?;

        // Poll device flow if active
        if let Some(ref mut flow) = app.device_flow {
            if !flow.state.is_terminal() {
                flow.poll().await;

                // Check if authorized
                if let DeviceFlowState::Authorized { access_token, refresh_token, expires_in } = &flow.state {
                    // Save credentials
                    app.credentials.access_token = Some(access_token.clone());
                    app.credentials.refresh_token = Some(refresh_token.clone());
                    app.credentials.expires_at = Some(
                        chrono::Utc::now().timestamp() + expires_in
                    );
                    app.credentials_manager.save(&app.credentials)?;

                    // Move to provisioning or main screen
                    app.current_screen = Screen::Provisioning;
                    app.device_flow = None;
                }
            }
        }
    }
    Screen::Provisioning => {
        terminal.draw(|f| {
            ui::render_provisioning_screen(f, &ProvisioningState {
                plans: &app.vps_plans,
                selected_idx: app.selected_plan_idx,
                ssh_password: &app.ssh_password_input,
                entering_password: app.entering_ssh_password,
                provisioning_status: None,  // TODO: track status
                error: None,
            });
        })?;
    }
    Screen::CommandDeck | Screen::Conversation => {
        // Existing rendering logic
    }
}
```

### 4.2 Input Handling for New Screens

Add to input handler:

```rust
// In handle_key_event or similar:
match app.current_screen {
    Screen::Login => {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                // Cancel login and exit
                return Ok(true); // exit
            }
            _ => {}
        }
    }
    Screen::Provisioning => {
        if app.entering_ssh_password {
            match key.code {
                KeyCode::Enter => {
                    app.entering_ssh_password = false;
                }
                KeyCode::Esc => {
                    app.entering_ssh_password = false;
                    app.ssh_password_input.clear();
                }
                KeyCode::Backspace => {
                    app.ssh_password_input.pop();
                }
                KeyCode::Char(c) => {
                    app.ssh_password_input.push(c);
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    app.entering_ssh_password = true;
                }
                KeyCode::Up => {
                    if app.selected_plan_idx > 0 {
                        app.selected_plan_idx -= 1;
                    }
                }
                KeyCode::Down => {
                    if app.selected_plan_idx < app.vps_plans.len().saturating_sub(1) {
                        app.selected_plan_idx += 1;
                    }
                }
                KeyCode::Enter => {
                    if app.ssh_password_input.len() >= 12 {
                        // Trigger provisioning
                        app.start_vps_provisioning().await;
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    // Skip provisioning, go to main screen
                    app.current_screen = Screen::CommandDeck;
                }
                _ => {}
            }
        }
    }
    _ => { /* existing handling */ }
}
```

---

## 5. Token Refresh Logic

### 5.1 Add to `src/app/mod.rs`

```rust
impl App {
    /// Check and refresh token if needed
    pub async fn ensure_valid_token(&mut self) -> Result<String, AuthError> {
        let now = chrono::Utc::now().timestamp();

        // Check if token exists and is not expired (with 5 min buffer)
        if let Some(token) = &self.credentials.access_token {
            if let Some(expires_at) = self.credentials.expires_at {
                if expires_at > now + 300 {
                    return Ok(token.clone());
                }
            }
        }

        // Need to refresh
        if let Some(refresh_token) = &self.credentials.refresh_token {
            match self.central_api.refresh_token(refresh_token).await {
                Ok(resp) => {
                    self.credentials.access_token = Some(resp.access_token.clone());
                    self.credentials.expires_at = Some(now + resp.expires_in);
                    self.credentials_manager.save(&self.credentials)?;
                    Ok(resp.access_token)
                }
                Err(ApiError::Unauthorized) => {
                    // Refresh token invalid, need to re-login
                    self.credentials = Credentials::default();
                    self.credentials_manager.clear()?;
                    self.current_screen = Screen::Login;
                    Err(AuthError::SessionExpired)
                }
                Err(e) => Err(AuthError::Network(format!("{:?}", e))),
            }
        } else {
            self.current_screen = Screen::Login;
            Err(AuthError::NotLoggedIn)
        }
    }
}
```

---

## 6. Startup Flow Diagram

```
┌─────────────────┐
│   App Start     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Load Credentials│
│ from ~/.spoq/   │
└────────┬────────┘
         │
         ▼
    ┌────────────┐
    │Has Token?  │──No──▶ Screen::Login
    └─────┬──────┘              │
          │Yes                  ▼
          ▼               Device Flow
    ┌────────────┐        (show URL)
    │ Has VPS?   │              │
    │ Status=    │◀─────────────┘
    │ ready?     │        On Success
    └─────┬──────┘
          │
     No   │   Yes
     ▼    │    │
Screen::  │    │
Provision │    │
    │     │    │
    ▼     │    │
[User     │    │
 Options] │    │
    │     │    │
    │ Skip│    │
    │     ▼    ▼
    │  ┌──────────────┐
    └─▶│Screen::      │
       │CommandDeck   │
       └──────────────┘
```

---

## 7. Cargo.toml Changes

Add to `[dependencies]`:
```toml
chrono = { version = "0.4", features = ["serde"] }
```

(Note: `reqwest`, `serde`, `serde_json`, `tokio`, `dirs` already present)

---

## 8. Testing Checklist

1. **Login Flow**
   - [ ] Device code displayed correctly
   - [ ] Polling respects interval
   - [ ] Success transitions to Provisioning/Main
   - [ ] Denial shows error and allows retry
   - [ ] Expiration shows error and allows retry
   - [ ] Cancel (Q) exits cleanly

2. **Credentials Persistence**
   - [ ] ~/.spoq directory created if missing
   - [ ] credentials.json saved after login
   - [ ] Token loaded on app restart
   - [ ] Invalid credentials trigger re-login

3. **Token Refresh**
   - [ ] Expired tokens auto-refresh
   - [ ] Invalid refresh tokens trigger login
   - [ ] 401 responses trigger refresh

4. **VPS Provisioning**
   - [ ] Plans load from API
   - [ ] Plan selection works (↑/↓)
   - [ ] Password input masked
   - [ ] Validation: min 12 chars
   - [ ] Provisioning status updates
   - [ ] Skip option goes to main screen

5. **Conductor Integration**
   - [ ] Uses VPS URL from credentials
   - [ ] Auth header included in requests
   - [ ] Fallback to default URL works

---

## 9. Implementation Order

1. **Phase 1: Auth Module**
   - Create `src/auth/` directory
   - Implement `credentials.rs`
   - Implement `central_api.rs`
   - Implement `device_flow.rs`
   - Add module to `lib.rs`

2. **Phase 2: UI Screens**
   - Create `src/ui/login.rs`
   - Create `src/ui/provisioning.rs`
   - Update `src/ui/mod.rs`
   - Add Screen variants to `types.rs`

3. **Phase 3: App Integration**
   - Add new fields to App struct
   - Update constructors
   - Implement `ensure_valid_token()`
   - Add startup flow in `main.rs`

4. **Phase 4: Input Handling**
   - Handle Login screen inputs
   - Handle Provisioning screen inputs
   - Update event loop

5. **Phase 5: Conductor Integration**
   - Update `conductor.rs` for dynamic URL
   - Add auth header support
   - Update WebSocket config

6. **Phase 6: Testing**
   - Manual testing of full flow
   - Edge cases (network errors, expiration, etc.)
