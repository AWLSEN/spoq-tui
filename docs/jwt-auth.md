# JWT Authentication

## Overview

The SPOQ TUI authenticates with the backend using JWT Bearer tokens for both REST API and WebSocket connections.

## Backend URLs

| Service | URL |
|---------|-----|
| REST API | `http://100.85.185.33:8000` |
| WebSocket | `ws://100.85.185.33:8000/ws` |

## Authentication Methods

### REST API (HTTP)
All requests include the `Authorization` header:
```
Authorization: Bearer <JWT_TOKEN>
```

### WebSocket
Token is passed as a query parameter:
```
ws://100.85.185.33:8000/ws?token=<JWT_TOKEN>
```

## Token Sources

### 1. Credentials File (Production)
Location: `~/.spoq/credentials.json`

```json
{
  "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "refresh_token": "spoq_...",
  "expires_at": 1800439669,
  "user_id": "dev-user",
  "vps_url": "http://100.85.185.33:8000",
  "vps_status": "ready"
}
```

The app automatically loads and uses `access_token` from this file.

### 2. Environment Variable (Development)
Set `SPOQ_DEV_TOKEN` to bypass the credentials file:

```bash
export SPOQ_DEV_TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
cargo run
```

Or inline:
```bash
SPOQ_DEV_TOKEN="<token>" cargo run
```

## Dev Token (for local testing)

```
eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJkZXYtdXNlciIsImV4cCI6MjA4NDI5MDY4MSwiaWF0IjoxNzY4OTMwNjgxfQ.ym2C0QhyeVkB-0ozmpdp-ZDFC72oZU8hGo6kkIM4PA8
```

**Payload:**
```json
{
  "sub": "dev-user",
  "exp": 2084290681,
  "iat": 1768930681
}
```

Expires: ~2036 (long-lived dev token)

## Token from credentials.json

```
eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJkZXYtdXNlciIsImlhdCI6MTc2ODkwMzY2OSwiZXhwIjoxODAwNDM5NjY5fQ.N8WUSKmO27CEr-Ew5VfpUv317CSTFHZc-upP7Zh9dkA
```

**Payload:**
```json
{
  "sub": "dev-user",
  "iat": 1768903669,
  "exp": 1800439669
}
```

Expires: ~2027

## Testing with curl

```bash
# Get threads
curl -H "Authorization: Bearer <TOKEN>" http://100.85.185.33:8000/v1/threads

# Health check
curl http://100.85.185.33:8000/v1/health

# Stream (POST)
curl -X POST \
  -H "Authorization: Bearer <TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{"prompt":"Hello","session_id":"test-123"}' \
  http://100.85.185.33:8000/v1/stream
```

## Code References

- Token loading: `src/auth/credentials.rs`
- HTTP auth header: `src/conductor.rs:146` (`add_auth_header`)
- WebSocket auth: `src/websocket/client.rs:97` (query param)
- Dev token env var: `SPOQ_DEV_TOKEN`
