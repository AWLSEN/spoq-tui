# Spoq Platform Architecture Overview

## Vision

Spoq is a personal AI computing platform that gives users their own "brain" - a dedicated computing environment running Conductor (AI orchestration) accessible from any device, anywhere.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SPOQ PLATFORM                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│                         ┌─────────────────────┐                              │
│                         │     CONDUCTOR       │                              │
│                         │     (The Brain)     │                              │
│                         │                     │                              │
│                         │  • AI Orchestration │                              │
│                         │  • Agent Runtime    │                              │
│                         │  • Local Models     │                              │
│                         │  • User Data        │                              │
│                         └─────────────────────┘                              │
│                                    │                                         │
│                           Secure Tunnel                                      │
│                         (WireGuard/SSH)                                      │
│                                    │                                         │
│                         ┌─────────────────────┐                              │
│                         │        TUI          │                              │
│                         │  (Spark Interface)  │                              │
│                         └─────────────────────┘                              │
│                                    │                                         │
│              ┌─────────────────────┼─────────────────────┐                   │
│              ▼                     ▼                     ▼                   │
│         [iPhone]              [iPad]                [Mac]                    │
│         Termius               Termius              Terminal                  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Two Product Tiers

### Tier 1: Spoq Cloud (VPS-based)

- **Model**: Monthly subscription
- **Infrastructure**: We manage servers
- **Local inference**: No (API-based only)
- **Target**: Users who want simplicity, no hardware
- **Setup**: Web app, 5 minutes

### Tier 2: Spoq Jetson (Self-hosted Appliance)

- **Model**: One-time hardware purchase
- **Infrastructure**: User's home network
- **Local inference**: Yes (on-device AI)
- **Target**: Privacy-focused, power users
- **Setup**: Apple-like pairing, 3 minutes
- **Form factor**: HomePod/Alexa-like appliance

## Base Plan Limits (Both Tiers)

| Resource | Limit |
|----------|-------|
| Profiles | 2 max |
| Devices per profile | 2 max |
| Total devices | 4 max |

## Core Components

### 1. Conductor
The AI orchestration layer running on the server (VPS or Jetson).
- Agent runtime and management
- Local model inference (Jetson only)
- User workspace isolation
- API endpoints for TUI

### 2. TUI (Spark)
Terminal-based interaction layer.
- Runs over SSH connection
- Rich terminal UI (Ratatui)
- Chat, agents, tools
- Cross-platform via Termius

### 3. Networking Layer
Secure remote access infrastructure.
- Custom subdomains (alice.spoq.dev)
- WireGuard tunnels (Jetson)
- SSH over tunnel
- Dynamic DNS

### 4. User Management
Multi-user support with isolation.
- Profile creation and management
- Device registration (ssh.id)
- Namespace isolation
- Resource limits

## Technology Stack

| Component | Technology |
|-----------|------------|
| TUI | Rust + Ratatui |
| Conductor | Rust/Python |
| Cloud Orchestration | Kubernetes/Docker |
| Networking | WireGuard, frp, Cloudflare |
| Web App | Next.js |
| Database | PostgreSQL |
| Auth | SSH keys via ssh.id |

## Development Roadmap

### Phase 1: Cloud MVP
1. Web app for signup/setup
2. Container orchestration
3. Subdomain routing
4. ssh.id device registration
5. Basic multi-user

### Phase 2: Cloud Polish
1. Billing integration (Stripe)
2. Usage monitoring
3. Profile management UI
4. Device management

### Phase 3: Jetson Development
1. Jetson image/firmware
2. Pairing protocol (BLE or WiFi)
3. WireGuard automation
4. Local inference integration

### Phase 4: Jetson Launch
1. Hardware sourcing
2. Manufacturing
3. Packaging/branding
4. Distribution

## Related Documents

- [01-cloud-product.md](./01-cloud-product.md) - Cloud implementation details
- [02-jetson-product.md](./02-jetson-product.md) - Jetson appliance details
- [03-networking.md](./03-networking.md) - Networking architecture
- [04-multi-user.md](./04-multi-user.md) - Multi-user system
- [05-setup-flows.md](./05-setup-flows.md) - User experience flows
