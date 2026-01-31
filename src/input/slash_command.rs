//! Slash command definitions and parsing.
//!
//! This module provides slash commands that users can type in the input field
//! to perform special actions like syncing credentials, opening billing portal,
//! viewing GitHub repos, or starting a new chat.

/// Represents all available slash commands in the TUI.
///
/// Each command has a primary name and may have aliases.
/// Commands are shown in an autocomplete dropdown when the user types `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    /// Sync credentials to VPS
    /// Primary: /sync
    Sync,

    /// Open billing portal for subscription management
    /// Primary: /manage
    /// Aliases: /upgrade
    Manage,

    /// Show GitHub repositories
    /// Primary: /repos
    Repos,

    /// Start a new chat / go to dashboard
    /// Primary: /new
    /// Aliases: /clear
    New,

    /// Show help and command documentation
    /// Primary: /help
    Help,

    /// Open settings panel
    /// Primary: /settings
    /// Aliases: /config
    Settings,

    /// View and manage threads
    /// Primary: /threads
    /// Aliases: /sessions, /resume
    Threads,

    /// Manage Claude Code accounts
    /// Primary: /claude
    /// Aliases: /accounts
    Claude,
}

impl SlashCommand {
    /// Get all available slash commands
    pub fn all() -> Vec<Self> {
        vec![
            SlashCommand::Sync,
            SlashCommand::Manage,
            SlashCommand::Repos,
            SlashCommand::New,
            SlashCommand::Help,
            SlashCommand::Settings,
            SlashCommand::Threads,
            SlashCommand::Claude,
        ]
    }

    /// Parse a slash command from user input.
    ///
    /// Accepts the command with or without the leading `/`.
    /// Returns None if the input doesn't match any command.
    ///
    /// # Examples
    ///
    /// ```
    /// use spoq::input::slash_command::SlashCommand;
    ///
    /// assert_eq!(SlashCommand::parse("/sync"), Some(SlashCommand::Sync));
    /// assert_eq!(SlashCommand::parse("sync"), Some(SlashCommand::Sync));
    /// assert_eq!(SlashCommand::parse("/upgrade"), Some(SlashCommand::Manage));
    /// assert_eq!(SlashCommand::parse("/unknown"), None);
    /// ```
    pub fn parse(input: &str) -> Option<Self> {
        let normalized = input.trim().trim_start_matches('/').to_lowercase();

        match normalized.as_str() {
            "sync" => Some(SlashCommand::Sync),
            "manage" | "upgrade" => Some(SlashCommand::Manage),
            "repos" => Some(SlashCommand::Repos),
            "new" | "clear" => Some(SlashCommand::New),
            "help" => Some(SlashCommand::Help),
            "settings" | "config" => Some(SlashCommand::Settings),
            "threads" | "sessions" | "resume" => Some(SlashCommand::Threads),
            "claude" | "accounts" => Some(SlashCommand::Claude),
            _ => None,
        }
    }

    /// Get the primary name of the command (what's shown in autocomplete).
    ///
    /// Always includes the leading `/`.
    pub fn name(&self) -> &'static str {
        match self {
            SlashCommand::Sync => "/sync",
            SlashCommand::Manage => "/manage",
            SlashCommand::Repos => "/repos",
            SlashCommand::New => "/new",
            SlashCommand::Help => "/help",
            SlashCommand::Settings => "/settings",
            SlashCommand::Threads => "/threads",
            SlashCommand::Claude => "/claude",
        }
    }

    /// Get all aliases for this command (including primary name).
    ///
    /// Returns a list of all valid ways to invoke this command.
    /// All aliases include the leading `/`.
    pub fn aliases(&self) -> Vec<&'static str> {
        match self {
            SlashCommand::Sync => vec!["/sync"],
            SlashCommand::Manage => vec!["/manage", "/upgrade"],
            SlashCommand::Repos => vec!["/repos"],
            SlashCommand::New => vec!["/new", "/clear"],
            SlashCommand::Help => vec!["/help"],
            SlashCommand::Settings => vec!["/settings", "/config"],
            SlashCommand::Threads => vec!["/threads", "/sessions", "/resume"],
            SlashCommand::Claude => vec!["/claude", "/accounts"],
        }
    }

    /// Get a human-readable description of what the command does.
    pub fn description(&self) -> &'static str {
        match self {
            SlashCommand::Sync => "Sync credentials to VPS",
            SlashCommand::Manage => "Manage subscription and billing",
            SlashCommand::Repos => "Browse GitHub repositories",
            SlashCommand::New => "Start a new chat",
            SlashCommand::Help => "Show help and documentation",
            SlashCommand::Settings => "Open settings panel",
            SlashCommand::Threads => "View and manage threads",
            SlashCommand::Claude => "Manage Claude Code accounts",
        }
    }

    /// Filter commands by a search query.
    ///
    /// Returns commands whose primary name or aliases match the query.
    /// Query is matched case-insensitively and can omit the leading `/`.
    ///
    /// # Examples
    ///
    /// ```
    /// use spoq::input::slash_command::SlashCommand;
    ///
    /// let results = SlashCommand::filter("");
    /// assert_eq!(results.len(), 7); // All commands
    ///
    /// let results = SlashCommand::filter("/sy");
    /// assert_eq!(results, vec![SlashCommand::Sync]);
    ///
    /// let results = SlashCommand::filter("upg");
    /// assert_eq!(results, vec![SlashCommand::Manage]);
    /// ```
    pub fn filter(query: &str) -> Vec<Self> {
        if query.is_empty() {
            return Self::all();
        }

        let normalized_query = query.trim().trim_start_matches('/').to_lowercase();

        Self::all()
            .into_iter()
            .filter(|cmd| {
                cmd.aliases().iter().any(|alias| {
                    alias
                        .trim_start_matches('/')
                        .to_lowercase()
                        .starts_with(&normalized_query)
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_slash() {
        assert_eq!(SlashCommand::parse("/sync"), Some(SlashCommand::Sync));
        assert_eq!(SlashCommand::parse("/manage"), Some(SlashCommand::Manage));
        assert_eq!(SlashCommand::parse("/repos"), Some(SlashCommand::Repos));
        assert_eq!(SlashCommand::parse("/new"), Some(SlashCommand::New));
        assert_eq!(SlashCommand::parse("/help"), Some(SlashCommand::Help));
        assert_eq!(SlashCommand::parse("/settings"), Some(SlashCommand::Settings));
        assert_eq!(SlashCommand::parse("/threads"), Some(SlashCommand::Threads));
    }

    #[test]
    fn test_parse_without_slash() {
        assert_eq!(SlashCommand::parse("sync"), Some(SlashCommand::Sync));
        assert_eq!(SlashCommand::parse("manage"), Some(SlashCommand::Manage));
        assert_eq!(SlashCommand::parse("repos"), Some(SlashCommand::Repos));
        assert_eq!(SlashCommand::parse("new"), Some(SlashCommand::New));
        assert_eq!(SlashCommand::parse("help"), Some(SlashCommand::Help));
        assert_eq!(SlashCommand::parse("settings"), Some(SlashCommand::Settings));
        assert_eq!(SlashCommand::parse("threads"), Some(SlashCommand::Threads));
    }

    #[test]
    fn test_parse_aliases() {
        assert_eq!(SlashCommand::parse("/upgrade"), Some(SlashCommand::Manage));
        assert_eq!(SlashCommand::parse("/clear"), Some(SlashCommand::New));
        assert_eq!(SlashCommand::parse("/config"), Some(SlashCommand::Settings));
        assert_eq!(SlashCommand::parse("/sessions"), Some(SlashCommand::Threads));
        assert_eq!(SlashCommand::parse("/resume"), Some(SlashCommand::Threads));
    }

    #[test]
    fn test_parse_case_insensitive() {
        assert_eq!(SlashCommand::parse("/SYNC"), Some(SlashCommand::Sync));
        assert_eq!(SlashCommand::parse("/MaNaGe"), Some(SlashCommand::Manage));
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(SlashCommand::parse("/unknown"), None);
        assert_eq!(SlashCommand::parse("/foobar"), None);
        assert_eq!(SlashCommand::parse(""), None);
    }

    #[test]
    fn test_name() {
        assert_eq!(SlashCommand::Sync.name(), "/sync");
        assert_eq!(SlashCommand::Manage.name(), "/manage");
        assert_eq!(SlashCommand::Repos.name(), "/repos");
        assert_eq!(SlashCommand::New.name(), "/new");
        assert_eq!(SlashCommand::Help.name(), "/help");
        assert_eq!(SlashCommand::Settings.name(), "/settings");
        assert_eq!(SlashCommand::Threads.name(), "/threads");
    }

    #[test]
    fn test_aliases() {
        assert_eq!(SlashCommand::Sync.aliases(), vec!["/sync"]);
        assert_eq!(SlashCommand::Manage.aliases(), vec!["/manage", "/upgrade"]);
        assert_eq!(SlashCommand::Repos.aliases(), vec!["/repos"]);
        assert_eq!(SlashCommand::New.aliases(), vec!["/new", "/clear"]);
        assert_eq!(SlashCommand::Help.aliases(), vec!["/help"]);
        assert_eq!(SlashCommand::Settings.aliases(), vec!["/settings", "/config"]);
        assert_eq!(SlashCommand::Threads.aliases(), vec!["/threads", "/sessions", "/resume"]);
    }

    #[test]
    fn test_filter_partial_match() {
        let results = SlashCommand::filter("/sy");
        assert_eq!(results, vec![SlashCommand::Sync]);

        let results = SlashCommand::filter("man");
        assert_eq!(results, vec![SlashCommand::Manage]);

        let results = SlashCommand::filter("/rep");
        assert_eq!(results, vec![SlashCommand::Repos]);

        let results = SlashCommand::filter("ne");
        assert_eq!(results, vec![SlashCommand::New]);
    }

    #[test]
    fn test_filter_alias_match() {
        let results = SlashCommand::filter("upg");
        assert_eq!(results, vec![SlashCommand::Manage]);

        let results = SlashCommand::filter("/clear");
        assert_eq!(results, vec![SlashCommand::New]);
    }

    #[test]
    fn test_filter_no_match() {
        let results = SlashCommand::filter("/xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_filter_case_insensitive() {
        let results = SlashCommand::filter("/SYN");
        assert_eq!(results, vec![SlashCommand::Sync]);
    }
}
