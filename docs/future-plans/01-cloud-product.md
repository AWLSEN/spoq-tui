# Spoq Cloud Product Implementation

## Overview

Spoq Cloud provides a managed Conductor instance running on our infrastructure. Users subscribe monthly and access their personal AI computer from any device via SSH.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  SPOQ CLOUD INFRASTRUCTURE                                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Load Balancer / Ingress                                             │    │
│  │  *.spoq.dev → Route to correct container                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│  ┌─────────────────────────────────┼───────────────────────────────────┐    │
│  │  Container Orchestration (Kubernetes/Docker Swarm)                   │    │
│  │                                                                      │    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │    │
│  │  │Container │  │Container │  │Container │  │Container │            │    │
│  │  │alice     │  │bob       │  │carol     │  │  ...     │            │    │
│  │  │.spoq.dev │  │.spoq.dev │  │.spoq.dev │  │          │            │    │
│  │  │          │  │          │  │          │  │          │            │    │
│  │  │Conductor │  │Conductor │  │Conductor │  │Conductor │            │    │
│  │  │SSH Server│  │SSH Server│  │SSH Server│  │SSH Server│            │    │
│  │  │TUI       │  │TUI       │  │TUI       │  │TUI       │            │    │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │    │
│  │                                                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Shared Services                                                     │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │    │
│  │  │ API Server   │  │ PostgreSQL   │  │ Redis        │              │    │
│  │  │ (management) │  │ (metadata)   │  │ (sessions)   │              │    │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Container Specification

Each user gets an isolated container running:

```dockerfile
# Dockerfile for user container
FROM ubuntu:22.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    openssh-server \
    wireguard-tools \
    curl \
    git \
    python3 \
    && rm -rf /var/lib/apt/lists/*

# Install Conductor
COPY conductor /usr/local/bin/conductor

# Install TUI (Spark)
COPY spoq-tui /usr/local/bin/spoq

# SSH configuration
RUN mkdir /var/run/sshd
RUN sed -i 's/#PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config

# Create user template
# (Users are created dynamically)

EXPOSE 22

CMD ["/usr/sbin/sshd", "-D"]
```

## Resource Limits Per Container

| Resource | Limit |
|----------|-------|
| CPU | 1 core |
| Memory | 2 GB |
| Storage | 10 GB |
| Bandwidth | Fair share |

## Database Schema

```sql
-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    subdomain VARCHAR(63) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    subscription_status VARCHAR(20) DEFAULT 'trial',
    subscription_ends_at TIMESTAMP
);

-- Profiles (2 max per user)
CREATE TABLE profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    username VARCHAR(32) NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(user_id, username)
);

-- Devices (2 max per profile, 4 max per user)
CREATE TABLE devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID REFERENCES profiles(id),
    name VARCHAR(64),
    ssh_public_key TEXT NOT NULL,
    sshid_code VARCHAR(32),
    last_connected_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Container assignments
CREATE TABLE containers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) UNIQUE,
    container_id VARCHAR(64),
    host_node VARCHAR(255),
    status VARCHAR(20) DEFAULT 'running',
    created_at TIMESTAMP DEFAULT NOW()
);
```

## API Endpoints

### Public API (spoq.dev/api)

```
POST /api/auth/register
  Body: { email, password }
  Returns: { user_id, token }

POST /api/auth/login
  Body: { email, password }
  Returns: { token }

GET /api/subdomain/check/:name
  Returns: { available: boolean }

POST /api/setup/subdomain
  Auth: Bearer token
  Body: { subdomain }
  Returns: { subdomain, status }

POST /api/setup/profile
  Auth: Bearer token
  Body: { username }
  Returns: { profile_id }

POST /api/devices/add
  Auth: Bearer token
  Body: { profile_id, sshid_code } or { profile_id, ssh_public_key }
  Returns: { device_id, connection_info }

GET /api/devices
  Auth: Bearer token
  Returns: { devices: [...] }

DELETE /api/devices/:id
  Auth: Bearer token
  Returns: { success: boolean }

GET /api/status
  Auth: Bearer token
  Returns: { container_status, profiles, devices }
```

### Internal API (Container Management)

```
POST /internal/containers/create
  Body: { user_id, subdomain }
  Returns: { container_id }

POST /internal/containers/:id/add-user
  Body: { username, ssh_keys: [...] }
  Returns: { success: boolean }

DELETE /internal/containers/:id
  Returns: { success: boolean }

GET /internal/containers/:id/status
  Returns: { status, resource_usage }
```

## Web App Pages

### 1. Landing Page (spoq.dev)
- Product description
- Pricing
- Sign up CTA

### 2. Sign Up (spoq.dev/signup)
- Email/password registration
- Or OAuth (Google, GitHub)

### 3. Setup Wizard (spoq.dev/setup)

**Step 1: Choose Subdomain**
```
┌─────────────────────────────────────────────────────────────┐
│  Choose your address                                        │
│                                                             │
│  ┌──────────────┐                                          │
│  │ alice        │ .spoq.dev                                │
│  └──────────────┘                                          │
│  ✓ Available                                               │
│                                                             │
│                                    [Continue →]            │
└─────────────────────────────────────────────────────────────┘
```

**Step 2: Create Profile**
```
┌─────────────────────────────────────────────────────────────┐
│  Create your profile                                        │
│                                                             │
│  This is your login username on your Spoq computer.        │
│                                                             │
│  Username: ┌──────────────┐                                │
│            │ alice        │                                │
│            └──────────────┘                                │
│                                                             │
│                                    [Continue →]            │
└─────────────────────────────────────────────────────────────┘
```

**Step 3: Add Device**
```
┌─────────────────────────────────────────────────────────────┐
│  Connect your device                                        │
│                                                             │
│  1. Download Termius (free SSH app)                        │
│     [App Store]  [Google Play]                             │
│                                                             │
│  2. In Termius:                                            │
│     Settings → SSH Key → Share → Copy ssh.id code         │
│                                                             │
│  3. Enter your code:                                       │
│     ┌────────────────────────────────────┐                 │
│     │ oaftobark                          │                 │
│     └────────────────────────────────────┘                 │
│                                                             │
│                                    [Add Device →]          │
└─────────────────────────────────────────────────────────────┘
```

**Step 4: Done**
```
┌─────────────────────────────────────────────────────────────┐
│  ✓ You're all set!                                         │
│                                                             │
│  In Termius, create a new host:                            │
│                                                             │
│    Host: alice.spoq.dev                                    │
│    User: alice                                             │
│                                                             │
│  Connect and run: spoq                                     │
│                                                             │
│  [Open Termius]  [Go to Dashboard]                         │
└─────────────────────────────────────────────────────────────┘
```

### 4. Dashboard (spoq.dev/dashboard)
- Account status
- Manage profiles
- Manage devices
- Usage stats
- Billing

## SSH Key Flow (ssh.id)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  HOW ssh.id INTEGRATION WORKS                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. User opens Termius, goes to Settings → SSH Key → Share                 │
│     └── Gets code like "oaftobark"                                         │
│                                                                              │
│  2. User enters code in web app                                             │
│                                                                              │
│  3. Our API fetches the public key:                                         │
│     curl -fs https://sshid.io/oaftobark                                    │
│     └── Returns: ssh-ed25519 AAAAC3... user@device                         │
│                                                                              │
│  4. API adds key to user's container:                                       │
│     POST /internal/containers/:id/add-key                                  │
│     └── Appends to /home/alice/.ssh/authorized_keys                        │
│                                                                              │
│  5. User can now SSH from that device                                       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## DNS Configuration

```
# Cloudflare DNS (or similar)

# Wildcard for user subdomains
*.spoq.dev    A       <load-balancer-ip>

# API and web
spoq.dev      A       <web-server-ip>
api.spoq.dev  CNAME   spoq.dev

# With Cloudflare proxy for DDoS protection
```

## Routing (Ingress)

Using nginx or Traefik to route SSH connections:

```yaml
# Traefik configuration example
entryPoints:
  ssh:
    address: ":22"

tcp:
  routers:
    ssh-router:
      entryPoints:
        - ssh
      rule: "HostSNI(`*`)"
      service: ssh-service
      tls:
        passthrough: true

  services:
    ssh-service:
      loadBalancer:
        servers:
          - address: "container-ip:22"
```

Alternative: Use SSH jump host that routes based on username prefix.

## Billing Integration (Stripe)

```typescript
// Subscription plans
const plans = {
  monthly: {
    id: 'price_monthly_xxx',
    amount: 1500, // $15/month
    interval: 'month'
  },
  yearly: {
    id: 'price_yearly_xxx',
    amount: 15000, // $150/year (2 months free)
    interval: 'year'
  }
};

// Webhook handlers
app.post('/webhooks/stripe', async (req, res) => {
  const event = stripe.webhooks.constructEvent(...);

  switch (event.type) {
    case 'customer.subscription.created':
      await activateContainer(event.data.object.metadata.user_id);
      break;
    case 'customer.subscription.deleted':
      await suspendContainer(event.data.object.metadata.user_id);
      break;
    case 'invoice.payment_failed':
      await notifyPaymentFailed(event.data.object.metadata.user_id);
      break;
  }
});
```

## Monitoring

```yaml
# Metrics to track
- container_cpu_usage
- container_memory_usage
- container_storage_usage
- ssh_connections_active
- ssh_connections_total
- api_requests_total
- api_latency_seconds

# Alerts
- Container down > 5 minutes
- CPU usage > 90% sustained
- Storage > 80% full
- Payment failed
```

## Security Considerations

1. **SSH Key Only**: No password authentication
2. **Container Isolation**: Each user in separate container
3. **Network Isolation**: Containers can't communicate with each other
4. **Rate Limiting**: API and SSH connection limits
5. **Audit Logging**: Track all administrative actions
6. **Encryption**: TLS for web, SSH for terminal
7. **Backups**: Daily encrypted backups of user data

## Cost Estimation

| Component | Monthly Cost |
|-----------|--------------|
| VPS (per 10 users) | $40 |
| Database | $15 |
| Load Balancer | $10 |
| Bandwidth | ~$20 |
| Domain/DNS | ~$2 |
| **Per User Cost** | ~$8 |
| **Price Point** | $15/month |
| **Margin** | ~47% |

## Implementation Phases

### Phase 1: MVP (2-3 weeks)
- [ ] Basic web app (Next.js)
- [ ] User registration
- [ ] Single container deployment
- [ ] ssh.id integration
- [ ] Manual subdomain setup

### Phase 2: Automation (2 weeks)
- [ ] Automatic container provisioning
- [ ] Subdomain routing
- [ ] Multi-profile support
- [ ] Device management UI

### Phase 3: Polish (2 weeks)
- [ ] Stripe integration
- [ ] Dashboard with usage stats
- [ ] Monitoring and alerts
- [ ] Documentation

### Phase 4: Scale (Ongoing)
- [ ] Multi-node deployment
- [ ] Auto-scaling
- [ ] Geographic distribution
- [ ] Performance optimization
