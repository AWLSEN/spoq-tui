# ORB Memory Architecture: 8GB RAM → 100GB+ Effective Memory

**Hardware Target:** Raspberry Pi 5 / Jetson Orin Nano
**Physical RAM:** 8GB DDR4/LPDDR5
**Storage:** 256GB-512GB NVMe SSD
**Goal:** Run 20-50 concurrent background agents 24/7

---

## The ORB Constraint: 8GB RAM, Unlimited Time

**Unlike a laptop**, the ORB has unique properties:

| Property | Laptop | ORB |
|----------|--------|-----|
| **Uptime** | Hours (user sessions) | 24/7 (always on) |
| **User presence** | Required | Not required |
| **RAM** | 16-32GB | 8GB (fixed) |
| **Storage** | 500GB-2TB SSD | 256-512GB NVMe |
| **Workload** | Interactive | Background batch |
| **Priority** | Latency | Throughput |

**Key insight:** The ORB can trade latency for capacity because agents run for HOURS unattended.

---

## The SSD-as-RAM Strategy

### Why This Works for Background Agents

Traditional swap is terrible for interactive workloads (latency kills UX). But for BACKGROUND agents working overnight:

```
Agent workflow:
1. User: "Fix these 5 bugs" (10pm)
2. Agent 1-5 spawn, start working
3. User goes to sleep
4. Agents work for 3-6 hours (user doesn't care about latency!)
5. User wakes up (6am) → 5 PRs ready

Total work: 6 hours
Acceptable SSD read latency: 100μs (user is ASLEEP!)
```

**The ORB can use SSD as extended RAM because the user isn't waiting.**

---

## Architecture: Three-Tier Memory System

```
┌────────────────────────────────────────────────────────┐
│                    8GB Physical RAM                     │
├────────────────────────────────────────────────────────┤
│                                                         │
│  Tier 0: HOT (2GB)                                     │
│  ├─ Active agent conversations (currently processing)  │
│  ├─ Conductor runtime                                  │
│  └─ System/tools (shared, never evicted)               │
│                                                         │
│  Tier 1: WARM (4GB compressed → 20GB effective)       │
│  ├─ Recent agent contexts (last 1 hour)               │
│  ├─ zswap compressed pool                             │
│  └─ Decompressed on access (1-10μs)                   │
│                                                         │
│  Tier 2: COLD (2GB working set)                        │
│  ├─ Least recently used agents                        │
│  ├─ Backed by SSD swap                                │
│  └─ 100μs latency (acceptable for background)         │
│                                                         │
└────────────────────────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────┐
│              256GB NVMe SSD Storage                     │
├────────────────────────────────────────────────────────┤
│                                                         │
│  • Swap partition: 64GB (for cold agent contexts)      │
│  • Agent state snapshots: 32GB                         │
│  • Conductor database: 16GB                            │
│  • System/logs: 16GB                                   │
│  • Free space: 128GB                                   │
│                                                         │
└────────────────────────────────────────────────────────┘
```

---

## How 8GB Becomes 100GB+ Effective

### Math for 50 Concurrent Agents

```rust
// Traditional approach (won't fit!)
struct Agent {
    system_prompt: 50 MB,
    tools: 100 MB,
    conversation: 200 MB,  // Growing over hours
}

50 agents × 350 MB = 17.5 GB RAM needed ❌ (only have 8GB!)

// ORB approach (DOES fit!)
struct OrbMemory {
    // Tier 0: HOT (always in DDR)
    system_prompt_shared: 50 MB × 1 = 50 MB,    // Deduplicated!
    tools_shared: 100 MB × 1 = 100 MB,           // Deduplicated!
    conductor_runtime: 200 MB,
    active_agent_workset: 1.65 GB,               // 3-5 agents actively processing

    // Tier 1: WARM (compressed 5:1 in zswap)
    warm_contexts: 20 GB uncompressed → 4 GB compressed,

    // Tier 2: COLD (on SSD, paged in when needed)
    cold_contexts: 80 GB on SSD (40-45 agents waiting/idle),
}

Total physical RAM used: 2 + 4 + 2 = 8 GB ✓
Total effective capacity: 2 + 20 + 80 = 102 GB ✓
```

**Reduction: 17.5GB → 8GB physical (12× less), but 102GB effective capacity!**

---

## Implementation: Conductor-Specific Optimizations

### 1. Content-Addressed Agent State

Every agent shares the same system prompt and tools:

```rust
// In conductor/src/agent_manager.rs
struct AgentMemoryManager {
    // Shared immutable data (loaded once, never duplicated)
    system_prompt: Arc<Vec<u8>>,  // 50MB, shared by all agents
    tools: Arc<Vec<Tool>>,          // 100MB, shared by all agents

    // Per-agent mutable state (stored deduplicated + compressed)
    agents: HashMap<AgentId, AgentState>,
}

struct AgentState {
    conversation: CompressedBuffer,  // zstd compressed, ~5:1 ratio
    last_access: Instant,
    tier: MemoryTier,  // Hot, Warm, or Cold
}

enum MemoryTier {
    Hot,   // In active DDR, being processed NOW
    Warm,  // In zswap, accessed in last hour
    Cold,  // On SSD, older than 1 hour
}
```

### 2. Intelligent Agent Scheduling

**Only keep ACTIVE agents in hot memory:**

```rust
impl Conductor {
    async fn schedule_agents(&mut self) {
        // Find agents that need CPU time
        let active_agents: Vec<_> = self.agents
            .iter()
            .filter(|a| a.state == AgentState::WaitingForCpu)
            .take(5)  // Max 5 concurrent active agents
            .collect();

        for agent in active_agents {
            // Promote to HOT tier (load from SSD if needed)
            self.memory.promote_to_hot(agent.id).await;

            // Run agent for quantum (10-30 seconds)
            agent.run_quantum().await;

            // If still working, keep HOT
            // If waiting for LLM response, demote to WARM
            if agent.state == AgentState::WaitingForLlm {
                self.memory.demote_to_warm(agent.id);
            }
        }
    }
}
```

**Key insight:** Most agents are WAITING (for LLM response, tool execution, etc.), not actively using CPU. Keep only the ~5 actively working agents in HOT tier.

### 3. zswap Configuration for ORB

```bash
#!/bin/bash
# /opt/spoq/scripts/setup-orb-memory.sh

echo "Configuring ORB memory system..."

# Enable zswap with ARM-optimized settings
echo zstd > /sys/module/zswap/parameters/compressor  # Best compression
echo zsmalloc > /sys/module/zswap/parameters/zpool   # Efficient allocator
echo 50 > /sys/module/zswap/parameters/max_pool_percent  # 4GB of 8GB
echo 1 > /sys/module/zswap/parameters/enabled

# Configure swap on NVMe SSD
if [ ! -f /swapfile ]; then
    fallocate -l 64G /swapfile
    chmod 600 /swapfile
    mkswap /swapfile
    swapon /swapfile
fi

# Tune for background workloads (aggressive swapping is OK!)
sysctl -w vm.swappiness=100             # Swap aggressively
sysctl -w vm.vfs_cache_pressure=50      # Less VFS cache, more for agents
sysctl -w vm.dirty_ratio=10             # Flush to SSD regularly
sysctl -w vm.watermark_scale_factor=500 # Reclaim earlier

echo "ORB memory system ready:"
echo "- Physical RAM: 8GB"
echo "- zswap compressed: 4GB (20GB effective)"
echo "- SSD swap: 64GB"
echo "- Total effective: ~80GB+"
```

### 4. Agent State Persistence

**Save agent state to SSD periodically:**

```rust
// In conductor/src/persistence.rs
impl AgentPersistence {
    async fn snapshot_cold_agents(&self) {
        for agent in self.agents.iter() {
            if agent.tier == MemoryTier::Cold {
                // Serialize to disk
                let state = bincode::serialize(&agent.state)?;
                let compressed = zstd::encode_all(&state[..], 3)?;

                tokio::fs::write(
                    format!("/var/lib/conductor/agents/{}.state", agent.id),
                    compressed
                ).await?;

                // Can now evict from RAM completely
                self.memory.evict(agent.id);
            }
        }
    }

    async fn restore_agent(&self, id: AgentId) -> Result<Agent> {
        // Read from disk
        let compressed = tokio::fs::read(
            format!("/var/lib/conductor/agents/{}.state", id)
        ).await?;

        let state = zstd::decode_all(&compressed[..])?;
        let agent: AgentState = bincode::deserialize(&state)?;

        Ok(agent)
    }
}
```

---

## Real-World Performance

### Scenario: Overnight Bug Fixing

```
8pm: User queues 20 bugs to fix
     ├─ 20 agents spawn
     ├─ Memory usage: 2GB (all share system prompt/tools)
     └─ All agents in WARM tier (compressed)

9pm: First 5 agents promoted to HOT
     ├─ Start working on bugs 1-5
     └─ Memory usage: 3.5GB (5 active + compressed others)

10pm: Agents 1-2 finish
     ├─ PRs created, demoted to COLD (saved to SSD)
     ├─ Agents 6-7 promoted to HOT
     └─ Memory usage: 3.5GB (steady state)

2am: Agents 3-10 finished
     ├─ All saved to SSD
     ├─ Agents 11-15 now HOT
     └─ Memory usage: 3GB

6am: All 20 agents complete
     ├─ Final memory: 1.5GB (all in COLD/SSD)
     ├─ User wakes up: 20 PRs ready!
     └─ Peak memory: 3.5GB (never exceeded!)
```

**Result: 20 concurrent multi-hour tasks on 8GB RAM device.**

---

## Why This is PERFECT for the ORB

### 1. Background Workload Tolerates Latency

| Workload Type | Latency Sensitivity | ORB Agents |
|---------------|---------------------|------------|
| Interactive (laptop) | HIGH (user waiting) | N/A |
| Background (ORB) | LOW (user sleeping) | ✓ Perfect |

**SSD latency (100μs) is INVISIBLE when user is asleep.**

### 2. Agents Naturally Tier Themselves

```
Agent lifecycle:
1. Spawn → COLD (waiting in queue)
2. Selected for execution → WARM (loading context)
3. Start processing → HOT (active work)
4. Waiting for LLM response → WARM (compress conversation)
5. Resume processing → HOT (decompress)
6. Complete → COLD (save to SSD, evict from RAM)
```

**Natural temporal locality means hot/warm/cold tiers work perfectly.**

### 3. Raspberry Pi / Jetson are FAST at Sequential I/O

```
Raspberry Pi 5 (NVMe):
├─ Sequential read: 450 MB/s
├─ Sequential write: 400 MB/s
└─ Random read (for paging): 40-50k IOPS

Jetson Orin Nano (NVMe):
├─ Sequential read: 3,500 MB/s
├─ Sequential write: 3,000 MB/s
└─ Random read: 180k IOPS
```

**Loading a 200MB agent conversation from SSD:**
- Raspberry Pi: 450ms (acceptable!)
- Jetson: 57ms (feels instant!)

---

## Implementation Roadmap for Conductor

### Week 1-2: Basic Tiering

```rust
// conductor/src/memory_tier.rs
pub struct TieredMemory {
    hot: HashMap<AgentId, AgentContext>,     // Active agents
    warm: LruCache<AgentId, Compressed>,     // Recent (zstd compressed)
    cold_index: HashMap<AgentId, PathBuf>,   // SSD-backed (path to file)
}

impl TieredMemory {
    pub async fn get_agent(&mut self, id: AgentId) -> AgentContext {
        // Check tiers in order
        if let Some(ctx) = self.hot.get(&id) {
            return ctx.clone();
        }

        if let Some(compressed) = self.warm.get(&id) {
            let ctx = decompress(compressed);
            self.hot.insert(id, ctx.clone());  // Promote to HOT
            return ctx;
        }

        // Load from SSD
        let path = self.cold_index.get(&id).unwrap();
        let ctx = self.load_from_disk(path).await;
        self.hot.insert(id, ctx.clone());  // Promote to HOT
        ctx
    }
}
```

### Week 3-4: Agent Scheduler Integration

```rust
// conductor/src/scheduler.rs
impl AgentScheduler {
    async fn run_agent(&mut self, id: AgentId) {
        // Get agent context (auto-promoted from tier)
        let mut agent = self.memory.get_agent(id).await;

        // Run for quantum
        agent.execute_quantum().await;

        // Decide tier based on state
        match agent.state {
            AgentState::Active => {
                // Keep in HOT
                self.memory.keep_hot(id, agent);
            }
            AgentState::WaitingForLlm => {
                // Demote to WARM (compress)
                self.memory.demote_to_warm(id, agent);
            }
            AgentState::Complete => {
                // Save to SSD, evict from RAM
                self.memory.demote_to_cold(id, agent).await;
            }
        }
    }
}
```

### Week 5-6: System-Level Integration

```bash
# Install as systemd service
cat > /etc/systemd/system/conductor.service <<EOF
[Unit]
Description=SPOQ Conductor - Background AI Orchestrator
After=network.target

[Service]
Type=simple
User=conductor
Environment="RUST_LOG=info"
ExecStartPre=/opt/spoq/scripts/setup-orb-memory.sh
ExecStart=/opt/spoq/bin/conductor --config /etc/conductor/config.toml
Restart=always
RestartSec=10

# Memory limits (allow using full 8GB + swap)
MemoryMax=8G
MemorySwapMax=64G

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable conductor
systemctl start conductor
```

---

## Monitoring & Observability

### Dashboard Metrics

```rust
pub struct OrbMemoryStats {
    // Physical RAM
    pub ram_total: u64,        // 8GB
    pub ram_used: u64,         // Current usage
    pub ram_available: u64,

    // Memory tiers
    pub hot_bytes: u64,        // Active agents
    pub warm_bytes_compressed: u64,   // zswap pool
    pub warm_bytes_uncompressed: u64, // Effective capacity
    pub cold_bytes: u64,       // SSD-backed

    // Compression stats
    pub compression_ratio: f64,  // Typical: 5:1 for conversations
    pub zswap_hit_rate: f64,     // Cache hit rate

    // Agent stats
    pub agents_total: u32,
    pub agents_hot: u32,       // Currently processing
    pub agents_warm: u32,      // In compressed pool
    pub agents_cold: u32,      // On SSD
}
```

**Example output:**

```
SPOQ ORB Memory Dashboard
========================
Physical RAM: 7.2 / 8.0 GB (90%)
  ├─ HOT tier:  2.1 GB (5 active agents)
  ├─ WARM tier: 4.0 GB compressed (19.8 GB effective, 4.95:1 ratio)
  └─ System:    1.1 GB (conductor + OS)

SSD Swap: 12.4 / 64 GB (19%)
  └─ COLD tier: 12.4 GB (32 hibernated agents)

Agents:
  ├─ Total: 42
  ├─ Active (HOT): 5
  ├─ Recent (WARM): 15
  └─ Hibernated (COLD): 22

Performance:
  ├─ zswap hit rate: 94%
  ├─ SSD page-in: 840 MB/hour
  └─ Avg agent restore: 320ms
```

---

## The Magic: Real Capacity Numbers

| Config | Physical RAM | Effective Capacity | Max Concurrent Agents |
|--------|--------------|-------------------|----------------------|
| **Basic** (no optimization) | 8GB | 8GB | ~5 agents |
| **+ Deduplication** | 8GB | 12GB | ~10 agents |
| **+ zswap (50%)** | 8GB | 32GB | ~25 agents |
| **+ SSD tier** | 8GB | 80GB+ | **50+ agents** |

**12× capacity increase from 8GB physical RAM!**

---

## Advantages for ORB vs. Laptop

| Aspect | Laptop (Bad) | ORB (Perfect) |
|--------|--------------|---------------|
| **User impact** | User waiting → latency critical | User sleeping → latency irrelevant |
| **Uptime** | Hours → can't finish long tasks | 24/7 → can run for days |
| **Power** | Battery drain → SSD thrashing bad | Plugged in → SSD usage free |
| **Workload** | Interactive → swap = slow UX | Batch → swap = more capacity |
| **Priorities** | Low latency | High throughput |

**The ORB's constraints are actually ADVANTAGES for this architecture!**

---

## Next Steps

1. **Validate with prototype** (2 weeks)
   - Implement basic tiering in conductor
   - Run 20 agents overnight on Raspberry Pi 5
   - Measure memory usage

2. **Optimize compression** (1 week)
   - Tune zstd compression levels
   - Benchmark ARM vs x86 performance
   - Test with real Claude Code conversations

3. **Production hardening** (2 weeks)
   - Add crash recovery (agent state persisted)
   - Implement progressive eviction
   - Stress test with 50 agents

4. **Ship it** 🚀
   - Package as conductor update
   - Document configuration for ORB users
   - Monitor real-world usage

---

## Conclusion

**The ORB is PERFECT for this memory architecture because:**

1. ✅ Background workload tolerates SSD latency
2. ✅ 24/7 uptime enables long-running tasks
3. ✅ Agents naturally tier (active → waiting → complete)
4. ✅ NVMe SSDs are fast enough (esp. on Jetson)
5. ✅ Users care about CAPACITY, not LATENCY

**With this architecture:**
- 8GB RAM → 80GB+ effective
- 5 agents max → 50+ agents concurrent
- ORB Lite viable → No need to force ORB Pro

**This turns the ORB's limitation (8GB RAM) into a feature: "Runs 50 agents on 8GB RAM."**

Let's build it.
