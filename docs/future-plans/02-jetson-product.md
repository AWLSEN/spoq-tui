# Spoq Jetson Product Implementation

## Overview

Spoq Jetson is a self-hosted AI appliance in a HomePod/Alexa form factor. Users purchase the hardware once and run Conductor locally with on-device AI inference.

## Product Vision

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  SPOQ JETSON - "Your Personal AI Computer"                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│                        ┌─────────────────────┐                               │
│                        │    ┌───────────┐    │                               │
│                        │    │  ○ ○ ○    │    │  ← LED status ring            │
│                        │    │           │    │                               │
│                        │    │   SPOQ    │    │                               │
│                        │    │           │    │                               │
│                        │    └───────────┘    │                               │
│                        │                     │                               │
│                        │  ┌─────────────┐   │                               │
│                        │  │   Jetson    │   │  ← NVIDIA Jetson inside        │
│                        │  │   Orin     │   │                               │
│                        │  └─────────────┘   │                               │
│                        │                     │                               │
│                        └─────────────────────┘                               │
│                         ~6" diameter, ~4" tall                               │
│                                                                              │
│  Features:                                                                   │
│  • Plug in, pair with phone, done                                           │
│  • Local AI inference (no cloud required for basic tasks)                   │
│  • Access from anywhere via secure tunnel                                   │
│  • Privacy-first (data never leaves home)                                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Hardware Specifications

### Option A: Jetson Orin Nano (Entry)

| Spec | Value |
|------|-------|
| GPU | 1024 CUDA cores |
| AI Performance | 40 TOPS |
| CPU | 6-core Arm Cortex-A78AE |
| Memory | 8 GB LPDDR5 |
| Storage | 128 GB NVMe (upgradeable) |
| Power | 15W |
| Cost (module) | ~$200 |

### Option B: Jetson Orin NX (Performance)

| Spec | Value |
|------|-------|
| GPU | 1024 CUDA cores |
| AI Performance | 100 TOPS |
| CPU | 8-core Arm Cortex-A78AE |
| Memory | 16 GB LPDDR5 |
| Storage | 256 GB NVMe |
| Power | 25W |
| Cost (module) | ~$400 |

### Enclosure Components

| Component | Est. Cost |
|-----------|-----------|
| Custom enclosure (injection molded) | $15-25 |
| LED ring (RGB, addressable) | $5 |
| Power supply (USB-C PD) | $10 |
| Carrier board | $50-100 |
| WiFi/BT module | Included |
| Ethernet port | Included |
| Thermal solution | $10 |
| **Total BOM** | ~$300-500 |

### Retail Pricing

| Model | BOM | Retail | Margin |
|-------|-----|--------|--------|
| Orin Nano | ~$300 | $499 | 40% |
| Orin NX | ~$500 | $799 | 37% |

## Software Stack

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  JETSON SOFTWARE STACK                                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Application Layer                                                   │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │    │
│  │  │ Conductor    │  │ TUI (Spark)  │  │ Setup Daemon │              │    │
│  │  │ (AI runtime) │  │              │  │ (pairing)    │              │    │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Services Layer                                                      │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │    │
│  │  │ SSH Server   │  │ WireGuard    │  │ LED Control  │              │    │
│  │  │              │  │              │  │              │              │    │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  AI/ML Layer                                                         │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │    │
│  │  │ Ollama       │  │ TensorRT     │  │ CUDA         │              │    │
│  │  │ (LLM server) │  │ (inference)  │  │              │              │    │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  System Layer                                                        │    │
│  │  ┌──────────────────────────────────────────────────────────────┐   │    │
│  │  │  JetPack OS (Ubuntu-based)                                    │   │    │
│  │  │  Linux Kernel with NVIDIA drivers                             │   │    │
│  │  └──────────────────────────────────────────────────────────────┘   │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Local AI Models

### Pre-installed Models

| Model | Size | Use Case |
|-------|------|----------|
| Llama 3.2 3B | ~2 GB | Fast responses, simple tasks |
| Mistral 7B | ~4 GB | General purpose |
| CodeLlama 7B | ~4 GB | Code assistance |
| Whisper Small | ~500 MB | Voice transcription |

### Downloadable Models (Orin NX only)

| Model | Size | Use Case |
|-------|------|----------|
| Llama 3.1 8B | ~5 GB | Higher quality responses |
| Mixtral 8x7B | ~26 GB | Best local quality |

## LED Status System

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LED STATES                                                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ○ ○ ○  Pulsing Blue      → Ready to pair (first boot)                     │
│  ○ ○ ○  Solid Blue        → Pairing in progress                            │
│  ○ ○ ○  Pulsing Cyan      → Connecting to WiFi                             │
│  ○ ○ ○  Solid Green       → Running, healthy                               │
│  ○ ○ ○  Pulsing Green     → Active inference (thinking)                    │
│  ○ ○ ○  Solid Orange      → Updating software                              │
│  ○ ○ ○  Pulsing Orange    → Needs attention (check app)                    │
│  ○ ○ ○  Solid Red         → Error state                                    │
│  ○ ○ ○  Pulsing Red       → Critical error                                 │
│  ○ ○ ○  Rainbow cycle     → Factory reset in progress                      │
│  ○ ○ ○  Off               → Powered off / standby                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### LED Control Implementation

```rust
// led_controller.rs
use rppal::gpio::Gpio;

pub enum LedState {
    PairingReady,    // Pulsing blue
    Pairing,         // Solid blue
    Connecting,      // Pulsing cyan
    Running,         // Solid green
    Thinking,        // Pulsing green
    Updating,        // Solid orange
    NeedsAttention,  // Pulsing orange
    Error,           // Solid red
    CriticalError,   // Pulsing red
    FactoryReset,    // Rainbow
    Off,
}

impl LedController {
    pub fn set_state(&mut self, state: LedState) {
        match state {
            LedState::Running => self.solid(Color::GREEN),
            LedState::Thinking => self.pulse(Color::GREEN, Duration::from_millis(500)),
            LedState::PairingReady => self.pulse(Color::BLUE, Duration::from_secs(2)),
            // ... etc
        }
    }
}
```

## Pairing Methods

### Method 1: Bluetooth LE (Recommended)

Apple-like experience, no app required for initial WiFi setup.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  BLUETOOTH PAIRING FLOW                                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. User plugs in Jetson                                                    │
│     └── LED pulses blue                                                     │
│     └── BLE advertises: "Spoq-XXXX" (last 4 of serial)                     │
│                                                                              │
│  2. User opens Spoq iOS/Android app                                         │
│     └── App shows nearby Spoq devices                                       │
│     └── Or: iOS shows "Spoq nearby" banner (like AirPods)                  │
│                                                                              │
│  3. User taps "Connect"                                                     │
│     └── LED solid blue                                                      │
│     └── BLE handshake establishes secure channel                           │
│                                                                              │
│  4. App shares WiFi credentials (from phone)                                │
│     └── Like HomePod, automatically uses phone's current WiFi              │
│     └── LED pulsing cyan                                                    │
│                                                                              │
│  5. Jetson connects to WiFi                                                 │
│     └── Registers with Spoq cloud (gets subdomain)                         │
│     └── LED solid green                                                     │
│                                                                              │
│  6. App prompts for profile setup                                           │
│     └── Username                                                            │
│     └── Device automatically added (phone's Termius key or generated)      │
│                                                                              │
│  7. Done                                                                    │
│     └── "Your Spoq is ready at alice.spoq.dev"                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Method 2: WiFi Hotspot (No App Fallback)

Works without installing any app.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  WIFI HOTSPOT PAIRING FLOW                                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. User plugs in Jetson                                                    │
│     └── LED pulses blue                                                     │
│     └── Creates WiFi network: "Spoq-Setup-XXXX"                            │
│                                                                              │
│  2. User connects phone/laptop to "Spoq-Setup-XXXX"                        │
│     └── Captive portal opens automatically                                  │
│     └── Or navigate to: http://192.168.4.1                                 │
│                                                                              │
│  3. Web UI guides through setup:                                            │
│     └── Select home WiFi network                                           │
│     └── Enter WiFi password                                                │
│     └── Create profile (username)                                          │
│     └── Enter ssh.id code for device                                       │
│                                                                              │
│  4. Jetson connects to home WiFi                                            │
│     └── "Spoq-Setup" network disappears                                    │
│     └── LED solid green                                                     │
│                                                                              │
│  5. Setup complete                                                          │
│     └── Shows: "Connect to alice.spoq.dev"                                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Method 3: QR Code on Device (Simplest)

Physical QR sticker on bottom of device.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  QR CODE PAIRING FLOW                                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Device has unique QR code printed on bottom:                               │
│  ┌─────────────────────────────────────────┐                               │
│  │  ┌───────────┐                          │                               │
│  │  │ ▄▄▄ ▄▄▄  │  Serial: SPQ-A7X2-9K3M  │                               │
│  │  │ █▄█ █▄█  │  Setup: spoq.dev/setup   │                               │
│  │  │ ▀▀▀ ▀▀▀  │                          │                               │
│  │  └───────────┘                          │                               │
│  └─────────────────────────────────────────┘                               │
│                                                                              │
│  Flow:                                                                       │
│  1. User scans QR with phone camera                                         │
│  2. Opens spoq.dev/setup?device=SPQ-A7X2-9K3M                              │
│  3. Web app guides WiFi + profile setup                                    │
│  4. Jetson receives config via cloud relay                                  │
│                                                                              │
│  Requires: Jetson connected to ethernet first                              │
│  Or: Combined with WiFi hotspot method                                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Network Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  HOME NETWORK                                                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Jetson (192.168.1.100)                                              │    │
│  │                                                                      │    │
│  │  ┌──────────────────┐  ┌──────────────────┐                         │    │
│  │  │ WireGuard Server │  │ SSH Server       │                         │    │
│  │  │ wg0: 10.100.0.1  │  │ Listens on wg0   │                         │    │
│  │  │ UDP :51820       │  │ 10.100.0.1:22    │                         │    │
│  │  └──────────────────┘  └──────────────────┘                         │    │
│  │           │                                                          │    │
│  │           │ Port forward (UPnP auto)                                 │    │
│  │           ▼                                                          │    │
│  └───────────┼──────────────────────────────────────────────────────────┘    │
│              │                                                               │
│  ┌───────────┼──────────────────────────────────────────────────────────┐    │
│  │  Router   │                                                          │    │
│  │  └── UDP 51820 → 192.168.1.100:51820                                │    │
│  │  └── Public IP: 73.x.x.x                                            │    │
│  │  └── DynDNS: alice.spoq.dev → 73.x.x.x                              │    │
│  └───────────┼──────────────────────────────────────────────────────────┘    │
│              │                                                               │
└──────────────┼───────────────────────────────────────────────────────────────┘
               │
          [Internet]
               │
     ┌─────────┴─────────┐
     │                   │
 [iPhone]            [iPad]
 WireGuard           WireGuard
 10.100.0.10         10.100.0.11
     │                   │
 [Termius]           [Termius]
 SSH to              SSH to
 10.100.0.1          10.100.0.1
```

## Firmware Update System

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OTA UPDATE FLOW                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. Jetson checks for updates periodically (daily)                          │
│     └── GET https://updates.spoq.dev/check?version=X.Y.Z                   │
│                                                                              │
│  2. If update available:                                                    │
│     └── LED turns solid orange                                             │
│     └── Downloads update in background                                      │
│     └── Notifies user via TUI: "Update available, restart to apply"        │
│                                                                              │
│  3. User approves (or auto-install at 3am):                                │
│     └── System restarts                                                     │
│     └── A/B partition swap for safe rollback                               │
│                                                                              │
│  4. After reboot:                                                           │
│     └── Verify health check                                                │
│     └── If failed, auto-rollback to previous version                       │
│                                                                              │
│  Partition Layout:                                                           │
│  ┌────────────────────────────────────────────────────────────────┐        │
│  │  /dev/nvme0n1p1  Boot                                          │        │
│  │  /dev/nvme0n1p2  System A (active)                             │        │
│  │  /dev/nvme0n1p3  System B (standby)                            │        │
│  │  /dev/nvme0n1p4  User data (persistent)                        │        │
│  └────────────────────────────────────────────────────────────────┘        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Factory Reset

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  FACTORY RESET METHODS                                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Method 1: Via TUI                                                          │
│  $ spoq settings → Factory Reset → Confirm                                 │
│                                                                              │
│  Method 2: Physical Button                                                  │
│  Hold reset button for 10 seconds while LED flashes                        │
│                                                                              │
│  Method 3: Via App                                                          │
│  Spoq app → Device Settings → Factory Reset                                │
│                                                                              │
│  What happens:                                                              │
│  1. LED rainbow cycle                                                       │
│  2. User data wiped                                                         │
│  3. Network config cleared                                                  │
│  4. Returns to pairing mode                                                 │
│  5. LED pulsing blue                                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Mobile App (iOS/Android)

### Core Features

1. **Pairing** - BLE setup flow
2. **Status** - Device health, online status
3. **Device Management** - Add/remove devices
4. **Profile Management** - Add/remove profiles
5. **Network Settings** - Change WiFi, view IP
6. **Updates** - Trigger firmware updates
7. **Factory Reset** - Remote reset

### App Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  SPOQ MOBILE APP                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Framework: React Native or Flutter                                  │    │
│  │                                                                      │    │
│  │  Screens:                                                            │    │
│  │  ├── Onboarding                                                     │    │
│  │  ├── Device Discovery (BLE scan)                                    │    │
│  │  ├── Pairing                                                        │    │
│  │  ├── WiFi Setup                                                     │    │
│  │  ├── Profile Creation                                               │    │
│  │  ├── Home (device status)                                           │    │
│  │  ├── Settings                                                       │    │
│  │  └── Device Management                                              │    │
│  │                                                                      │    │
│  │  Native Modules:                                                     │    │
│  │  ├── BLE (react-native-ble-plx)                                     │    │
│  │  ├── WiFi provisioning                                              │    │
│  │  └── Keychain (secure storage)                                      │    │
│  │                                                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  Communication with Jetson:                                                  │
│  ├── BLE: Pairing, WiFi config                                             │
│  ├── Local API: When on same network                                       │
│  └── Cloud API: When remote (status, management)                           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Cloud Integration

Even self-hosted Jetsons connect to Spoq cloud for:

1. **Subdomain management** - alice.spoq.dev DNS
2. **Dynamic DNS updates** - Track changing home IPs
3. **Relay fallback** - When direct connection fails
4. **Push notifications** - Alerts via mobile app
5. **Remote status** - Is my Jetson online?
6. **OTA updates** - Firmware distribution

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  JETSON ↔ CLOUD COMMUNICATION                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Jetson → Cloud (outbound only, no inbound ports needed):                   │
│                                                                              │
│  1. Registration                                                            │
│     POST /api/devices/register                                             │
│     Body: { serial, public_key }                                           │
│     Returns: { subdomain, relay_config }                                   │
│                                                                              │
│  2. Heartbeat (every 60s)                                                   │
│     POST /api/devices/heartbeat                                            │
│     Body: { serial, public_ip, local_ip, status }                          │
│                                                                              │
│  3. DNS Update (on IP change)                                               │
│     POST /api/devices/dns-update                                           │
│     Body: { serial, public_ip }                                            │
│                                                                              │
│  4. Update Check                                                            │
│     GET /api/updates/check?version=X.Y.Z                                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Implementation Phases

### Phase 1: Proof of Concept (4 weeks)
- [ ] Basic Jetson image (JetPack + Conductor)
- [ ] WireGuard auto-setup
- [ ] WiFi hotspot pairing
- [ ] LED control (basic)
- [ ] ssh.id integration

### Phase 2: Mobile App (4 weeks)
- [ ] iOS app skeleton
- [ ] BLE pairing protocol
- [ ] WiFi provisioning
- [ ] Basic device management

### Phase 3: Polish (4 weeks)
- [ ] Android app
- [ ] OTA update system
- [ ] A/B partitions
- [ ] Factory reset
- [ ] Full LED states

### Phase 4: Hardware (8+ weeks)
- [ ] Enclosure design
- [ ] Thermal testing
- [ ] Carrier board selection/design
- [ ] Manufacturing partner
- [ ] Certifications (FCC, CE)

### Phase 5: Launch
- [ ] Packaging design
- [ ] Documentation
- [ ] Support system
- [ ] Distribution (direct, Amazon?)

## Bill of Materials (Detailed)

| Component | Part | Qty | Unit Cost | Total |
|-----------|------|-----|-----------|-------|
| Compute module | Jetson Orin Nano 8GB | 1 | $199 | $199 |
| Carrier board | Custom or Seeed J401 | 1 | $69 | $69 |
| Storage | 128GB NVMe SSD | 1 | $20 | $20 |
| WiFi/BT | Intel AX200 | 1 | $15 | $15 |
| Enclosure | Custom injection mold | 1 | $15 | $15 |
| LED ring | WS2812B 12-LED | 1 | $3 | $3 |
| Power supply | 65W USB-C PD | 1 | $12 | $12 |
| Thermal | Heatsink + fan | 1 | $8 | $8 |
| Cables/misc | Internal wiring | 1 | $5 | $5 |
| **BOM Total** | | | | **$346** |
| Assembly | | | | $15 |
| Packaging | Box, manual, stickers | | | $5 |
| **Unit Cost** | | | | **$366** |
| **Retail Price** | | | | **$499** |
| **Margin** | | | | **27%** |

Note: Volume pricing at 1000+ units would reduce BOM by ~15-20%.
