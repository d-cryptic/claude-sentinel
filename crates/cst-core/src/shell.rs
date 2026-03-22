//! Shell integration — generate `shell-init` code and `_env` exports.

use std::collections::HashMap;

/// Shell types supported for init code generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellKind {
    Zsh,
    Bash,
    Fish,
    PowerShell,
}

impl ShellKind {
    /// Detect shell from `$SHELL` env var.
    pub fn detect() -> Self {
        let shell = std::env::var("SHELL").unwrap_or_default();
        if shell.contains("zsh") {
            Self::Zsh
        } else if shell.contains("fish") {
            Self::Fish
        } else if shell.contains("bash") {
            Self::Bash
        } else {
            Self::Bash // safe default on Unix
        }
    }
}

/// Generate the shell init code (emitted by `cst shell-init`).
/// The user adds `eval "$(cst shell-init)"` to their rc file.
pub fn shell_init_code(shell: &ShellKind) -> String {
    match shell {
        ShellKind::Zsh | ShellKind::Bash => {
            r#"
# claude-sentinel shell integration
cst() {
    case "$1" in
        use)
            if [ -z "$2" ]; then
                command cst tui
            else
                eval "$(command cst _env "$2" 2>&1)"
            fi
            ;;
        switch-all)
            # switch-all also switches the current shell immediately
            command cst switch-all "$2" "$3"
            if [ -n "$3" ]; then
                eval "$(command cst _env "${3}:${CST_CURRENT#*:}" 2>&1)"
            fi
            ;;
        *)
            command cst "$@"
            ;;
    esac
}

# Auto-switch check: runs before each prompt
_cst_check_switch() {
    # 1. One-shot pending switch (daemon-initiated for this specific shell)
    local switch_file="${HOME}/.claude-sentinel/pending-switch"
    if [ -f "$switch_file" ]; then
        eval "$(cat "$switch_file")" 2>/dev/null
        rm -f "$switch_file"
        printf '⚡ claude-sentinel: switched to %s\n' "$CST_CURRENT" >&2
    fi

    # 2. Broadcast switch (switch-all — applies to all shells running the from-profile)
    if [ -n "${CST_CURRENT:-}" ]; then
        local _cst_bc
        _cst_bc="$(command cst _broadcast-switch "${CST_CURRENT}" "${CST_BROADCAST_ID:-}" 2>/dev/null)"
        if [ -n "$_cst_bc" ]; then
            eval "$_cst_bc"
            printf '⚡ claude-sentinel: broadcast → %s\n' "$CST_CURRENT" >&2
        fi
    fi
}

if [ -n "$ZSH_VERSION" ]; then
    precmd_functions+=(_cst_check_switch)
elif [ -n "$BASH_VERSION" ]; then
    PROMPT_COMMAND="${PROMPT_COMMAND:+${PROMPT_COMMAND}; }_cst_check_switch"
fi
"#
            .trim()
            .to_string()
        }
        ShellKind::Fish => {
            r#"
# claude-sentinel shell integration (fish)
function cst
    if test "$argv[1]" = "use"
        if test -z "$argv[2]"
            command cst tui
        else
            eval (command cst _env "$argv[2]" 2>&1)
        end
    else
        command cst $argv
    end
end

function _cst_check_switch --on-event fish_prompt
    set switch_file "$HOME/.claude-sentinel/pending-switch"
    if test -f $switch_file
        eval (cat $switch_file) 2>/dev/null
        rm -f $switch_file
        echo "⚡ claude-sentinel: switched to $CST_CURRENT" >&2
    end
    if test -n "$CST_CURRENT"
        set _cst_bc (command cst _broadcast-switch "$CST_CURRENT" "$CST_BROADCAST_ID" 2>/dev/null)
        if test -n "$_cst_bc"
            eval $_cst_bc
            echo "⚡ claude-sentinel: broadcast → $CST_CURRENT" >&2
        end
    end
end
"#
            .trim()
            .to_string()
        }
        ShellKind::PowerShell => {
            r#"
# claude-sentinel shell integration (PowerShell)
function cst {
    if ($args[0] -eq "use") {
        if (-not $args[1]) {
            & cst.exe tui
        } else {
            Invoke-Expression (& cst.exe _env $args[1] 2>&1)
        }
    } else {
        & cst.exe @args
    }
}
"#
            .trim()
            .to_string()
        }
    }
}

/// Generate env export lines (emitted by `cst _env <profile:session>`).
/// The shell function eval's this output.
pub fn env_exports(env_vars: &HashMap<String, String>, shell: &ShellKind) -> String {
    let mut lines = Vec::new();
    for (key, val) in env_vars {
        let line = match shell {
            ShellKind::Zsh | ShellKind::Bash => {
                format!("export {key}='{val}'")
            }
            ShellKind::Fish => {
                format!("set -gx {key} '{val}'")
            }
            ShellKind::PowerShell => {
                format!("$env:{key} = '{val}'")
            }
        };
        lines.push(line);
    }
    lines.sort(); // deterministic output
    lines.join("\n")
}

/// Parse `"profile:session"` into `(profile, session)`.
/// If no `:` is present, session defaults to `"default"`.
pub fn parse_profile_session(input: &str) -> (String, String) {
    match input.split_once(':') {
        Some((p, s)) => (p.to_string(), s.to_string()),
        None => (input.to_string(), "default".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_profile_session_with_colon() {
        let (p, s) = parse_profile_session("work:backend");
        assert_eq!(p, "work");
        assert_eq!(s, "backend");
    }

    #[test]
    fn test_parse_profile_session_without_colon() {
        let (p, s) = parse_profile_session("personal");
        assert_eq!(p, "personal");
        assert_eq!(s, "default");
    }

    #[test]
    fn test_env_exports_bash_format() {
        let mut vars = HashMap::new();
        vars.insert("CLAUDE_CONFIG_DIR".to_string(), "/home/user/.claude-sentinel/...".to_string());
        let output = env_exports(&vars, &ShellKind::Bash);
        assert!(output.contains("export CLAUDE_CONFIG_DIR="));
    }

    #[test]
    fn test_env_exports_fish_format() {
        let mut vars = HashMap::new();
        vars.insert("CST_CURRENT".to_string(), "work:backend".to_string());
        let output = env_exports(&vars, &ShellKind::Fish);
        assert!(output.contains("set -gx CST_CURRENT"));
    }

    #[test]
    fn test_shell_init_code_contains_function() {
        let code = shell_init_code(&ShellKind::Zsh);
        assert!(code.contains("function cst") || code.contains("cst()"));
        assert!(code.contains("_cst_check_switch"));
    }
}
