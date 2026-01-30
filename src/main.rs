use spoq::app::{start_websocket_with_config, App, AppMessage, BrowseListSelectAction, Focus, Screen, ScrollBoundary, UnifiedPickerAction};
use spoq::cli::{parse_args, run_cli_command};
use spoq::credential_watcher::spawn_file_watcher;
use spoq::debug::{DebugEvent, DebugEventKind, StateChangeData, StateType};
use spoq::input::translate_shifted_char;
use spoq::models;
use spoq::models::dashboard::WaitingFor;
use spoq::startup::{run_preflight_checks, StartupConfig};
use spoq::terminal::{setup_panic_hook, TerminalManager};
use spoq::ui;
use spoq::websocket::WsClientConfig;

use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use crossterm::execute;
use futures::StreamExt;
use ratatui::Terminal;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

/// Background update check and download on startup.
///
/// This function runs non-blocking in the background:
/// 1. Load update state to check last check time
/// 2. Check for available updates (respecting rate limiting)
/// 3. Download the update if available
/// 4. Store the pending update path in state for next launch
///
/// Errors are silently ignored to avoid disrupting the user experience.
async fn check_and_download_update() {
    use spoq::update::{check_for_update, detect_platform, download_binary, UpdateStateManager};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Load update state to check when we last checked
    let state_manager = match UpdateStateManager::new() {
        Some(mgr) => mgr,
        None => return, // Can't determine home dir - skip update check
    };

    let mut state = state_manager.load();

    // Rate limit: only check for updates once per 24 hours
    const CHECK_INTERVAL_SECONDS: i64 = 24 * 60 * 60;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    if let Some(last_check) = state.last_check {
        if now - last_check < CHECK_INTERVAL_SECONDS {
            // Too soon since last check - skip
            return;
        }
    }

    // Update last check time
    state.last_check = Some(now);
    let _ = state_manager.save(&state);

    // Step 1: Check for updates
    let check_result = match check_for_update().await {
        Ok(result) => result,
        Err(_) => return, // Network error or API down - silently skip
    };

    if !check_result.update_available {
        // Already on latest version
        return;
    }

    // Step 2: Download the update
    let platform = match detect_platform() {
        Ok(p) => p,
        Err(_) => return, // Unsupported platform - skip
    };

    let download_result = match download_binary(platform, Some(&check_result.latest_version)).await
    {
        Ok(result) => result,
        Err(_) => return, // Download failed - silently skip
    };

    // Step 3: Store the pending update path in state
    state.pending_update_path = Some(download_result.file_path.to_string_lossy().to_string());
    state.available_version = Some(check_result.latest_version);
    let _ = state_manager.save(&state);

    // Update is now ready for installation on next launch
    // User will see notification in TUI or can run `spoq --update` manually
}

fn main() -> Result<()> {
    // Handle CLI commands before any TUI initialization
    let command = parse_args(std::env::args());
    if let Some(result) = run_cli_command(command) {
        return result;
    }

    // Initialize tracing to write to /tmp/spoq_debug.log
    {
        use std::fs::OpenOptions;
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};

        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/spoq_debug.log")
            .expect("Failed to open /tmp/spoq_debug.log");

        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_writer(std::sync::Mutex::new(log_file))
                    .with_ansi(false)
                    .with_target(false),
            )
            .with(filter)
            .init();
    }

    color_eyre::install()?;

    // Log startup to confirm new binary is running
    tracing::info!("=== SPOQ STARTING (sync debug build) ===");

    // Setup panic hook to ensure terminal cleanup on panic
    setup_panic_hook();

    // Create Tokio runtime for the entire application
    // This runtime will be used for auth flows and then for TUI async operations
    let runtime = tokio::runtime::Runtime::new()?;

    // =========================================================
    // Pre-flight checks - auth, VPS, health (via startup module)
    // Set SPOQ_DEV=1 to skip auth and use localhost:8000
    // =========================================================
    let startup_config = StartupConfig::from_env();
    let startup_result = match run_preflight_checks(&runtime, startup_config) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Startup failed: {}", e);
            std::process::exit(1);
        }
    };

    // Extract startup results
    let credentials = startup_result.credentials;
    let vps_url = startup_result.vps_url;
    let debug_tx = startup_result.debug_tx;
    let debug_server_handle = startup_result.debug_server_handle;

    // =========================================================
    // Update check - run in background, non-blocking
    // =========================================================
    runtime.spawn(async {
        check_and_download_update().await;
    });

    // =========================================================
    // TUI initialization - user is now authenticated
    // =========================================================

    // Setup terminal using RAII pattern
    // Small delay to let PTY fully initialize (needed for some terminal emulators)
    thread::sleep(Duration::from_millis(100));

    // Create terminal manager - handles all setup and cleanup via RAII
    let mut term_manager = TerminalManager::new()?;

    // Initialize application state with debug sender, VPS URL, and credentials
    let mut app = App::with_credentials(debug_tx, vps_url, credentials)?;

    // Log initial auth state for debugging
    app.log_initial_auth_state();

    // Capture initial terminal dimensions
    let size = term_manager.size()?;
    app.update_terminal_dimensions(size.width, size.height);

    // Initialize server connection - user is already authenticated with ready VPS
    // Login and Provisioning screens are handled by pre-flight checks above
    runtime.block_on(async {
        // Load threads from backend (async initialization)
        app.initialize().await;

        // Load folders for the folder picker (async, non-blocking)
        app.load_folders();

        // Load GitHub repos for empty state (async, non-blocking)
        app.load_repos();

        // Preload unified picker data in background (repos, threads, folders)
        // This enables instant @ picker opening
        app.preload_picker_data();

        // Connect WebSocket for real-time communication
        // Build config with token from credentials and VPS URL
        let mut ws_config = WsClientConfig::default();

        // Use VPS URL for WebSocket host (strip protocol prefix)
        // Use wss:// for HTTPS URLs, ws:// for HTTP/plain URLs
        // Default to wss:// for domains (likely behind Cloudflare Tunnel)
        if let Some(ref url) = app.vps_url {
            let (host, use_tls) = if url.starts_with("https://") {
                (url.strip_prefix("https://").unwrap(), true)
            } else if url.starts_with("http://") {
                (url.strip_prefix("http://").unwrap(), false)
            } else {
                // No protocol prefix - default to TLS for domains, non-TLS for IPs
                let is_ip = url.split(':').next().map_or(false, |h| {
                    h.parse::<std::net::Ipv4Addr>().is_ok()
                        || h.parse::<std::net::Ipv6Addr>().is_ok()
                });
                (url.as_str(), !is_ip) // Use TLS for domains
            };
            ws_config = ws_config.with_host(host).with_tls(use_tls);
        }

        // Add auth token if available
        if let Some(ref token) = app.credentials.access_token {
            ws_config = ws_config.with_auth(token);
        }

        // Emit debug event showing connection attempt
        if let Some(ref tx) = app.debug_tx {
            let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                StateChangeData::new(
                    StateType::WebSocket,
                    "WS_CONNECTING",
                    format!(
                        "Connecting to {} (use_tls={}, has_token={})",
                        ws_config.host,
                        ws_config.use_tls,
                        ws_config.auth_token.is_some()
                    ),
                ),
            )));
        }

        // If connection fails, app continues in SSE-only mode
        match start_websocket_with_config(app.message_tx.clone(), ws_config).await {
            Ok(sender) => {
                if let Some(ref tx) = app.debug_tx {
                    let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                        StateChangeData::new(
                            StateType::WebSocket,
                            "WS_INIT",
                            "WebSocket connected successfully",
                        ),
                    )));
                }
                app.ws_sender = Some(sender);
            }
            Err(e) => {
                if let Some(ref tx) = app.debug_tx {
                    let _ = tx.send(DebugEvent::new(DebugEventKind::StateChange(
                        StateChangeData::new(
                            StateType::WebSocket,
                            "WS_INIT_FAILED",
                            format!("WebSocket connection failed: {}", e),
                        ),
                    )));
                }
                app.ws_sender = None;
            }
        }

        // =========================================================================
        // Initialize Credential Auto-Sync
        // =========================================================================
        tracing::info!("Starting credential change detection...");

        // Start file watcher for ~/.claude.json and ~/.config/gh/hosts.yml
        match spawn_file_watcher(app.message_tx.clone()) {
            Ok(watcher) => {
                app.set_credential_file_watcher(watcher);
                tracing::info!("Credential file watcher started");
            }
            Err(e) => {
                // Non-fatal: Keychain polling still works
                tracing::warn!("Failed to start file watcher: {}", e);
            }
        }

    });

    // Main event loop
    let result = runtime.block_on(run_app(term_manager.terminal(), &mut app));

    // Before exiting, save input history
    app.input_history.save();

    // Restore terminal explicitly (also happens via Drop, but this shows intent)
    term_manager.restore()?;

    // Cleanup debug server if it was started
    if let Some(handle) = debug_server_handle {
        handle.abort();
    }

    result
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    // Track migration progress animation
    let migration_start = tokio::time::Instant::now();
    const MIGRATION_DURATION_MS: u64 = 5000; // 5 seconds

    // Create async event stream for keyboard input
    let mut event_stream = EventStream::new();

    // Take the message receiver from the app (we need ownership for select!)
    let mut message_rx: Option<mpsc::UnboundedReceiver<AppMessage>> = app.message_rx.take();

    loop {
        // Update migration progress if it's running
        if app.migration_progress.is_some() {
            let elapsed_ms = migration_start.elapsed().as_millis() as u64;
            if elapsed_ms >= MIGRATION_DURATION_MS {
                // Migration complete, hide progress bar
                app.migration_progress = None;
                app.mark_dirty();
            } else {
                // Calculate progress percentage (0-100)
                let progress = ((elapsed_ms * 100) / MIGRATION_DURATION_MS) as u8;
                if app.migration_progress != Some(progress) {
                    app.migration_progress = Some(progress);
                    app.mark_dirty();
                }
            }
        }

        // Draw the UI only when needed (dirty flag or streaming)
        if app.needs_redraw || app.is_streaming() {
            // Synchronized output (DEC mode 2026) - batch all updates atomically
            // This prevents flickering/tearing during render
            let mut stdout = std::io::stdout();
            let _ = execute!(stdout, BeginSynchronizedUpdate);
            terminal.draw(|f| {
                ui::render(f, &mut *app);
            })?;
            let _ = execute!(stdout, EndSynchronizedUpdate);
            app.needs_redraw = false;
        }

        // Poll both keyboard events and message channel using tokio::select!
        // 16ms tick for smooth 60fps-like scrolling animation
        let timeout = tokio::time::sleep(std::time::Duration::from_millis(16));

        tokio::select! {
            // Handle timeout for UI updates (migration progress, animations, etc.)
            _ = timeout => {
                // Increment tick counter for animations (spinner, cursor blink)
                app.tick();

                // Check for thread switcher auto-confirm (Tab release simulation)
                app.check_switcher_timeout();

                // Unified picker uses local filtering now - no debounced API calls needed
            }

            // Handle keyboard events
            event_result = event_stream.next() => {
                if let Some(Ok(event)) = event_result {
                    match event {
                        Event::Resize(width, height) => {
                            // Update app state with new terminal dimensions
                            app.update_terminal_dimensions(width, height);
                            // Redraw will happen on next loop iteration
                            continue;
                        }
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            // Any key press likely changes state (input, navigation, etc.)
                            app.mark_dirty();

                            // DEBUG: Log ALL key events
                            app.emit_debug_state_change(
                                "KeyEvent",
                                &format!(
                                    "code={:?} mods={:?}",
                                    key.code, key.modifiers
                                ),
                                "",
                            );

                            // Global keybinds (always active)
                            match key.code {
                                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    // Priority 1: In Conversation view + streaming: Cancel the stream
                                    if app.screen == Screen::Conversation && app.is_streaming() {
                                        app.cancel_active_stream();
                                        app.last_ctrl_c_time = None; // Reset exit timer
                                        app.mark_dirty();
                                        continue;
                                    }

                                    // Priority 2: If textarea has text, clear it
                                    if !app.textarea.is_empty() {
                                        app.textarea.clear();
                                        app.last_ctrl_c_time = None; // Reset exit timer
                                        app.mark_dirty();
                                        continue;
                                    }

                                    // Priority 3: Double Ctrl+C to exit
                                    let now = std::time::Instant::now();
                                    if let Some(last_time) = app.last_ctrl_c_time {
                                        if now.duration_since(last_time).as_secs() < 2 {
                                            // Second Ctrl+C within 2 seconds - exit
                                            app.quit();
                                            return Ok(());
                                        }
                                    }
                                    // First Ctrl+C or timeout expired - set timestamp
                                    app.last_ctrl_c_time = Some(now);
                                    app.mark_dirty(); // Force redraw to show warning message
                                    continue;
                                }
                                // Shift+Escape to return to CommandDeck from Conversation
                                // (kept for terminals that support it)
                                KeyCode::Esc if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }
                                // Ctrl+W to return to CommandDeck (explicit close/back binding)
                                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }
                                // Shift+N to create new thread
                                KeyCode::Char('N') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                    app.create_new_thread();
                                    continue;
                                }
                                // CapsLock is tricky - use Ctrl+N as alternative
                                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    app.create_new_thread();
                                    continue;
                                }
                                // Alt+P to submit as Programming thread (from CommandDeck)
                                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                                    if app.screen == Screen::CommandDeck && !app.textarea.is_empty() {
                                        app.submit_input(models::ThreadType::Programming);
                                    }
                                    continue;
                                }
                                _ => {}
                            }

                            // =========================================================
                            // Sync Dialog Dismissal
                            // Any key press dismisses the sync dialog when complete/failed
                            // =========================================================
                            {
                                use spoq::app::SyncStatus;
                                match &app.sync_status {
                                    SyncStatus::Complete { .. } | SyncStatus::Failed { .. } => {
                                        // Any key press dismisses the dialog
                                        app.sync_status = SyncStatus::Idle;
                                        app.mark_dirty();
                                        continue;
                                    }
                                    _ => {}
                                }
                            }

                            // =========================================================
                            // Help Dialog Dismissal
                            // Any key press dismisses the help dialog
                            // =========================================================
                            if app.help_dialog_visible {
                                app.help_dialog_visible = false;
                                app.mark_dirty();
                                continue;
                            }

                            // =========================================================
                            // Dashboard Question Overlay Key Handling (CommandDeck)
                            // MUST come BEFORE permission handling to take priority
                            // =========================================================
                            if app.screen == Screen::CommandDeck {
                                if let Some(spoq::view_state::OverlayState::Question { .. }) = app.dashboard.overlay() {
                                    // Check if "Other" text input mode is active
                                    if app.dashboard.is_question_other_active() {
                                        match key.code {
                                            KeyCode::Esc => {
                                                app.dashboard.question_cancel_other();
                                                app.mark_dirty();
                                                continue;
                                            }
                                            KeyCode::Enter => {
                                                if let Some((thread_id, request_id, answers)) = app.dashboard.question_confirm() {
                                                    app.submit_dashboard_question(&thread_id, &request_id, answers);
                                                }
                                                app.mark_dirty();
                                                continue;
                                            }
                                            KeyCode::Backspace => {
                                                app.dashboard.question_backspace();
                                                app.mark_dirty();
                                                continue;
                                            }
                                            KeyCode::Char(c) => {
                                                app.dashboard.question_type_char(c);
                                                app.mark_dirty();
                                                continue;
                                            }
                                            _ => continue,
                                        }
                                    }

                                    // Normal question navigation (not in "Other" text mode)
                                    match key.code {
                                        KeyCode::Esc => {
                                            app.dashboard.collapse_overlay();
                                            app.mark_dirty();
                                            continue;
                                        }
                                        KeyCode::Up => {
                                            app.dashboard.question_prev_option();
                                            app.mark_dirty();
                                            continue;
                                        }
                                        KeyCode::Down => {
                                            app.dashboard.question_next_option();
                                            app.mark_dirty();
                                            continue;
                                        }
                                        KeyCode::Tab => {
                                            app.dashboard.question_next_tab();
                                            app.mark_dirty();
                                            continue;
                                        }
                                        KeyCode::Char(' ') => {
                                            app.dashboard.question_toggle_option();
                                            app.mark_dirty();
                                            continue;
                                        }
                                        KeyCode::Enter => {
                                            if let Some((thread_id, request_id, answers)) = app.dashboard.question_confirm() {
                                                app.submit_dashboard_question(&thread_id, &request_id, answers);
                                            }
                                            app.mark_dirty();
                                            continue;
                                        }
                                        KeyCode::Char('n') | KeyCode::Char('N') => {
                                            // Close overlay (deny)
                                            app.dashboard.collapse_overlay();
                                            app.mark_dirty();
                                            continue;
                                        }
                                        _ => {
                                            // Ignore other keys while overlay is open
                                            continue;
                                        }
                                    }
                                }
                            }

                            // Handle input routing based on the top needs-action thread type
                            // This takes priority over all other key handling
                            if let Some((thread_id, waiting_for)) = app.dashboard.get_top_needs_action_thread() {
                                match waiting_for {
                                    WaitingFor::Permission { ref request_id, ref tool_name } => {
                                        // Check if this is an AskUserQuestion prompt
                                        if tool_name == "AskUserQuestion" && app.is_ask_user_question_pending() {
                                            // Handle "Other" text input mode
                                            if app.question_state.other_active {
                                                match key.code {
                                                    KeyCode::Esc => {
                                                        app.question_cancel_other();
                                                        continue;
                                                    }
                                                    KeyCode::Enter => {
                                                        if app.question_confirm() {
                                                            continue;
                                                        }
                                                        continue;
                                                    }
                                                    KeyCode::Backspace => {
                                                        app.question_backspace();
                                                        continue;
                                                    }
                                                    KeyCode::Char(c) => {
                                                        app.question_type_char(c);
                                                        continue;
                                                    }
                                                    _ => continue,
                                                }
                                            }

                                            // Handle question navigation keys
                                            match key.code {
                                                KeyCode::Tab => {
                                                    app.question_next_tab();
                                                    continue;
                                                }
                                                KeyCode::Up => {
                                                    app.question_prev_option();
                                                    continue;
                                                }
                                                KeyCode::Down => {
                                                    app.question_next_option();
                                                    continue;
                                                }
                                                KeyCode::Char(' ') => {
                                                    app.question_toggle_option();
                                                    continue;
                                                }
                                                KeyCode::Enter => {
                                                    app.question_confirm();
                                                    continue;
                                                }
                                                KeyCode::Char('n') | KeyCode::Char('N') => {
                                                    // Conversation: textarea is hidden, so always capture
                                                    // CommandDeck: only capture when textarea is empty
                                                    if app.screen == Screen::Conversation || app.textarea.is_empty() {
                                                        let permission_id = app.dashboard.pending_permissions_iter()
                                                            .find(|(_, p)| p.tool_name == "AskUserQuestion")
                                                            .map(|(_, p)| p.permission_id.clone());
                                                        if let Some(pid) = permission_id {
                                                            app.deny_permission(&pid);
                                                            continue;
                                                        }
                                                    }
                                                    // Fall through to type 'n' in textarea
                                                }
                                                KeyCode::Char('a') | KeyCode::Char('A') => {
                                                    // Conversation: textarea is hidden, so always capture
                                                    // CommandDeck: only capture when textarea is empty
                                                    if app.screen == Screen::Conversation || app.textarea.is_empty() {
                                                        if app.open_ask_user_question_dialog() {
                                                            tracing::debug!("Opened AskUserQuestion dialog via 'A' key");
                                                            continue;
                                                        }
                                                    }
                                                    // Fall through to type 'a' in textarea
                                                }
                                                _ => {
                                                    // Fall through to normal input handling
                                                }
                                            }
                                        } else {
                                            // Standard permission prompt (y/a/n)
                                            // Conversation: textarea is hidden, so always capture
                                            // CommandDeck: only capture when textarea is empty
                                            if let KeyCode::Char(c) = key.code {
                                                if app.screen == Screen::Conversation || app.textarea.is_empty() {
                                                    // Debug: emit key press to debug system
                                                    app.emit_debug_state_change(
                                                        "permission_key",
                                                        "Key pressed during permission",
                                                        &format!("key: '{}', tool: {}, request_id: {}", c, tool_name, request_id),
                                                    );
                                                    if app.handle_permission_key(c) {
                                                        app.emit_debug_state_change(
                                                            "permission_key",
                                                            "Permission handled",
                                                            &format!("key: '{}' -> handled", c),
                                                        );
                                                        continue;
                                                    }
                                                    // Key wasn't Y/N/A - fall through to type in textarea
                                                }
                                                // Textarea has content OR key wasn't Y/N/A: fall through
                                            }
                                            // Non-char keys or unhandled char keys fall through to normal input
                                        }
                                    }

                                    WaitingFor::UserInput => {
                                        // UserInput dialogs only capture specific dialog navigation keys
                                        // Let ALL other keys (including Y/N/A) fall through to normal input handling
                                        match key.code {
                                            KeyCode::Tab => {
                                                app.question_next_tab();
                                                continue;
                                            }
                                            KeyCode::Up => {
                                                app.question_prev_option();
                                                continue;
                                            }
                                            KeyCode::Down => {
                                                app.question_next_option();
                                                continue;
                                            }
                                            KeyCode::Char(' ') => {
                                                app.question_toggle_option();
                                                continue;
                                            }
                                            KeyCode::Enter => {
                                                app.question_confirm();
                                                continue;
                                            }
                                            // All other keys fall through to normal input handling
                                            // This includes Y/N/A which should NOT be captured here
                                            _ => {
                                                // Fall through - let the key be handled by normal input processing
                                            }
                                        }
                                    }

                                    WaitingFor::PlanApproval { ref request_id } => {
                                        // Conversation: textarea is hidden, so always capture
                                        // CommandDeck: only capture when textarea is empty
                                        if let KeyCode::Char(c) = key.code {
                                            if app.screen == Screen::Conversation || app.textarea.is_empty() {
                                                app.emit_debug_state_change(
                                                    "plan_approval_key",
                                                    "Key pressed during plan approval",
                                                    &format!("key: '{}', request_id: {}, thread_id: {}", c, request_id, thread_id),
                                                );
                                                if app.handle_permission_key(c) {
                                                    app.emit_debug_state_change(
                                                        "plan_approval_key",
                                                        "Plan approval handled",
                                                        &format!("key: '{}' -> handled", c),
                                                    );
                                                    continue;
                                                }
                                                // Key wasn't Y/N/A - fall through to type in textarea
                                            }
                                            // Textarea has content OR key wasn't Y/N/A: fall through
                                        }
                                        // Non-char keys or unhandled char keys fall through to normal input
                                    }
                                }
                            }

                            // Slash Autocomplete Key Handling (HIGHEST PRIORITY when visible)
                            // =========================================================
                            if app.slash_autocomplete_visible {
                                match key.code {
                                    KeyCode::Esc => {
                                        // Close autocomplete, remove / + query from input
                                        app.remove_slash_and_query_from_input();
                                        app.slash_autocomplete_visible = false;
                                        app.mark_dirty();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        // Select and execute command
                                        let filtered = app.filtered_slash_commands();
                                        if let Some(command) = filtered.get(app.slash_autocomplete_cursor) {
                                            let cmd_to_execute = command.clone();
                                            // Clean up: remove / and query, clear textarea
                                            app.remove_slash_and_query_from_input();
                                            app.textarea.clear();
                                            app.slash_autocomplete_visible = false;
                                            app.slash_autocomplete_query.clear();
                                            app.slash_autocomplete_cursor = 0;
                                            // Execute the command immediately
                                            tracing::info!("Slash autocomplete: executing {:?}", cmd_to_execute);
                                            app.execute_slash_command(cmd_to_execute);
                                            app.mark_dirty();
                                        }
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        if app.slash_autocomplete_query.is_empty() {
                                            // Query is empty, close autocomplete and remove /
                                            app.textarea.backspace(); // Remove the /
                                            app.slash_autocomplete_visible = false;
                                            app.mark_dirty();
                                        } else {
                                            // Remove last char from query
                                            app.slash_autocomplete_query.pop();
                                            // Also backspace in textarea
                                            app.textarea.backspace();
                                            // Reset cursor to top when filter changes
                                            app.slash_autocomplete_cursor = 0;
                                            app.mark_dirty();
                                        }
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        let filtered = app.filtered_slash_commands();
                                        if !filtered.is_empty() {
                                            app.slash_autocomplete_cursor = app.slash_autocomplete_cursor.saturating_sub(1);
                                            app.mark_dirty();
                                        }
                                        continue;
                                    }
                                    KeyCode::Down => {
                                        let filtered = app.filtered_slash_commands();
                                        if !filtered.is_empty() {
                                            app.slash_autocomplete_cursor = (app.slash_autocomplete_cursor + 1).min(filtered.len() - 1);
                                            app.mark_dirty();
                                        }
                                        continue;
                                    }
                                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                                        // Append character to query
                                        app.slash_autocomplete_query.push(c);
                                        // Also insert in textarea
                                        app.textarea.insert_char(c);
                                        // Reset cursor to top when filter changes
                                        app.slash_autocomplete_cursor = 0;
                                        app.mark_dirty();
                                        continue;
                                    }
                                    _ => {
                                        // Other keys are ignored while autocomplete is open
                                        continue;
                                    }
                                }
                            }

                            // File Picker Key Handling (when visible)
                            // =========================================================
                            if app.file_picker.visible {
                                if app.handle_file_picker_key(key) {
                                    continue;
                                }
                            }

                            // Unified @ Picker Key Handling (when visible)
                            // =========================================================
                            if app.unified_picker.visible {
                                match key.code {
                                    KeyCode::Esc => {
                                        // Close picker and remove @query from textarea
                                        app.close_unified_picker();
                                        app.remove_unified_picker_query_from_input();
                                        app.mark_dirty();
                                        continue;
                                    }
                                    KeyCode::Char(' ') => {
                                        // Space = "I want to type a message first"
                                        // Show selection in input and save for later submission
                                        if let Some(item) = app.unified_picker.selected_item().cloned() {
                                            let display_name = item.display_name();

                                            // Replace @query with @selection + space
                                            app.remove_unified_picker_query_from_input();
                                            app.textarea.insert_char('@');
                                            for ch in display_name.chars() {
                                                app.textarea.insert_char(ch);
                                            }
                                            app.textarea.insert_char(' ');

                                            app.unified_picker.set_pending_selection(item);
                                        }
                                        app.unified_picker.close();
                                        app.mark_dirty();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        let action = app.unified_picker_submit();
                                        app.mark_dirty();

                                        match action {
                                            UnifiedPickerAction::None => {
                                                // Nothing selected - do nothing
                                            }
                                            UnifiedPickerAction::MessageRequired => {
                                                // No message - save pending selection, show selection in input, add space
                                                if let Some(item) = app.unified_picker.selected_item().cloned() {
                                                    // Get the display name for the selected item
                                                    let display_name = item.display_name();

                                                    // Remove current @query from textarea and replace with @selection
                                                    app.remove_unified_picker_query_from_input();
                                                    // Insert @name followed by space
                                                    app.textarea.insert_char('@');
                                                    for ch in display_name.chars() {
                                                        app.textarea.insert_char(ch);
                                                    }
                                                    app.textarea.insert_char(' ');

                                                    app.unified_picker.set_pending_selection(item);
                                                }
                                                app.unified_picker.close();
                                            }
                                            UnifiedPickerAction::StartNewThread { path, name, message } => {
                                                // Clear pending selection on success
                                                app.unified_picker.clear_pending_selection();

                                                // Set working directory
                                                let folder = models::Folder { name, path };
                                                app.selected_folder = Some(folder);

                                                // Clear textarea and set the message
                                                app.textarea.clear();
                                                app.textarea.set_content(&message);

                                                // Submit to create new thread
                                                app.submit_input(models::ThreadType::Programming);
                                            }
                                            UnifiedPickerAction::CloneRepo { name, url: _, message } => {
                                                // Show clone progress in picker
                                                app.unified_picker.start_clone(&format!("Cloning {}...", name));
                                                app.mark_dirty();

                                                // Clone repo asynchronously
                                                let client = app.client.clone();
                                                let message_tx = app.message_tx.clone();
                                                let clone_name = name.clone();

                                                tokio::spawn(async move {
                                                    match client.clone_repo(&clone_name).await {
                                                        Ok(response) => {
                                                            let _ = message_tx.send(AppMessage::UnifiedPickerCloneComplete {
                                                                local_path: response.path,
                                                                name: clone_name,
                                                                message,
                                                            });
                                                        }
                                                        Err(e) => {
                                                            let _ = message_tx.send(AppMessage::UnifiedPickerCloneFailed {
                                                                error: e.to_string(),
                                                            });
                                                        }
                                                    }
                                                });
                                            }
                                            UnifiedPickerAction::ResumeThread { id, title: _, message } => {
                                                // Clear pending selection on success
                                                app.unified_picker.clear_pending_selection();

                                                // Open the thread
                                                app.open_thread(id);

                                                // If there's a message, set it and submit
                                                if let Some(msg) = message {
                                                    app.textarea.clear();
                                                    app.textarea.set_content(&msg);
                                                    app.submit_input(models::ThreadType::Programming);
                                                }
                                            }
                                        }
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        if app.unified_picker.query.is_empty() {
                                            // Query is empty, close picker and remove @
                                            app.textarea.backspace(); // Remove the @
                                            app.close_unified_picker();
                                            app.mark_dirty();
                                        } else {
                                            // Remove last char from query - filters locally (instant)
                                            app.unified_picker_backspace();
                                            app.textarea.backspace();
                                            app.mark_dirty();
                                        }
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        app.unified_picker_move_up();
                                        app.mark_dirty();
                                        continue;
                                    }
                                    KeyCode::Down => {
                                        app.unified_picker_move_down();
                                        app.mark_dirty();
                                        continue;
                                    }
                                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                                        // Type char - filters locally (instant)
                                        app.unified_picker_type_char(c);
                                        app.textarea.insert_char(c);
                                        app.mark_dirty();
                                        continue;
                                    }
                                    _ => {
                                        continue;
                                    }
                                }
                            }

                            // Thread switcher handling (takes priority when visible)
                            if app.thread_switcher.visible {
                                match key.code {
                                    KeyCode::Tab | KeyCode::Down => {
                                        app.cycle_switcher_forward();
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        app.cycle_switcher_backward();
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        app.close_switcher();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        app.confirm_switcher_selection();
                                        continue;
                                    }
                                    _ => {
                                        // Any other key closes and confirms
                                        app.confirm_switcher_selection();
                                        continue;
                                    }
                                }
                            }

                            // BrowseList screen handling (full-screen threads/repos view)
                            // Auto-search: typing characters starts searching automatically
                            // Arrow keys always work for navigation
                            if app.screen == Screen::BrowseList {
                                match key.code {
                                    KeyCode::Esc => {
                                        // If searching, clear search first; otherwise return to CommandDeck
                                        if !app.browse_list.search_query.is_empty() {
                                            app.browse_list_clear_search();
                                        } else {
                                            app.close_browse_list();
                                        }
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        app.browse_list_move_up();
                                        continue;
                                    }
                                    KeyCode::Down => {
                                        app.browse_list_move_down();
                                        continue;
                                    }
                                    KeyCode::Char('k') if key.modifiers.is_empty() && app.browse_list.search_query.is_empty() => {
                                        // vim-style up (only when not searching)
                                        app.browse_list_move_up();
                                        continue;
                                    }
                                    KeyCode::Char('j') if key.modifiers.is_empty() && app.browse_list.search_query.is_empty() => {
                                        // vim-style down (only when not searching)
                                        app.browse_list_move_down();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        // Select the current item
                                        let action = app.browse_list_select();
                                        match action {
                                            BrowseListSelectAction::OpenThread { id, .. } => {
                                                // Close browse list and navigate to the thread
                                                app.screen = Screen::CommandDeck; // Temporarily go to CommandDeck
                                                app.open_thread(id); // This handles all the navigation and message loading
                                            }
                                            BrowseListSelectAction::SetWorkingDirectory { path, name } => {
                                                // Set working directory and go to CommandDeck
                                                app.close_browse_list();
                                                app.selected_folder = Some(models::Folder {
                                                    name,
                                                    path,
                                                });
                                                app.mark_dirty();
                                            }
                                            BrowseListSelectAction::CloneRepo { name, url: _ } => {
                                                // Start clone animation
                                                app.browse_list_start_clone(&name);

                                                // Clone repo asynchronously
                                                let client = app.client.clone();
                                                let message_tx = app.message_tx.clone();
                                                let clone_name = name.clone();

                                                tokio::spawn(async move {
                                                    match client.clone_repo(&clone_name).await {
                                                        Ok(response) => {
                                                            let _ = message_tx.send(AppMessage::BrowseListCloneComplete {
                                                                local_path: response.path,
                                                                name: clone_name,
                                                            });
                                                        }
                                                        Err(e) => {
                                                            let _ = message_tx.send(AppMessage::BrowseListCloneFailed {
                                                                error: e.to_string(),
                                                            });
                                                        }
                                                    }
                                                });
                                            }
                                            BrowseListSelectAction::None => {}
                                        }
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        // Remove last character from search
                                        match app.browse_list.mode {
                                            spoq::app::BrowseListMode::Threads => {
                                                // Threads: debounced API search
                                                if let Some(query) = app.browse_list_backspace() {
                                                    let message_tx = app.message_tx.clone();
                                                    tokio::spawn(async move {
                                                        tokio::time::sleep(std::time::Duration::from_millis(
                                                            spoq::ui::SEARCH_DEBOUNCE_MS,
                                                        ))
                                                        .await;
                                                        let _ = message_tx
                                                            .send(AppMessage::BrowseListSearchDebounced { query });
                                                    });
                                                }
                                            }
                                            spoq::app::BrowseListMode::Repos => {
                                                // Repos: instant local filter
                                                app.browse_list_repos_backspace();
                                            }
                                        }
                                        continue;
                                    }
                                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                                        // Auto-search: any printable character adds to search
                                        match app.browse_list.mode {
                                            spoq::app::BrowseListMode::Threads => {
                                                // Threads: debounced API search
                                                let query = app.browse_list_type_char(c);
                                                let message_tx = app.message_tx.clone();
                                                tokio::spawn(async move {
                                                    tokio::time::sleep(std::time::Duration::from_millis(
                                                        spoq::ui::SEARCH_DEBOUNCE_MS,
                                                    ))
                                                    .await;
                                                    let _ = message_tx
                                                        .send(AppMessage::BrowseListSearchDebounced { query });
                                                });
                                            }
                                            spoq::app::BrowseListMode::Repos => {
                                                // Repos: instant local filter
                                                app.browse_list_repos_type_char(c);
                                            }
                                        }
                                        continue;
                                    }
                                    _ => continue,
                                }
                            }

                            // Handle OAuth consent 'o' key to open URL in browser
                            if let KeyCode::Char('o') = key.code {
                                if let Some(url) = &app.session_state.oauth_url {
                                    // Open URL in browser using the 'open' crate
                                    if let Err(_e) = open::that(url) {
                                        // Silently ignore errors - user can manually copy URL from UI
                                    }
                                    // Don't clear the URL yet - leave it until OAuth is completed
                                    continue;
                                }
                            }

                            // Auto-focus to Input when user starts typing
                            // (printable characters only, not Ctrl combinations)
                            if let KeyCode::Char(_) = key.code {
                                if !key.modifiers.contains(KeyModifiers::CONTROL) && app.focus != Focus::Input {
                                    app.focus = Focus::Input;
                                    // Character will be processed by input handling below
                                }
                            }

                            // Handle input-specific keys when Input is focused
                            if app.focus == Focus::Input {
                                // Check for Shift+Escape FIRST (before plain Escape)
                                // This ensures Shift+Escape goes back to CommandDeck even when typing
                                if key.code == KeyCode::Esc && key.modifiers.contains(KeyModifiers::SHIFT) {
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                    continue;
                                }

                                // Shift+Tab cycles permission mode (works while typing, all threads)
                                if key.code == KeyCode::BackTab {
                                    if app.screen == Screen::Conversation || app.screen == Screen::CommandDeck {
                                        app.cycle_permission_mode();
                                    }
                                    continue;
                                }

                                // macOS-style text navigation shortcuts (modifier + key)
                                // Check these BEFORE plain key handlers
                                match key.code {
                                    // Alt+Backspace: Delete word backward
                                    KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.delete_word_backward();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    // Super+Backspace (Cmd+Backspace): Delete to line start
                                    // Note: Most terminals intercept this, so Ctrl+U is the reliable alternative
                                    KeyCode::Backspace if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.delete_to_line_start();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    // Alt+Left: Move cursor word left
                                    KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.move_cursor_word_left();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    // Super+Left (Cmd+Left): Move cursor to line start
                                    KeyCode::Left if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.move_cursor_home();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    // Alt+Right: Move cursor word right
                                    KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                                        app.textarea.move_cursor_word_right();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    // Super+Right (Cmd+Right): Move cursor to line end
                                    KeyCode::Right if key.modifiers.contains(KeyModifiers::SUPER) => {
                                        app.textarea.move_cursor_end();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    _ => {}
                                }

                                // Plain key handlers (without modifiers)
                                match key.code {
                                    // Ctrl+U = Unix "kill line" - delete to line start
                                    // Works in ALL terminals (unlike Cmd+Backspace which terminals intercept)
                                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        app.textarea.delete_to_line_start();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    // Ctrl+J = ASCII LF (newline) - works in ALL terminals
                                    // MUST come before plain Char(c) handler
                                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        app.textarea.insert_newline();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    // Plain characters (no modifiers or only SHIFT)
                                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                                        // Reset scroll to show input when typing (unified scroll)
                                        if app.screen == Screen::Conversation {
                                            app.user_has_scrolled = false;
                                            app.unified_scroll = 0;
                                        }

                                        // Apply shift translation for non-uppercase characters
                                        let char_to_insert = if key.modifiers.contains(KeyModifiers::SHIFT) && !c.is_uppercase() {
                                            translate_shifted_char(c)
                                        } else {
                                            c
                                        };

                                        // Check for / trigger for slash command autocomplete (only at very start of empty input)
                                        if char_to_insert == '/' && app.is_slash_autocomplete_trigger() {
                                            app.textarea.insert_char('/');
                                            app.slash_autocomplete_visible = true;
                                            app.slash_autocomplete_query.clear();
                                            app.slash_autocomplete_cursor = 0;
                                            app.mark_dirty();
                                            continue;
                                        }

                                        // Check for @ trigger for file picker (Conversation screen)
                                        if char_to_insert == '@' && app.screen == Screen::Conversation {
                                            let (row, col) = app.textarea.cursor();
                                            let lines = app.textarea.lines();
                                            let line_content = lines.get(row).map(|s| s.as_str()).unwrap_or("");

                                            if app.is_file_picker_trigger(line_content, col) {
                                                app.textarea.insert_char('@');
                                                app.open_file_picker();
                                                app.mark_dirty();
                                                continue;
                                            }
                                        }

                                        // Check for @ trigger for unified picker (repos, threads, folders)
                                        // Only trigger on CommandDeck and when it looks like a mention, not an email
                                        if char_to_insert == '@' && app.screen == Screen::CommandDeck {
                                            let (row, col) = app.textarea.cursor();
                                            let lines = app.textarea.lines();
                                            let line_content = lines.get(row).map(|s| s.as_str()).unwrap_or("");

                                            // Check if this is a valid @ trigger position (not an email)
                                            if app.is_folder_picker_trigger(line_content, col) {
                                                app.textarea.insert_char('@');
                                                app.open_unified_picker();
                                                app.mark_dirty();
                                                continue;
                                            }
                                        }

                                        // Normal character insertion
                                        app.textarea.insert_char(char_to_insert);
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::Backspace => {
                                        // Check if we should clear the folder chip instead of backspace
                                        if app.should_clear_folder_on_backspace() {
                                            app.clear_folder();
                                        } else {
                                            app.textarea.backspace();
                                            app.reset_cursor_blink();
                                        }
                                        continue;
                                    }
                                    KeyCode::Delete => {
                                        app.textarea.delete_char();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::Left => {
                                        app.textarea.move_cursor_left();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::Right => {
                                        app.textarea.move_cursor_right();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::Up => {
                                        // If cursor is on first line, try to navigate history up
                                        if app.textarea.is_cursor_on_first_line() {
                                            let current_content = app.textarea.content();
                                            if let Some(history_entry) = app.input_history.navigate_up(&current_content) {
                                                let entry = history_entry.to_string();
                                                app.textarea.set_content(&entry);
                                                app.reset_cursor_blink();
                                            }
                                        } else {
                                            // Normal cursor movement
                                            app.textarea.move_cursor_up();
                                            app.reset_cursor_blink();
                                        }
                                        continue;
                                    }
                                    KeyCode::Down => {
                                        // If cursor is on last line and navigating history, go forward
                                        if app.textarea.is_cursor_on_last_line() {
                                            // Only handle history navigation if we're currently navigating
                                            if app.input_history.is_navigating() {
                                                if let Some(history_entry) = app.input_history.navigate_down() {
                                                    let entry = history_entry.to_string();
                                                    app.textarea.set_content(&entry);
                                                    app.reset_cursor_blink();
                                                } else {
                                                    // At bottom of history, restore original input
                                                    let original = app.input_history.get_current_input().to_string();
                                                    app.textarea.set_content(&original);
                                                    app.reset_cursor_blink();
                                                }
                                            }
                                            // If not navigating, Down on last line does nothing
                                        } else {
                                            // Normal cursor movement in multi-line input
                                            app.textarea.move_cursor_down();
                                            app.reset_cursor_blink();
                                        }
                                        continue;
                                    }
                                    KeyCode::Home => {
                                        app.textarea.move_cursor_home();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::End => {
                                        app.textarea.move_cursor_end();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                        // Shift+Enter inserts a newline (works in Kitty protocol terminals)
                                        app.textarea.insert_newline();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+Enter inserts a newline
                                        app.textarea.insert_newline();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+Enter inserts a newline (fallback - may not work in all terminals)
                                        app.textarea.insert_newline();
                                        app.reset_cursor_blink();
                                        continue;
                                    }
                                    KeyCode::Enter => {
                                        // Check for pending selection from @ picker
                                        if app.unified_picker.has_pending_selection() {
                                            use crate::models::picker::PickerItem;

                                            if let Some(pending) = app.unified_picker.take_pending_selection() {
                                                // Extract message - remove @query prefix if present
                                                let content = app.textarea.content_expanded();
                                                let message = if content.starts_with('@') {
                                                    // Find the space after @query and take everything after
                                                    content.splitn(2, ' ').nth(1).unwrap_or("").trim().to_string()
                                                } else {
                                                    content.trim().to_string()
                                                };

                                                if message.is_empty() {
                                                    // Still no message - reopen picker
                                                    app.unified_picker.set_pending_selection(pending);
                                                    app.open_unified_picker();
                                                    continue;
                                                }

                                                match pending {
                                                    PickerItem::Folder { path, name } | PickerItem::Repo { local_path: Some(path), name, .. } => {
                                                        // Set working directory and submit
                                                        let folder = models::Folder { name, path };
                                                        app.selected_folder = Some(folder);
                                                        app.textarea.clear();
                                                        app.textarea.set_content(&message);
                                                        app.submit_input(models::ThreadType::Programming);
                                                    }
                                                    PickerItem::Repo { local_path: None, name, url: _ } => {
                                                        // Remote repo needs clone first
                                                        // Show picker with clone progress (don't reset state)
                                                        app.unified_picker.visible = true;
                                                        app.unified_picker.start_clone(&format!("Cloning {}...", name));
                                                        app.mark_dirty();

                                                        let client = app.client.clone();
                                                        let message_tx = app.message_tx.clone();
                                                        let clone_name = name.clone();
                                                        let clone_message = message.clone();
                                                        tokio::spawn(async move {
                                                            match client.clone_repo(&clone_name).await {
                                                                Ok(response) => {
                                                                    let _ = message_tx.send(AppMessage::UnifiedPickerCloneComplete {
                                                                        local_path: response.path,
                                                                        name: clone_name,
                                                                        message: clone_message,
                                                                    });
                                                                }
                                                                Err(e) => {
                                                                    let _ = message_tx.send(AppMessage::UnifiedPickerCloneFailed {
                                                                        error: e.to_string(),
                                                                    });
                                                                }
                                                            }
                                                        });
                                                    }
                                                    PickerItem::Thread { id, .. } => {
                                                        // Resume thread with message
                                                        app.open_thread(id);
                                                        app.textarea.clear();
                                                        app.textarea.set_content(&message);
                                                        app.submit_input(models::ThreadType::Programming);
                                                    }
                                                }
                                                continue;
                                            }
                                        }

                                        // No pending selection - plain Enter = Conversation thread
                                        app.submit_input(models::ThreadType::Conversation);
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        // Plain Escape (no Shift) - depends on input state and screen
                                        if app.screen == Screen::Conversation {
                                            if app.textarea.is_empty() {
                                                // Empty input: go back to CommandDeck
                                                app.navigate_to_command_deck();
                                            } else {
                                                // Has content: just unfocus to allow navigation
                                                app.focus = Focus::Threads;
                                            }
                                        } else {
                                            // On CommandDeck: unfocus input
                                            app.focus = Focus::Threads;
                                        }
                                        continue;
                                    }
                                    _ => {}
                                }
                            }

                            // Panel navigation (when not typing in input)
                            match key.code {
                                KeyCode::Tab => {
                                    // Double-tap Tab opens thread switcher
                                    app.handle_tab_press();
                                }
                                KeyCode::BackTab => {
                                    // Shift+Tab in Conversation/CommandDeck screens: cycle permission mode (all threads)
                                    if app.screen == Screen::Conversation || app.screen == Screen::CommandDeck {
                                        app.cycle_permission_mode();
                                    }
                                }
                                KeyCode::Esc if app.focus != Focus::Input => {
                                    // Escape when not in input: go back to CommandDeck
                                    if app.screen == Screen::Conversation {
                                        app.navigate_to_command_deck();
                                    }
                                }
                                KeyCode::Enter if app.focus == Focus::Threads => {
                                    // Open selected thread when pressing Enter on Threads panel
                                    app.open_selected_thread();
                                }
                                // Page scroll keys for conversation (unified scroll)
                                KeyCode::PageUp if app.screen == Screen::Conversation => {
                                    // Page up = scroll up to see older content
                                    app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                    app.user_has_scrolled = true;
                                    let new_scroll = (app.unified_scroll + 10).min(app.max_scroll);
                                    let needs_redraw = if new_scroll != app.unified_scroll {
                                        app.unified_scroll = new_scroll;
                                        app.scroll_position = app.unified_scroll as f32;
                                        true
                                    } else if app.max_scroll > 0 {
                                        app.scroll_boundary_hit = Some(ScrollBoundary::Top);
                                        app.boundary_hit_tick = app.tick_count;
                                        true
                                    } else {
                                        false
                                    };
                                    if needs_redraw {
                                        app.mark_dirty();
                                    }
                                }
                                KeyCode::PageDown if app.screen == Screen::Conversation => {
                                    // Page down = scroll down to see newer content / input
                                    app.scroll_velocity = 0.0; // Reset momentum on user scroll
                                    let new_scroll = app.unified_scroll.saturating_sub(10);
                                    let needs_redraw = if new_scroll != app.unified_scroll {
                                        app.unified_scroll = new_scroll;
                                        app.scroll_position = app.unified_scroll as f32;
                                        if app.unified_scroll == 0 {
                                            app.user_has_scrolled = false; // Back at bottom
                                        }
                                        true
                                    } else {
                                        app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
                                        app.boundary_hit_tick = app.tick_count;
                                        true
                                    };
                                    if needs_redraw {
                                        app.mark_dirty();
                                    }
                                }
                                KeyCode::Up => {
                                    app.move_up();
                                }
                                KeyCode::Down => {
                                    let max_threads = app.cache.threads().len();
                                    app.move_down(max_threads);
                                }
                                KeyCode::Char('q') if app.focus != Focus::Input => {
                                    app.quit();
                                    return Ok(());
                                }
                                // 'd' to dismiss focused error in Conversation screen
                                KeyCode::Char('d') if app.focus != Focus::Input && app.screen == Screen::Conversation => {
                                    if app.has_errors() {
                                        app.dismiss_focused_error();
                                    }
                                }
                                // 't' to toggle thinking/reasoning block in Conversation screen
                                KeyCode::Char('t') if app.focus != Focus::Input && app.screen == Screen::Conversation => {
                                    app.toggle_reasoning();
                                }
                                // 'A' to open first user input question dialog in dashboard view
                                KeyCode::Char('a') | KeyCode::Char('A') if app.focus != Focus::Input && app.screen == Screen::CommandDeck => {
                                    app.open_ask_user_question_dialog();
                                }
                                // Note: Custom mouse selection removed - native terminal selection now handles copy
                                _ => {}
                            }
                        }
                        Event::Mouse(mouse_event) => {
                            // Handle mouse events for scroll only (click/hover system removed)
                            match mouse_event.kind {
                                // Momentum-based scrolling for smooth feel
                                // Each scroll event adds velocity, momentum system handles animation
                                MouseEventKind::ScrollDown => {
                                    if app.screen == Screen::Conversation {
                                        if app.unified_scroll > 0 {
                                            app.unified_scroll -= 1;
                                        }
                                        app.user_has_scrolled = app.unified_scroll > 0;
                                        app.scroll_changed = true;
                                    }
                                }
                                MouseEventKind::ScrollUp => {
                                    if app.screen == Screen::Conversation {
                                        app.unified_scroll = (app.unified_scroll + 1).min(app.max_scroll);
                                        app.user_has_scrolled = true;
                                        app.scroll_changed = true;
                                    }
                                }
                                // Ignore other mouse events (right click, drag, etc.)
                                // Terminal handles text selection natively
                                _ => {}
                            }
                            continue;
                        }
                        Event::Paste(text) => {
                            // Handle paste events from bracketed paste mode
                            // Auto-focus to input if not already focused
                            if app.focus != Focus::Input {
                                app.focus = Focus::Input;
                            }

                            if app.should_summarize_paste(&text) {
                                // Insert as atomic token
                                app.textarea.insert_paste_token(text);
                                app.reset_cursor_blink();
                            } else {
                                // Insert normally character by character
                                for ch in text.chars() {
                                    app.textarea.insert_char(ch);
                                }
                                app.reset_cursor_blink();
                            }

                            app.mark_dirty();
                            continue;
                        }
                        _ => {
                            // Ignore other events (focus, etc.)
                        }
                    }
                }
            }

            // Handle async messages from streaming/connection
            msg = async {
                match &mut message_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(msg) = msg {
                    app.handle_message(msg);
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
