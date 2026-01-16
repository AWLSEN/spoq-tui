//! HTML dashboard for the debug server.
//!
//! This module contains the single-file HTML dashboard that provides a real-time
//! view of debug events via WebSocket connection.

/// HTML dashboard for the debug server.
///
/// This single-file HTML dashboard provides a real-time view of debug events
/// via WebSocket connection. It includes event viewing, statistics, state
/// inspection, and clipboard-based export functionality.
pub const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SPOQ Debug Dashboard</title>
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #1a1a2e;
            color: #eee;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }

        /* Header */
        .header {
            background: #16213e;
            padding: 12px 20px;
            display: flex;
            align-items: center;
            justify-content: space-between;
            border-bottom: 1px solid #0f3460;
        }

        .header-left {
            display: flex;
            align-items: center;
            gap: 16px;
        }

        .title {
            font-size: 18px;
            font-weight: 600;
            color: #e94560;
        }

        .connection-status {
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 13px;
            color: #aaa;
        }

        .status-dot {
            width: 10px;
            height: 10px;
            border-radius: 50%;
            background: #e94560;
            transition: background 0.3s;
        }

        .status-dot.connected {
            background: #4ade80;
        }

        .header-buttons {
            display: flex;
            gap: 8px;
        }

        button {
            background: #0f3460;
            color: #eee;
            border: 1px solid #1a4a7a;
            padding: 8px 14px;
            border-radius: 4px;
            cursor: pointer;
            font-size: 13px;
            transition: all 0.2s;
        }

        button:hover {
            background: #1a4a7a;
            border-color: #2a5a8a;
        }

        button:active {
            transform: scale(0.98);
        }

        button.primary {
            background: #e94560;
            border-color: #e94560;
        }

        button.primary:hover {
            background: #d63850;
        }

        /* Filter Bar */
        .filter-bar {
            background: #16213e;
            padding: 10px 20px;
            display: flex;
            gap: 16px;
            border-bottom: 1px solid #0f3460;
        }

        .filter-group {
            display: flex;
            align-items: center;
            gap: 8px;
        }

        .filter-label {
            font-size: 12px;
            color: #888;
        }

        select {
            background: #0f3460;
            color: #eee;
            border: 1px solid #1a4a7a;
            padding: 6px 10px;
            border-radius: 4px;
            font-size: 13px;
            cursor: pointer;
        }

        select:focus {
            outline: none;
            border-color: #e94560;
        }

        /* Main Content */
        .main-content {
            flex: 1;
            display: grid;
            grid-template-columns: 1fr 1fr 280px;
            grid-template-rows: 1fr auto;
            gap: 1px;
            background: #0f3460;
            overflow: hidden;
        }

        .panel {
            background: #1a1a2e;
            display: flex;
            flex-direction: column;
            overflow: hidden;
        }

        .panel-header {
            background: #16213e;
            padding: 10px 14px;
            font-size: 12px;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            color: #888;
            border-bottom: 1px solid #0f3460;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }

        .panel-content {
            flex: 1;
            overflow-y: auto;
            padding: 10px;
        }

        /* Events Panel */
        .event-item {
            font-family: 'SF Mono', 'Monaco', 'Inconsolata', monospace;
            font-size: 11px;
            padding: 8px 10px;
            margin-bottom: 4px;
            border-radius: 4px;
            background: #16213e;
            border-left: 3px solid #555;
        }

        .event-item.content { border-left-color: #4ade80; }
        .event-item.error { border-left-color: #ef4444; background: #2a1a1a; }
        .event-item.tool_call_start,
        .event-item.tool_call_end,
        .event-item.tool_result { border-left-color: #3b82f6; }
        .event-item.subagent_start,
        .event-item.subagent_end { border-left-color: #10b981; }
        .event-item.stream_lifecycle { border-left-color: #f59e0b; }
        .event-item.state_change { border-left-color: #8b5cf6; }
        .event-item.processed_event { border-left-color: #06b6d4; }

        .event-time {
            color: #666;
            font-size: 10px;
        }

        .event-type {
            color: #e94560;
            font-weight: 600;
            margin-left: 8px;
        }

        .event-payload {
            margin-top: 6px;
            color: #aaa;
            word-break: break-all;
            white-space: pre-wrap;
            max-height: 150px;
            overflow: hidden;
        }

        .event-payload.expanded {
            max-height: none;
        }

        /* Token Flow Panel */
        .token-section {
            margin-bottom: 16px;
        }

        .token-label {
            font-size: 11px;
            color: #888;
            margin-bottom: 6px;
            text-transform: uppercase;
        }

        .token-value {
            font-family: 'SF Mono', 'Monaco', 'Inconsolata', monospace;
            font-size: 13px;
            color: #eee;
            background: #16213e;
            padding: 8px 10px;
            border-radius: 4px;
            word-break: break-all;
        }

        .token-content {
            max-height: 200px;
            overflow-y: auto;
            white-space: pre-wrap;
        }

        .progress-bar {
            height: 8px;
            background: #0f3460;
            border-radius: 4px;
            overflow: hidden;
            margin-top: 8px;
        }

        .progress-fill {
            height: 100%;
            background: linear-gradient(90deg, #e94560, #f59e0b);
            width: 0%;
            transition: width 0.3s;
        }

        .collapsible-header {
            cursor: pointer;
            display: flex;
            align-items: center;
            gap: 6px;
        }

        .collapsible-header::before {
            content: '\25B6';
            font-size: 10px;
            transition: transform 0.2s;
        }

        .collapsible-header.expanded::before {
            transform: rotate(90deg);
        }

        .collapsible-content {
            display: none;
            margin-top: 8px;
        }

        .collapsible-content.expanded {
            display: block;
        }

        /* Statistics Panel */
        .stat-item {
            display: flex;
            justify-content: space-between;
            padding: 8px 0;
            border-bottom: 1px solid #0f3460;
        }

        .stat-item:last-child {
            border-bottom: none;
        }

        .stat-label {
            font-size: 12px;
            color: #888;
        }

        .stat-value {
            font-family: 'SF Mono', 'Monaco', 'Inconsolata', monospace;
            font-size: 13px;
            font-weight: 600;
            color: #4ade80;
        }

        .stat-group {
            margin-bottom: 16px;
        }

        .stat-group-title {
            font-size: 11px;
            color: #666;
            text-transform: uppercase;
            margin-bottom: 8px;
            padding-bottom: 4px;
            border-bottom: 1px solid #0f3460;
        }

        /* State Inspector */
        .state-inspector {
            grid-column: 1 / -1;
            max-height: 200px;
        }

        .state-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 12px;
        }

        .state-card {
            background: #16213e;
            padding: 12px;
            border-radius: 4px;
        }

        .state-card-title {
            font-size: 11px;
            color: #888;
            text-transform: uppercase;
            margin-bottom: 8px;
        }

        .state-card-value {
            font-family: 'SF Mono', 'Monaco', 'Inconsolata', monospace;
            font-size: 12px;
            color: #eee;
        }

        /* Auto-scroll toggle */
        .toggle-container {
            display: flex;
            align-items: center;
            gap: 6px;
        }

        .toggle {
            width: 36px;
            height: 20px;
            background: #0f3460;
            border-radius: 10px;
            position: relative;
            cursor: pointer;
            transition: background 0.2s;
        }

        .toggle.active {
            background: #4ade80;
        }

        .toggle::after {
            content: '';
            position: absolute;
            width: 16px;
            height: 16px;
            background: #eee;
            border-radius: 50%;
            top: 2px;
            left: 2px;
            transition: transform 0.2s;
        }

        .toggle.active::after {
            transform: translateX(16px);
        }

        .toggle-label {
            font-size: 11px;
            color: #888;
        }

        /* Toast notification */
        .toast {
            position: fixed;
            bottom: 20px;
            right: 20px;
            background: #16213e;
            color: #eee;
            padding: 12px 20px;
            border-radius: 6px;
            border: 1px solid #4ade80;
            opacity: 0;
            transform: translateY(20px);
            transition: all 0.3s;
            z-index: 1000;
        }

        .toast.show {
            opacity: 1;
            transform: translateY(0);
        }

        /* Scrollbar */
        ::-webkit-scrollbar {
            width: 8px;
            height: 8px;
        }

        ::-webkit-scrollbar-track {
            background: #1a1a2e;
        }

        ::-webkit-scrollbar-thumb {
            background: #0f3460;
            border-radius: 4px;
        }

        ::-webkit-scrollbar-thumb:hover {
            background: #1a4a7a;
        }
    </style>
</head>
<body>
    <div class="header">
        <div class="header-left">
            <div class="title">SPOQ Debug Dashboard</div>
            <div class="connection-status">
                <div class="status-dot" id="statusDot"></div>
                <span id="connectionText">Disconnected</span>
            </div>
        </div>
        <div class="header-buttons">
            <button class="primary" onclick="copyDebugReport()">COPY DEBUG REPORT</button>
            <button onclick="copyError()">Copy Error</button>
            <button onclick="copyState()">Copy State</button>
            <button onclick="clearAll()">Clear All</button>
        </div>
    </div>

    <div class="filter-bar">
        <div class="filter-group">
            <span class="filter-label">Event Type:</span>
            <select id="eventTypeFilter" onchange="applyFilters()">
                <option value="all">All Events</option>
                <option value="raw_sse_event">Raw SSE</option>
                <option value="processed_event">Processed</option>
                <option value="state_change">State Change</option>
                <option value="stream_lifecycle">Lifecycle</option>
                <option value="error">Error</option>
            </select>
        </div>
        <div class="filter-group">
            <span class="filter-label">Thread ID:</span>
            <select id="threadFilter" onchange="applyFilters()">
                <option value="all">All Threads</option>
            </select>
        </div>
    </div>

    <div class="main-content">
        <!-- Events Panel -->
        <div class="panel">
            <div class="panel-header">
                <span>Raw SSE Events</span>
                <div class="toggle-container">
                    <span class="toggle-label">Auto-scroll</span>
                    <div class="toggle active" id="autoScrollToggle" onclick="toggleAutoScroll()"></div>
                </div>
            </div>
            <div class="panel-content" id="eventsPanel"></div>
        </div>

        <!-- Token Flow Panel -->
        <div class="panel">
            <div class="panel-header">Token Flow</div>
            <div class="panel-content" id="tokenPanel">
                <div class="token-section">
                    <div class="token-label">Current Thread</div>
                    <div class="token-value" id="currentThread">-</div>
                </div>
                <div class="token-section">
                    <div class="token-label">Accumulated Content</div>
                    <div class="token-value token-content" id="accumulatedContent">-</div>
                </div>
                <div class="token-section">
                    <div class="token-label">Token Progress</div>
                    <div class="token-value">
                        <span id="tokenCount">0</span> tokens
                        <div class="progress-bar">
                            <div class="progress-fill" id="tokenProgress"></div>
                        </div>
                    </div>
                </div>
                <div class="token-section">
                    <div class="collapsible-header" onclick="toggleReasoning()">
                        <span class="token-label">Reasoning Content</span>
                    </div>
                    <div class="collapsible-content" id="reasoningContent">
                        <div class="token-value token-content" id="reasoningText">-</div>
                    </div>
                </div>
            </div>
        </div>

        <!-- Statistics Panel -->
        <div class="panel">
            <div class="panel-header">Statistics</div>
            <div class="panel-content">
                <div class="stat-group">
                    <div class="stat-group-title">Tokens</div>
                    <div class="stat-item">
                        <span class="stat-label">Received</span>
                        <span class="stat-value" id="statTokensReceived">0</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Tokens/sec</span>
                        <span class="stat-value" id="statTokensPerSec">0.0</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Peak Tokens/sec</span>
                        <span class="stat-value" id="statPeakTokensPerSec">0.0</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Avg Latency</span>
                        <span class="stat-value" id="statAvgLatency">0ms</span>
                    </div>
                </div>
                <div class="stat-group">
                    <div class="stat-group-title">Events</div>
                    <div class="stat-item">
                        <span class="stat-label">Content</span>
                        <span class="stat-value" id="statContentEvents">0</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Tool Calls</span>
                        <span class="stat-value" id="statToolEvents">0</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Errors</span>
                        <span class="stat-value" id="statErrorEvents">0</span>
                    </div>
                    <div class="stat-item">
                        <span class="stat-label">Total</span>
                        <span class="stat-value" id="statTotalEvents">0</span>
                    </div>
                </div>
                <div class="stat-group">
                    <div class="stat-group-title">Session</div>
                    <div class="stat-item">
                        <span class="stat-label">Duration</span>
                        <span class="stat-value" id="statDuration">0:00</span>
                    </div>
                </div>
            </div>
        </div>

        <!-- State Inspector -->
        <div class="panel state-inspector">
            <div class="panel-header">State Inspector</div>
            <div class="panel-content">
                <div class="state-grid">
                    <div class="state-card">
                        <div class="state-card-title">Active Thread</div>
                        <div class="state-card-value" id="stateThread">-</div>
                    </div>
                    <div class="state-card">
                        <div class="state-card-title">Streaming Status</div>
                        <div class="state-card-value" id="stateStreaming">Idle</div>
                    </div>
                    <div class="state-card">
                        <div class="state-card-title">Active Tools</div>
                        <div class="state-card-value" id="stateTools">0</div>
                    </div>
                    <div class="state-card">
                        <div class="state-card-title">Active Subagents</div>
                        <div class="state-card-value" id="stateSubagents">0</div>
                    </div>
                    <div class="state-card">
                        <div class="state-card-title">Cache Summary</div>
                        <div class="state-card-value" id="stateCache">Empty</div>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <div class="toast" id="toast">Copied to clipboard!</div>

    <script>
        // State
        let ws = null;
        let events = [];
        let filteredEvents = [];
        let autoScroll = true;
        let sessionStart = Date.now();
        let stats = {
            tokensReceived: 0,
            tokenTimestamps: [],
            latencies: [],
            contentEvents: 0,
            toolEvents: 0,
            errorEvents: 0,
            totalEvents: 0,
            currentTokensPerSecond: 0,
            peakTokensPerSecond: 0
        };
        let state = {
            thread: null,
            streaming: false,
            activeTools: 0,
            activeSubagents: 0,
            accumulatedContent: '',
            reasoningContent: '',
            threadIds: new Set()
        };
        let lastError = null;
        let lastErrorIndex = -1;

        // Connect WebSocket
        function connect() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            const host = window.location.host || 'localhost:3030';
            ws = new WebSocket(protocol + '//' + host + '/ws');

            ws.onopen = function() {
                document.getElementById('statusDot').classList.add('connected');
                document.getElementById('connectionText').textContent = 'Connected';
            };

            ws.onclose = function() {
                document.getElementById('statusDot').classList.remove('connected');
                document.getElementById('connectionText').textContent = 'Disconnected';
                setTimeout(connect, 5000);
            };

            ws.onerror = function() {
                document.getElementById('statusDot').classList.remove('connected');
                document.getElementById('connectionText').textContent = 'Error';
            };

            ws.onmessage = function(e) {
                try {
                    const event = JSON.parse(e.data);
                    handleEvent(event);
                } catch (err) {
                    console.error('Failed to parse event:', err);
                }
            };
        }

        // Handle incoming event
        function handleEvent(event) {
            events.push(event);
            stats.totalEvents++;

            // Track thread IDs
            if (event.thread_id) {
                state.threadIds.add(event.thread_id);
                updateThreadFilter();
            }

            // Update stats based on event type
            const eventType = event.event ? event.event.type : null;
            if (eventType === 'processed_event') {
                // Extract statistics from ProcessedEvent
                const processedEvent = event.event;
                if (processedEvent.message_type === 'StreamToken') {
                    // Use token statistics from the event if available
                    if (processedEvent.token_count !== undefined && processedEvent.token_count !== null) {
                        stats.tokensReceived = processedEvent.token_count;
                    }
                    if (processedEvent.tokens_per_second !== undefined && processedEvent.tokens_per_second !== null) {
                        stats.currentTokensPerSecond = processedEvent.tokens_per_second;
                        // Track peak tokens/second
                        if (stats.peakTokensPerSecond === undefined || processedEvent.tokens_per_second > stats.peakTokensPerSecond) {
                            stats.peakTokensPerSecond = processedEvent.tokens_per_second;
                        }
                    }
                    if (processedEvent.latency_ms !== undefined && processedEvent.latency_ms !== null) {
                        stats.latencies.push(processedEvent.latency_ms);
                        if (stats.latencies.length > 100) stats.latencies.shift();
                    }
                }
            } else if (eventType === 'raw_sse_event') {
                const sseType = event.event.event_type;
                if (sseType === 'content') {
                    stats.contentEvents++;
                    // Accumulate content for display
                    try {
                        const payload = JSON.parse(event.event.payload);
                        if (payload.text) {
                            state.accumulatedContent += payload.text;
                        }
                    } catch (e) {}
                } else if (sseType && sseType.indexOf('tool') !== -1) {
                    stats.toolEvents++;
                    if (sseType === 'tool_call_start') {
                        state.activeTools++;
                    } else if (sseType === 'tool_call_end' || sseType === 'tool_result') {
                        state.activeTools = Math.max(0, state.activeTools - 1);
                    }
                    // Track subagent start/end events
                    if (sseType === 'subagent_start') {
                        state.activeSubagents++;
                    } else if (sseType === 'subagent_end') {
                        state.activeSubagents = Math.max(0, state.activeSubagents - 1);
                    }
                }
            } else if (eventType === 'error') {
                stats.errorEvents++;
                lastError = event;
                lastErrorIndex = events.length - 1;
            } else if (eventType === 'stream_lifecycle') {
                const phase = event.event.phase;
                state.streaming = phase === 'connected' || phase === 'connecting';
                if (phase === 'completed' || phase === 'closed') {
                    state.accumulatedContent = '';
                    state.reasoningContent = '';
                }
            }

            // Update current thread
            if (event.thread_id) {
                state.thread = event.thread_id;
            }

            applyFilters();
            updateUI();
        }

        // Apply filters
        function applyFilters() {
            const typeFilter = document.getElementById('eventTypeFilter').value;
            const threadFilter = document.getElementById('threadFilter').value;

            filteredEvents = events.filter(function(e) {
                if (typeFilter !== 'all' && e.event && e.event.type !== typeFilter) return false;
                if (threadFilter !== 'all' && e.thread_id !== threadFilter) return false;
                return true;
            });

            renderEvents();
        }

        // Update thread filter dropdown
        function updateThreadFilter() {
            const select = document.getElementById('threadFilter');
            const current = select.value;

            // Clear and rebuild
            select.innerHTML = '<option value="all">All Threads</option>';
            state.threadIds.forEach(function(id) {
                const option = document.createElement('option');
                option.value = id;
                option.textContent = id.substring(0, 12) + '...';
                select.appendChild(option);
            });

            // Restore selection
            select.value = current;
        }

        // Render events
        function renderEvents() {
            const panel = document.getElementById('eventsPanel');
            const displayEvents = filteredEvents.slice(-500); // Show last 500

            let html = '';
            for (let i = 0; i < displayEvents.length; i++) {
                const e = displayEvents[i];
                const time = new Date(e.timestamp).toLocaleTimeString();
                let eventType = e.event ? e.event.type : 'unknown';
                let cssClass = eventType;

                // For raw SSE events, use the SSE event type for coloring
                if (eventType === 'raw_sse_event' && e.event && e.event.event_type) {
                    cssClass = e.event.event_type;
                }

                let payload = '';
                if (e.event) {
                    const copy = {};
                    for (const key in e.event) {
                        if (key !== 'type') copy[key] = e.event[key];
                    }
                    payload = JSON.stringify(copy, null, 2);
                }

                const sseType = (e.event && e.event.event_type) ? ': ' + e.event.event_type : '';
                html += '<div class="event-item ' + cssClass + '">' +
                    '<span class="event-time">' + time + '</span>' +
                    '<span class="event-type">' + eventType + sseType + '</span>' +
                    '<div class="event-payload">' + escapeHtml(payload) + '</div>' +
                    '</div>';
            }
            panel.innerHTML = html;

            if (autoScroll) {
                panel.scrollTop = panel.scrollHeight;
            }
        }

        // Update UI
        function updateUI() {
            // Token panel
            document.getElementById('currentThread').textContent = state.thread || '-';
            document.getElementById('accumulatedContent').textContent = state.accumulatedContent || '-';
            document.getElementById('tokenCount').textContent = stats.tokensReceived;

            // Progress bar (arbitrary max of 10000 tokens for visual)
            const progress = Math.min(100, (stats.tokensReceived / 10000) * 100);
            document.getElementById('tokenProgress').style.width = progress + '%';

            // Stats
            document.getElementById('statTokensReceived').textContent = stats.tokensReceived;
            document.getElementById('statContentEvents').textContent = stats.contentEvents;
            document.getElementById('statToolEvents').textContent = stats.toolEvents;
            document.getElementById('statErrorEvents').textContent = stats.errorEvents;
            document.getElementById('statTotalEvents').textContent = stats.totalEvents;

            // Tokens per second (use current rate from statistics)
            if (stats.currentTokensPerSecond > 0) {
                document.getElementById('statTokensPerSec').textContent = stats.currentTokensPerSecond.toFixed(1);
            } else {
                document.getElementById('statTokensPerSec').textContent = '0.0';
            }

            // Peak tokens per second
            if (stats.peakTokensPerSecond > 0) {
                document.getElementById('statPeakTokensPerSec').textContent = stats.peakTokensPerSecond.toFixed(1);
            } else {
                document.getElementById('statPeakTokensPerSec').textContent = '0.0';
            }

            // Average latency
            if (stats.latencies.length > 0) {
                let sum = 0;
                for (let i = 0; i < stats.latencies.length; i++) sum += stats.latencies[i];
                const avg = sum / stats.latencies.length;
                document.getElementById('statAvgLatency').textContent = Math.round(avg) + 'ms';
            } else {
                document.getElementById('statAvgLatency').textContent = '0ms';
            }

            // Duration
            const duration = Math.floor((now - sessionStart) / 1000);
            const mins = Math.floor(duration / 60);
            const secs = duration % 60;
            document.getElementById('statDuration').textContent = mins + ':' + (secs < 10 ? '0' : '') + secs;

            // State inspector
            document.getElementById('stateThread').textContent = state.thread || '-';
            document.getElementById('stateStreaming').textContent = state.streaming ? 'Active' : 'Idle';
            document.getElementById('stateTools').textContent = state.activeTools;
            document.getElementById('stateSubagents').textContent = state.activeSubagents;
            document.getElementById('stateCache').textContent = events.length + ' events';
        }

        // Toggle auto-scroll
        function toggleAutoScroll() {
            autoScroll = !autoScroll;
            const toggle = document.getElementById('autoScrollToggle');
            if (autoScroll) {
                toggle.classList.add('active');
            } else {
                toggle.classList.remove('active');
            }
        }

        // Toggle reasoning section
        function toggleReasoning() {
            const header = document.querySelector('.collapsible-header');
            const content = document.getElementById('reasoningContent');
            header.classList.toggle('expanded');
            content.classList.toggle('expanded');
        }

        // Copy functions
        function copyDebugReport() {
            const now = new Date();
            const duration = Math.floor((Date.now() - sessionStart) / 1000);
            const mins = Math.floor(duration / 60);
            const secs = duration % 60;

            // Calculate avg latency
            let avgLatency = 0;
            if (stats.latencies.length > 0) {
                let sum = 0;
                for (let i = 0; i < stats.latencies.length; i++) sum += stats.latencies[i];
                avgLatency = Math.round(sum / stats.latencies.length);
            }

            // Convert Set to Array for JSON
            const stateForJson = {
                thread: state.thread,
                streaming: state.streaming,
                activeTools: state.activeTools,
                activeSubagents: state.activeSubagents,
                accumulatedContent: state.accumulatedContent,
                reasoningContent: state.reasoningContent,
                threadIds: Array.from(state.threadIds)
            };

            let report = '# SPOQ Debug Report\n';
            report += 'Generated: ' + now.toISOString() + '\n\n';
            report += '## Session Info\n';
            report += '- Duration: ' + mins + 'm ' + secs + 's\n';
            report += '- Current Thread: ' + (state.thread || 'None') + '\n';
            report += '- Streaming: ' + (state.streaming ? 'Yes' : 'No') + '\n';
            report += '- Active Tools: ' + state.activeTools + '\n';
            report += '- Active Subagents: ' + state.activeSubagents + '\n\n';
            report += '## Statistics\n';
            report += '| Metric | Value |\n';
            report += '|--------|-------|\n';
            report += '| Tokens Received | ' + stats.tokensReceived + ' |\n';
            report += '| Content Events | ' + stats.contentEvents + ' |\n';
            report += '| Tool Events | ' + stats.toolEvents + ' |\n';
            report += '| Error Events | ' + stats.errorEvents + ' |\n';
            report += '| Total Events | ' + stats.totalEvents + ' |\n';
            report += '| Avg Latency | ' + avgLatency + 'ms |\n\n';
            report += '## Current State\n';
            report += '```json\n' + JSON.stringify(stateForJson, null, 2) + '\n```\n\n';
            report += '## Last 50 Events\n';
            report += '```json\n' + JSON.stringify(events.slice(-50), null, 2) + '\n```\n';

            if (lastError) {
                report += '\n## Last Error\n';
                report += '```json\n' + JSON.stringify(lastError, null, 2) + '\n```\n';
            }

            copyToClipboard(report);
        }

        function copyError() {
            if (!lastError) {
                showToast('No errors recorded');
                return;
            }

            const start = Math.max(0, lastErrorIndex - 5);
            const end = Math.min(events.length, lastErrorIndex + 6);
            const context = events.slice(start, end);

            const report = {
                error: lastError,
                context: {
                    eventsBefore: context.slice(0, lastErrorIndex - start),
                    eventsAfter: context.slice(lastErrorIndex - start + 1)
                }
            };

            copyToClipboard(JSON.stringify(report, null, 2));
        }

        function copyState() {
            const stateJson = JSON.stringify({
                thread: state.thread,
                streaming: state.streaming,
                activeTools: state.activeTools,
                activeSubagents: state.activeSubagents,
                accumulatedContent: state.accumulatedContent,
                reasoningContent: state.reasoningContent,
                threadIds: Array.from(state.threadIds),
                eventCount: events.length,
                stats: stats
            }, null, 2);

            copyToClipboard(stateJson);
        }

        function copyToClipboard(text) {
            navigator.clipboard.writeText(text).then(function() {
                showToast('Copied to clipboard!');
            }).catch(function(err) {
                console.error('Failed to copy:', err);
                showToast('Failed to copy');
            });
        }

        function showToast(message) {
            const toast = document.getElementById('toast');
            toast.textContent = message;
            toast.classList.add('show');
            setTimeout(function() { toast.classList.remove('show'); }, 2000);
        }

        function clearAll() {
            events = [];
            filteredEvents = [];
            stats = {
                tokensReceived: 0,
                tokenTimestamps: [],
                latencies: [],
                contentEvents: 0,
                toolEvents: 0,
                errorEvents: 0,
                totalEvents: 0,
                currentTokensPerSecond: 0,
                peakTokensPerSecond: 0
            };
            state = {
                thread: null,
                streaming: false,
                activeTools: 0,
                activeSubagents: 0,
                accumulatedContent: '',
                reasoningContent: '',
                threadIds: new Set()
            };
            lastError = null;
            lastErrorIndex = -1;
            sessionStart = Date.now();

            renderEvents();
            updateUI();
            updateThreadFilter();
        }

        function escapeHtml(text) {
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }

        // Update duration every second
        setInterval(updateUI, 1000);

        // Connect on load
        connect();
    </script>
</body>
</html>
"#;
