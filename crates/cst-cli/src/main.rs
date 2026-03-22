//! `cst` — Claude Sentinel CLI
//!
//! Intelligent Claude Code account, profile, and session manager.

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};

mod commands;
mod tui;
use commands::{profile as profile_cmd, session as session_cmd, shell as shell_cmd};

#[derive(Parser)]
#[command(
    name = "cst",
    about = "🛡 Claude Sentinel — intelligent Claude Code account manager",
    version,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Switch to a profile:session (shell function wraps this).
    /// Without args, opens the interactive TUI.
    Use {
        /// Profile name, optionally with session: "work" or "work:backend"
        profile_session: Option<String>,
    },

    /// Show current profile:session, auth type, and quota status.
    Status,

    /// List all profiles and their sessions.
    List,

    /// Show quota used %, tokens today, and time to reset.
    Remaining,

    /// Live real-time usage dashboard (like htop for Claude).
    Top,

    /// Switch ALL open shells currently on profile <from> to profile <to>.
    /// Each shell picks up the switch on its next prompt via the precmd hook.
    SwitchAll {
        /// Profile to switch away from.
        from: String,
        /// Profile to switch to.
        to: String,
    },

    /// Show switch history with reasons.
    History,

    /// Explain why the current profile is active.
    Why,

    /// Create a new profile.
    New {
        name: String,
        /// Auth type: oauth, api, bedrock, vertex
        #[arg(long, default_value = "oauth")]
        auth: String,
        /// Base on a template: pro, max, api, bedrock, vertex
        #[arg(long)]
        template: Option<String>,
    },

    /// Import current ~/.claude.json as a named profile.
    Import {
        #[arg(long)]
        r#as: Option<String>,
    },

    /// Clone a profile.
    Clone { source: String, destination: String },

    /// Delete a profile.
    Rm { name: String },

    /// Rename a profile.
    Rename { old: String, new: String },

    /// Re-run OAuth login for a profile.
    Login { profile: Option<String> },

    /// Add an API key to a profile's key pool.
    AddKey {
        profile: String,
        #[arg(long, default_value = "1")]
        slot: u8,
        /// Secret provider (skip interactive menu).
        /// Format: "keychain", "op://vault/item/field", "doppler:SECRET_NAME",
        /// "$ENV_VAR" or "env:VAR_NAME"
        #[arg(long)]
        source: Option<String>,
        /// Optional note/label for this key slot.
        #[arg(long)]
        note: Option<String>,
    },

    /// Session management subcommands.
    Session {
        #[command(subcommand)]
        action: SessionCommands,
    },

    /// Auto-switch daemon management.
    Daemon {
        #[command(subcommand)]
        action: DaemonCommands,
    },

    /// Auto-switch configuration.
    AutoSwitch {
        #[command(subcommand)]
        action: AutoSwitchCommands,
    },

    /// Pause auto-switching temporarily.
    Pause {
        #[arg(long)]
        minutes: Option<u64>,
    },

    /// Run a command with a specific profile (no persistent switch).
    Run {
        profile_session: String,
        #[arg(last = true)]
        cmd: Vec<String>,
    },

    /// Rebuild symlinks from ~/.claude/ to all sessions.
    Sync,

    /// Show usage statistics.
    Stats {
        profile_session: Option<String>,
    },

    /// Health check — validate all profiles, symlinks, and credentials.
    Doctor,

    /// Output shell init code (add `eval "$(cst shell-init)"` to your rc).
    ShellInit {
        #[arg(long)]
        shell: Option<String>,
    },

    /// Output env var exports for a profile:session (used by shell function).
    #[command(name = "_env", hide = true)]
    Env { profile_session: String },

    /// Check broadcast-switch.json and output env exports if this shell should switch.
    /// Called by the precmd hook with $CST_CURRENT and $CST_BROADCAST_ID.
    #[command(name = "_broadcast-switch", hide = true)]
    BroadcastSwitch {
        current: String,
        #[arg(default_value = "")]
        already_applied_id: String,
    },

    /// Check .cstrc in the directory tree and emit env exports if a different
    /// profile should be active.  Called by the shell precmd hook.
    #[command(name = "_auto-detect", hide = true)]
    AutoDetect {
        /// Directory to search from (usually $PWD).
        dir: String,
        /// Current profile:session (CST_CURRENT).
        #[arg(default_value = "")]
        current: String,
    },

    /// Show what `.cstrc` would activate in the current (or given) directory.
    AutoDetectStatus {
        #[arg(default_value = ".")]
        dir: String,
    },

    /// List available profile templates.
    Templates,

    /// Validate a profile's config and credentials.
    Validate { profile: String },

    /// Print Starship custom module output (add `eval "$(cst starship --config)"` to starship.toml).
    Starship {
        /// Print starship.toml config snippet instead of module output.
        #[arg(long)]
        config: bool,
    },

    /// Print tmux status bar segment (add `set -g status-right "#(cst tmux)"`).
    Tmux {
        /// Print tmux.conf snippet instead of segment output.
        #[arg(long)]
        config: bool,
    },

    /// Team profile sharing — push/pull configs via a shared git remote.
    Team {
        #[command(subcommand)]
        action: TeamCommands,
    },

    /// Generate shell tab completions.
    Completions {
        /// Shell: bash, zsh, fish, powershell
        shell: Shell,
    },

    /// Open the interactive TUI.
    Tui,

    /// First-run setup wizard.
    Init {
        /// Accept all defaults without prompting.
        #[arg(long)]
        yes: bool,
        /// Shell to configure: zsh, bash, fish
        #[arg(long)]
        shell: Option<String>,
        /// Skip starting the daemon.
        #[arg(long)]
        no_daemon: bool,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    /// Create a new session for the current profile.
    New {
        name: String,
        #[arg(long)]
        tag: Option<String>,
    },
    /// List sessions for a profile.
    List { profile: Option<String> },
    /// Delete a session.
    Rm { name: String },
    /// Add a description tag to a session.
    Tag { name: String, description: String },
    /// Archive a session (hidden from list, history kept).
    Archive { name: String },
    /// Activate a session under a different profile (creates it there if needed).
    Switch {
        /// Session name to switch.
        name: String,
        /// Target profile to activate the session under.
        #[arg(long = "to")]
        to_profile: String,
    },
}

#[derive(Subcommand)]
enum TeamCommands {
    /// Connect to a shared git remote for profile sync.
    Init {
        /// Git remote URL (SSH or HTTPS).
        remote_url: String,
        /// Branch to use (default: main).
        #[arg(long, default_value = "main")]
        branch: String,
    },
    /// Push local profile configs to the remote.
    Push,
    /// Pull profile configs from the remote.
    Pull {
        /// Override merge strategy: theirs (default), ours, merge.
        #[arg(long)]
        strategy: Option<String>,
    },
    /// Show sync status.
    Status,
}

#[derive(Subcommand)]
enum DaemonCommands {
    Start,
    Stop,
    Restart,
    Status,
    Logs,
}

#[derive(Subcommand)]
enum AutoSwitchCommands {
    /// Interactively configure fallback chain and reset estimate.
    Configure { profile: String },
    /// Show auto-switch event log.
    Log,
    /// Dry-run the fallback chain.
    Test { profile: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise tracing subscriber (respects RUST_LOG)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cst=info".parse()?),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        None => {
            // No subcommand → open TUI
            commands::tui::run().await
        }
        Some(Commands::Tui) => commands::tui::run().await,
        Some(Commands::ShellInit { shell }) => shell_cmd::shell_init(shell),
        Some(Commands::Env { profile_session }) => shell_cmd::env_cmd(&profile_session),
        Some(Commands::BroadcastSwitch { current, already_applied_id }) => {
            commands::switch_all::broadcast_switch_check(&current, &already_applied_id)
        }
        Some(Commands::AutoDetect { dir, current }) => {
            commands::auto_detect::check(&dir, &current)
        }
        Some(Commands::AutoDetectStatus { dir }) => {
            commands::auto_detect::status(&dir)
        }
        Some(Commands::Status) => commands::status::run(),
        Some(Commands::List) => commands::list::run(),
        Some(Commands::Remaining) => commands::quota::remaining(),
        Some(Commands::Top) => commands::top::run().await,
        Some(Commands::SwitchAll { from, to }) => commands::switch_all::run(&from, &to),
        Some(Commands::History) => commands::history::run(),
        Some(Commands::Why) => commands::history::why(),
        Some(Commands::New { name, auth, template }) => {
            profile_cmd::new(&name, &auth, template.as_deref()).await
        }
        Some(Commands::Import { r#as: alias }) => profile_cmd::import(alias.as_deref()),
        Some(Commands::Clone { source, destination }) => profile_cmd::clone(&source, &destination),
        Some(Commands::Rm { name }) => profile_cmd::remove(&name),
        Some(Commands::Rename { old, new }) => profile_cmd::rename(&old, &new),
        Some(Commands::Login { profile }) => profile_cmd::login(profile.as_deref()).await,
        Some(Commands::AddKey { profile, slot, source, note }) => {
            profile_cmd::add_key(&profile, slot, source.as_deref(), note.as_deref())
        }
        Some(Commands::Session { action }) => session_cmd::dispatch(action).await,
        Some(Commands::Daemon { action }) => commands::daemon::dispatch(action).await,
        Some(Commands::AutoSwitch { action }) => commands::auto_switch::dispatch(action).await,
        Some(Commands::Pause { minutes }) => commands::auto_switch::pause(minutes),
        Some(Commands::Run { profile_session, cmd }) => {
            commands::run::run_with_profile(&profile_session, &cmd).await
        }
        Some(Commands::Sync) => commands::sync::run(),
        Some(Commands::Stats { profile_session }) => {
            commands::stats::run(profile_session.as_deref())
        }
        Some(Commands::Doctor) => commands::doctor::run(),
        Some(Commands::Validate { profile }) => commands::doctor::validate(&profile),
        Some(Commands::Starship { config }) => {
            if config {
                commands::integrations::starship_config()
            } else {
                commands::integrations::starship()
            }
        }
        Some(Commands::Tmux { config }) => {
            if config {
                commands::integrations::tmux_config()
            } else {
                commands::integrations::tmux_segment()
            }
        }
        Some(Commands::Team { action }) => match action {
            TeamCommands::Init { remote_url, branch } => {
                commands::team::init(&remote_url, &branch)
            }
            TeamCommands::Push => commands::team::push(),
            TeamCommands::Pull { strategy } => commands::team::pull(strategy),
            TeamCommands::Status => commands::team::status(),
        },
        Some(Commands::Templates) => commands::templates::list(),
        Some(Commands::Init { yes, shell, no_daemon }) => {
            commands::init::run(yes, shell.as_deref(), !no_daemon).await
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut std::io::stdout());
            Ok(())
        }
        Some(Commands::Use { profile_session }) => {
            // `cst use` is normally handled by the shell function.
            // If called directly (not via eval), just print the env vars.
            let ps = profile_session.unwrap_or_else(|| "default:default".to_string());
            shell_cmd::env_cmd(&ps)
        }
    }
}
