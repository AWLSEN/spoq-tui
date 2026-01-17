# Multi-User and Device Management

## Overview

Spoq supports multiple users (profiles) and devices per Conductor instance. This document covers the architecture for user isolation, device management, and resource sharing.

## Base Plan Limits

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RESOURCE LIMITS                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Per Conductor Instance (Cloud or Jetson):                                  │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Profiles: 2 max                                                     │    │
│  │  ├── Profile 1 (Owner)                                              │    │
│  │  │   └── Devices: 2 max                                             │    │
│  │  │                                                                   │    │
│  │  └── Profile 2 (Family/Shared)                                      │    │
│  │      └── Devices: 2 max                                             │    │
│  │                                                                      │    │
│  │  Total Devices: 4 max                                               │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  Rationale:                                                                  │
│  • 2 profiles = typical household (individual + shared/family)              │
│  • 2 devices each = phone + computer                                        │
│  • Keeps resource usage predictable                                         │
│  • Higher tiers can increase limits                                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Profile Architecture

### Profile = Linux User

Each profile maps to a Linux user account with isolated home directory.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PROFILE STRUCTURE                                                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  /home/                                                                      │
│  ├── alice/                          # Profile 1 (Owner)                    │
│  │   ├── .ssh/                                                              │
│  │   │   └── authorized_keys         # SSH keys for alice's devices        │
│  │   ├── .spoq/                                                             │
│  │   │   ├── config.toml             # User preferences                    │
│  │   │   ├── history.db              # Conversation history                │
│  │   │   ├── agents/                 # Custom agents                       │
│  │   │   └── cache/                  # Model cache, temp files             │
│  │   └── workspace/                  # User's working directory            │
│  │                                                                          │
│  └── bob/                            # Profile 2                            │
│      ├── .ssh/                                                              │
│      │   └── authorized_keys         # SSH keys for bob's devices          │
│      ├── .spoq/                                                             │
│      │   ├── config.toml                                                   │
│      │   ├── history.db                                                    │
│      │   ├── agents/                                                       │
│      │   └── cache/                                                        │
│      └── workspace/                                                        │
│                                                                              │
│  Isolation:                                                                  │
│  • alice cannot read /home/bob                                              │
│  • bob cannot read /home/alice                                              │
│  • Standard Unix permissions (700 on home dirs)                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Profile Creation

```rust
// profile_manager.rs

use std::process::Command;

pub struct Profile {
    pub username: String,
    pub is_owner: bool,
    pub created_at: DateTime<Utc>,
    pub devices: Vec<Device>,
}

impl ProfileManager {
    pub fn create_profile(&self, username: &str, is_owner: bool) -> Result<Profile> {
        // Validate username
        if !is_valid_username(username) {
            return Err(Error::InvalidUsername);
        }

        // Check profile limit
        let existing = self.list_profiles()?;
        if existing.len() >= 2 {
            return Err(Error::ProfileLimitReached);
        }

        // Create Linux user
        Command::new("useradd")
            .args([
                "-m",                    // Create home directory
                "-s", "/bin/bash",       // Default shell
                "-G", "spoq",            // Add to spoq group
                username
            ])
            .status()?;

        // Create .spoq directory structure
        let home = format!("/home/{}", username);
        fs::create_dir_all(format!("{}/.spoq/agents", home))?;
        fs::create_dir_all(format!("{}/.spoq/cache", home))?;
        fs::create_dir_all(format!("{}/workspace", home))?;

        // Initialize user config
        let config = UserConfig::default();
        config.save(&format!("{}/.spoq/config.toml", home))?;

        // Set permissions
        Command::new("chown")
            .args(["-R", &format!("{}:{}", username, username), &home])
            .status()?;

        // Record in system database
        self.db.insert_profile(username, is_owner)?;

        Ok(Profile {
            username: username.to_string(),
            is_owner,
            created_at: Utc::now(),
            devices: vec![],
        })
    }

    pub fn delete_profile(&self, username: &str) -> Result<()> {
        // Cannot delete owner
        let profile = self.get_profile(username)?;
        if profile.is_owner {
            return Err(Error::CannotDeleteOwner);
        }

        // Remove Linux user and home directory
        Command::new("userdel")
            .args(["-r", username])  // -r removes home dir
            .status()?;

        // Remove from database
        self.db.delete_profile(username)?;

        // Remove WireGuard peers for this profile's devices
        for device in profile.devices {
            self.wireguard.remove_peer(&device.public_key)?;
        }

        Ok(())
    }
}
```

## Device Management

### Device = SSH Key + WireGuard Peer

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  DEVICE STRUCTURE                                                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Device:                                                                     │
│  ├── id: UUID                                                               │
│  ├── profile: "alice"                                                       │
│  ├── name: "iPhone 15 Pro"                                                  │
│  ├── ssh_public_key: "ssh-ed25519 AAAA..."                                 │
│  ├── sshid_code: "oaftobark" (optional, for tracking)                      │
│  ├── wireguard_public_key: "abc123..." (Jetson only)                       │
│  ├── wireguard_ip: "10.100.0.10" (Jetson only)                             │
│  ├── last_connected: DateTime                                               │
│  └── created_at: DateTime                                                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Adding a Device via ssh.id

```rust
// device_manager.rs

pub struct Device {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub name: String,
    pub ssh_public_key: String,
    pub sshid_code: Option<String>,
    pub wireguard_public_key: Option<String>,
    pub wireguard_ip: Option<String>,
    pub last_connected: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl DeviceManager {
    /// Add device using Termius ssh.id code
    pub async fn add_device_sshid(
        &self,
        profile: &str,
        sshid_code: &str,
        name: &str,
    ) -> Result<Device> {
        // Check device limit for profile
        let profile_devices = self.list_devices_for_profile(profile)?;
        if profile_devices.len() >= 2 {
            return Err(Error::DeviceLimitReached);
        }

        // Fetch SSH key from sshid.io
        let ssh_key = self.fetch_sshid_key(sshid_code).await?;

        // Add to authorized_keys
        self.add_authorized_key(profile, &ssh_key)?;

        // For Jetson: also set up WireGuard peer
        let (wg_pubkey, wg_ip) = if self.is_jetson() {
            let ip = self.allocate_wireguard_ip(profile)?;
            // User will provide WireGuard public key separately
            // or we generate a config for them
            (None, Some(ip))
        } else {
            (None, None)
        };

        // Create device record
        let device = Device {
            id: Uuid::new_v4(),
            profile_id: self.get_profile_id(profile)?,
            name: name.to_string(),
            ssh_public_key: ssh_key,
            sshid_code: Some(sshid_code.to_string()),
            wireguard_public_key: wg_pubkey,
            wireguard_ip: wg_ip,
            last_connected: None,
            created_at: Utc::now(),
        };

        self.db.insert_device(&device)?;

        Ok(device)
    }

    async fn fetch_sshid_key(&self, code: &str) -> Result<String> {
        let url = format!("https://sshid.io/{}", code);
        let response = reqwest::get(&url).await?;

        if !response.status().is_success() {
            return Err(Error::InvalidSshIdCode);
        }

        let key = response.text().await?;

        // Validate it's a valid SSH public key
        if !key.starts_with("ssh-") {
            return Err(Error::InvalidSshKey);
        }

        Ok(key.trim().to_string())
    }

    fn add_authorized_key(&self, profile: &str, key: &str) -> Result<()> {
        let auth_keys_path = format!("/home/{}/.ssh/authorized_keys", profile);

        // Ensure .ssh directory exists
        let ssh_dir = format!("/home/{}/.ssh", profile);
        fs::create_dir_all(&ssh_dir)?;

        // Append key
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&auth_keys_path)?;

        writeln!(file, "{}", key)?;

        // Fix permissions
        Command::new("chmod").args(["700", &ssh_dir]).status()?;
        Command::new("chmod").args(["600", &auth_keys_path]).status()?;
        Command::new("chown")
            .args(["-R", &format!("{}:{}", profile, profile), &ssh_dir])
            .status()?;

        Ok(())
    }

    pub fn remove_device(&self, device_id: Uuid) -> Result<()> {
        let device = self.get_device(device_id)?;
        let profile = self.get_profile_by_id(device.profile_id)?;

        // Remove from authorized_keys
        self.remove_authorized_key(&profile.username, &device.ssh_public_key)?;

        // Remove WireGuard peer if present
        if let Some(wg_key) = &device.wireguard_public_key {
            self.wireguard.remove_peer(wg_key)?;
        }

        // Delete from database
        self.db.delete_device(device_id)?;

        Ok(())
    }
}
```

### WireGuard IP Allocation (Jetson)

```rust
// wireguard_manager.rs

impl WireGuardManager {
    /// Allocate WireGuard IP for a device
    pub fn allocate_ip(&self, profile: &str) -> Result<String> {
        // IP scheme: 10.100.0.XY
        // X = profile number (1 or 2)
        // Y = device number (0 or 1)

        let profile_num = match profile {
            p if self.is_owner(p) => 1,
            _ => 2,
        };

        let existing_devices = self.get_profile_device_count(profile)?;
        let device_num = existing_devices; // 0-indexed

        let ip = format!("10.100.0.{}{}", profile_num, device_num);

        Ok(ip)
    }

    /// Add peer to WireGuard config
    pub fn add_peer(&self, public_key: &str, allowed_ip: &str) -> Result<()> {
        // Add to running interface
        Command::new("wg")
            .args([
                "set", "wg0",
                "peer", public_key,
                "allowed-ips", &format!("{}/32", allowed_ip),
            ])
            .status()?;

        // Also persist to config file
        self.update_config_file()?;

        Ok(())
    }

    /// Generate client config for device
    pub fn generate_client_config(
        &self,
        device_ip: &str,
        server_endpoint: &str,
    ) -> Result<String> {
        let private_key = self.generate_private_key()?;
        let public_key = self.derive_public_key(&private_key)?;

        let config = format!(r#"[Interface]
PrivateKey = {}
Address = {}/32
DNS = 10.100.0.1

[Peer]
PublicKey = {}
AllowedIPs = 10.100.0.0/24
Endpoint = {}:51820
PersistentKeepalive = 25
"#,
            private_key,
            device_ip,
            self.get_server_public_key()?,
            server_endpoint
        );

        Ok(config)
    }
}
```

## Shared Resources

### Local AI Inference (Jetson)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RESOURCE SHARING - AI INFERENCE                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Jetson GPU is shared between profiles:                                     │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Ollama Server (shared)                                              │    │
│  │                                                                      │    │
│  │  Queue:                                                              │    │
│  │  ┌──────────────────────────────────────────────────────────────┐   │    │
│  │  │ 1. alice: "Explain quantum computing"     [Processing]       │   │    │
│  │  │ 2. bob: "Write a Python script"           [Waiting]          │   │    │
│  │  │ 3. alice: "Continue..."                   [Waiting]          │   │    │
│  │  └──────────────────────────────────────────────────────────────┘   │    │
│  │                                                                      │    │
│  │  Fair scheduling: Round-robin between profiles                      │    │
│  │  Priority: Currently active session gets slight boost              │    │
│  │                                                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Request Queue Implementation

```rust
// inference_queue.rs

use tokio::sync::mpsc;
use std::collections::VecDeque;

pub struct InferenceRequest {
    pub id: Uuid,
    pub profile: String,
    pub prompt: String,
    pub model: String,
    pub response_tx: oneshot::Sender<InferenceResponse>,
}

pub struct InferenceQueue {
    queue: VecDeque<InferenceRequest>,
    processing: Option<InferenceRequest>,
    profile_last_served: HashMap<String, Instant>,
}

impl InferenceQueue {
    /// Fair scheduling: prioritize profile that hasn't been served recently
    pub fn next_request(&mut self) -> Option<InferenceRequest> {
        if self.queue.is_empty() {
            return None;
        }

        // Find request from least-recently-served profile
        let mut best_idx = 0;
        let mut oldest_time = Instant::now();

        for (idx, req) in self.queue.iter().enumerate() {
            let last_served = self.profile_last_served
                .get(&req.profile)
                .copied()
                .unwrap_or(Instant::now() - Duration::from_secs(3600));

            if last_served < oldest_time {
                oldest_time = last_served;
                best_idx = idx;
            }
        }

        let request = self.queue.remove(best_idx)?;
        self.profile_last_served.insert(request.profile.clone(), Instant::now());

        Some(request)
    }
}
```

## Database Schema

```sql
-- System-level tables (managed by Conductor)

-- Profiles
CREATE TABLE profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(32) UNIQUE NOT NULL,
    is_owner BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Devices
CREATE TABLE devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID REFERENCES profiles(id) ON DELETE CASCADE,
    name VARCHAR(64) NOT NULL,
    ssh_public_key TEXT NOT NULL,
    sshid_code VARCHAR(32),
    wireguard_public_key VARCHAR(64),
    wireguard_ip VARCHAR(15),
    last_connected_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Sessions (for tracking active connections)
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
    started_at TIMESTAMP DEFAULT NOW(),
    ended_at TIMESTAMP,
    source_ip VARCHAR(45)
);

-- Usage metrics (for future billing/limits)
CREATE TABLE usage_metrics (
    id SERIAL PRIMARY KEY,
    profile_id UUID REFERENCES profiles(id),
    metric_type VARCHAR(32),  -- 'inference', 'storage', 'bandwidth'
    value BIGINT,
    recorded_at TIMESTAMP DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_devices_profile ON devices(profile_id);
CREATE INDEX idx_sessions_device ON sessions(device_id);
CREATE INDEX idx_usage_profile_time ON usage_metrics(profile_id, recorded_at);
```

## TUI Management Interface

### Profile Management

```
$ spoq settings profiles

┌─ Profile Management ────────────────────────────────────────┐
│                                                              │
│  Profiles (2/2)                                             │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  alice (Owner)                           ● active      │ │
│  │  Created: Jan 10, 2024                                 │ │
│  │  Devices: 2/2                                          │ │
│  │    • iPhone 15 Pro        ● online                     │ │
│  │    • MacBook Pro          ○ offline                    │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  bob                                                    │ │
│  │  Created: Jan 12, 2024                                 │ │
│  │  Devices: 1/2                                          │ │
│  │    • iPad Air             ● online                     │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  [a] Add profile (limit reached)                            │
│  [d] Delete profile                                         │
│  [Enter] Manage selected profile                            │
│  [q] Back                                                   │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### Device Management

```
$ spoq settings devices

┌─ Device Management (alice) ─────────────────────────────────┐
│                                                              │
│  Devices (2/2)                                              │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  1. iPhone 15 Pro                         ● online     │ │
│  │     Added: Jan 10, 2024                                │ │
│  │     Last seen: 2 minutes ago                           │ │
│  │     ssh.id: oaftobark                                  │ │
│  │     WG IP: 10.100.0.10                                 │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  2. MacBook Pro                           ○ offline    │ │
│  │     Added: Jan 11, 2024                                │ │
│  │     Last seen: 3 hours ago                             │ │
│  │     ssh.id: r7xk2mpa                                   │ │
│  │     WG IP: 10.100.0.11                                 │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  [a] Add device (limit reached)                             │
│  [r] Remove device                                          │
│  [q] Back                                                   │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### Add Device Flow

```
$ spoq device add

┌─ Add Device ────────────────────────────────────────────────┐
│                                                              │
│  Step 1: Get your ssh.id code                               │
│  ─────────────────────────────                              │
│                                                              │
│  On your new device:                                        │
│  1. Open Termius app                                        │
│  2. Go to Settings → SSH Key                                │
│  3. Tap "Share" → Copy ssh.id code                         │
│                                                              │
│  Enter your ssh.id code:                                    │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ oaftobark                                              │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  [Enter] Fetch key                                          │
│  [m] Enter SSH key manually                                 │
│  [q] Cancel                                                 │
│                                                              │
└──────────────────────────────────────────────────────────────┘

> oaftobark [Enter]

┌─ Add Device ────────────────────────────────────────────────┐
│                                                              │
│  ✓ SSH key fetched from sshid.io                           │
│                                                              │
│  Step 2: Name your device                                   │
│  ────────────────────────                                   │
│                                                              │
│  Device name:                                               │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ iPhone 15 Pro                                          │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  [Enter] Add device                                         │
│  [q] Cancel                                                 │
│                                                              │
└──────────────────────────────────────────────────────────────┘

> iPhone 15 Pro [Enter]

┌─ Add Device ────────────────────────────────────────────────┐
│                                                              │
│  ✓ Device added successfully!                               │
│                                                              │
│  Device: iPhone 15 Pro                                      │
│  Profile: alice                                             │
│  WireGuard IP: 10.100.0.10                                  │
│                                                              │
│  Next steps:                                                │
│  1. In Termius, add a new host:                            │
│     Host: alice.spoq.dev                                   │
│     User: alice                                            │
│                                                              │
│  2. For WireGuard (optional, for better performance):       │
│     Scan this QR code with WireGuard app:                  │
│     ┌─────────────┐                                        │
│     │ ▄▄▄ ▄▄▄ ▄▄ │                                        │
│     │ █▄█ █▄█ ▄█ │                                        │
│     │ ▀▀▀ ▀▀▀ ▀▀ │                                        │
│     └─────────────┘                                        │
│                                                              │
│  [Enter] Done                                               │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

## Security Considerations

### Profile Isolation

```bash
# Each profile is a separate Linux user
# Standard Unix permissions enforce isolation

# Home directories
drwx------ alice alice /home/alice
drwx------ bob   bob   /home/bob

# alice cannot:
# - Read /home/bob
# - See bob's processes (with hidepid mount option)
# - Access bob's SSH keys

# Shared resources are accessed via Conductor service
# which runs as root and enforces access control
```

### Device Authorization

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  DEVICE AUTHORIZATION FLOW                                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Adding device requires:                                                     │
│  1. Active session as profile owner (SSH already connected)                 │
│  2. OR: Web app authentication (Cloud)                                      │
│  3. OR: Physical access to Jetson during setup                             │
│                                                                              │
│  Cannot add devices:                                                         │
│  • Remotely without authentication                                          │
│  • To another profile (unless owner)                                        │
│  • Beyond the limit (2 per profile)                                         │
│                                                                              │
│  Removing devices:                                                           │
│  • Profile owner can remove own devices                                     │
│  • System owner can remove any device                                       │
│  • Immediately revokes SSH and WireGuard access                            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Implementation Checklist

### Profile Management
- [ ] Create profile (Linux user + directory structure)
- [ ] Delete profile (with cascade to devices)
- [ ] List profiles
- [ ] Profile storage quota

### Device Management
- [ ] Add device via ssh.id
- [ ] Add device via manual SSH key
- [ ] Remove device
- [ ] List devices
- [ ] Track last connection time

### WireGuard (Jetson)
- [ ] Allocate WireGuard IPs
- [ ] Add/remove peers
- [ ] Generate client configs
- [ ] QR code generation

### TUI Interface
- [ ] Profile management screens
- [ ] Device management screens
- [ ] Add device wizard
- [ ] Status indicators (online/offline)

### Security
- [ ] Profile isolation verification
- [ ] SSH key validation
- [ ] Device limit enforcement
- [ ] Audit logging
