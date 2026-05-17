//! clap-derive structures for all `uecm-cli` subcommands.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "uecm-cli", version, about = "UECM command-line interface")]
pub struct Cli {
    /// Emit machine-readable JSON / NDJSON instead of human-friendly output.
    #[arg(long, global = true)]
    pub json: bool,

    /// Override DB path (otherwise resolved via startup module).
    #[arg(long, global = true, env = "UECM_DB_PATH")]
    pub db_path: Option<String>,

    /// Log level for tracing output to stderr.
    #[arg(long, global = true, default_value = "warn")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Domain,
}

#[derive(Subcommand, Debug)]
pub enum Domain {
    /// Diagnostic / self-test commands.
    System {
        #[command(subcommand)]
        action: SystemAction,
    },
    /// Machine inventory + discovery.
    Machine {
        #[command(subcommand)]
        action: MachineAction,
    },
    /// WinRM probe + onboarding.
    Winrm {
        #[command(subcommand)]
        action: WinrmAction,
    },
    /// Credential storage (DPAPI + cmdkey + SQLite metadata).
    Cred {
        #[command(subcommand)]
        action: CredAction,
    },
    /// Read / write system-level environment variables on remote hosts.
    Env {
        #[command(subcommand)]
        action: EnvAction,
    },
    /// Read / write / remove single INI keys on remote hosts.
    Ini {
        #[command(subcommand)]
        action: IniAction,
    },
    /// SMB share inventory + creation + SYSTEM credential injection.
    Share {
        #[command(subcommand)]
        action: ShareAction,
    },
}

// ---------- system ----------
#[derive(Subcommand, Debug)]
pub enum SystemAction {
    /// Print binary + library version.
    Version,
    /// Print resolved SQLite DB path.
    DbPath,
    /// Print resolved ps-scripts directory.
    PsDir,
    /// Force-run schema migrations on the DB.
    MigrateDb,
    /// Round-trip a message through the PowerShell bridge.
    Echo { message: String },
}

// ---------- machine ----------
#[derive(Subcommand, Debug)]
pub enum MachineAction {
    /// List all known machines.
    List,
    /// Probe a CIDR for live hosts (ports 5985 / 445).
    Scan {
        /// CIDR (e.g. 192.168.10.0/24).
        cidr: String,
        /// Per-port TCP connect timeout (ms).
        #[arg(long, default_value_t = 1000)]
        timeout_ms: u64,
    },
    /// Add a machine to the inventory by IP / hostname.
    Add {
        #[arg(long)]
        ip: String,
        #[arg(long)]
        hostname: Option<String>,
    },
    /// Refresh a machine: WinRM probe + detect UE installs + GPUs.
    ///
    /// Plan 1 inherits the UI's no-credential semantics: WinRM runs in the
    /// caller's Kerberos/NTLM context.
    Refresh {
        /// Machine row id.
        id: i64,
    },
    /// Show machine detail (UE installs, GPUs, last-seen).
    Detail { id: i64 },
    /// Delete a machine row.
    Delete {
        id: i64,
        #[arg(long)]
        yes: bool,
    },
    /// Rename a machine.
    Rename { id: i64, hostname: String },
}

// ---------- winrm ----------
#[derive(Subcommand, Debug)]
pub enum WinrmAction {
    /// Probe a single host's WinRM endpoint.
    Probe { host: String },
    /// Print the manual WinRM enable script (no-arg PS1 body).
    BootstrapScript {
        /// Write to this file instead of stdout.
        #[arg(long)]
        output: Option<String>,
    },
    /// Remote bootstrap WinRM via PsExec.
    Bootstrap {
        host: String,
        #[arg(long)]
        user: String,
        /// Password (leaks into shell history; prefer --pass-stdin).
        #[arg(long, group = "bootstrap_secret", conflicts_with = "pass_stdin")]
        pass: Option<String>,
        /// Read password from stdin (one line).
        #[arg(long, group = "bootstrap_secret", conflicts_with = "pass")]
        pass_stdin: bool,
        #[arg(long)]
        enable_local_admin: bool,
    },
}

// ---------- cred ----------
#[derive(Subcommand, Debug)]
pub enum CredAction {
    /// List saved credential aliases.
    List,
    /// Save a credential (cmdkey + DPAPI + SQLite metadata).
    Save {
        #[arg(long)]
        alias: String,
        #[arg(long)]
        user: String,
        #[arg(long, group = "secret", conflicts_with = "pass_stdin")]
        pass: Option<String>,
        #[arg(long, group = "secret", conflicts_with = "pass")]
        pass_stdin: bool,
        #[arg(long, default_value = "winrm")]
        kind: String,
    },
    /// Delete a credential alias.
    Delete { alias: String },
}

// ---------- env ----------
#[derive(Subcommand, Debug)]
pub enum EnvAction {
    /// Read an environment variable on a single host.
    Get {
        #[arg(long)]
        host: String,
        #[arg(long)]
        name: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Write an environment variable on one or more hosts.
    Set {
        #[command(flatten)]
        target: crate::cli::host_args::HostArgs,
        #[arg(long)]
        name: String,
        #[arg(long)]
        value: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

// ---------- ini ----------
#[derive(Subcommand, Debug)]
pub enum IniAction {
    /// Read all keys from one INI section on a single host.
    Read {
        #[arg(long)]
        host: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        section: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Write a single INI key on one or more hosts.
    Set {
        #[command(flatten)]
        target: crate::cli::host_args::HostArgs,
        #[arg(long)]
        file: String,
        #[arg(long)]
        section: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Remove a single INI key on one or more hosts.
    Remove {
        #[command(flatten)]
        target: crate::cli::host_args::HostArgs,
        #[arg(long)]
        file: String,
        #[arg(long)]
        section: String,
        #[arg(long)]
        key: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

// ---------- share ----------
#[derive(Subcommand, Debug)]
pub enum ShareAction {
    /// List share configs in the local inventory.
    List,
    /// Forget a share config (LOCAL inventory only; remote SMB share is NOT removed).
    Forget {
        id: i64,
        #[arg(long)]
        yes: bool,
    },
    /// Create an SMB share (Mode A = open Guest+Everyone; Mode B = dedicated ddc-svc).
    Create {
        #[arg(long, value_name = "a|b")]
        mode: String,
        #[arg(long)]
        host: String,
        #[arg(long)]
        share: String,
        #[arg(long)]
        local_path: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Inject the share's SYSTEM-context credential on a client machine.
    InjectSystemCred {
        #[arg(long)]
        client_host: String,
        #[arg(long)]
        target_host: String,
        #[arg(long, default_value = "ddc-svc")]
        svc_user: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_machine_scan() {
        let cli = Cli::try_parse_from(["uecm-cli", "machine", "scan", "192.168.10.0/24"]).unwrap();
        match cli.command {
            Domain::Machine { action: MachineAction::Scan { cidr, timeout_ms } } => {
                assert_eq!(cidr, "192.168.10.0/24");
                assert_eq!(timeout_ms, 1000);
            }
            _ => panic!("wrong variant"),
        }
        assert!(!cli.json);
    }

    #[test]
    fn parses_global_json_flag_before_subcommand() {
        let cli = Cli::try_parse_from(["uecm-cli", "--json", "system", "version"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn parses_machine_refresh_by_id() {
        let cli = Cli::try_parse_from(["uecm-cli", "machine", "refresh", "3"]).unwrap();
        match cli.command {
            Domain::Machine { action: MachineAction::Refresh { id } } => {
                assert_eq!(id, 3);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn refresh_rejects_unknown_flag() {
        // Confirms we removed the credential flags in Plan 1 — extra flags must error.
        let res = Cli::try_parse_from([
            "uecm-cli", "machine", "refresh", "3", "--cred-alias", "winrm-admin",
        ]);
        assert!(res.is_err());
    }

    #[test]
    fn parses_cred_save_with_alias_and_user() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "cred", "save",
            "--alias", "winrm-admin",
            "--user", "Administrator",
            "--pass-stdin",
        ]).unwrap();
        match cli.command {
            Domain::Cred { action: CredAction::Save { alias, user, pass, pass_stdin, .. } } => {
                assert_eq!(alias, "winrm-admin");
                assert_eq!(user, "Administrator");
                assert_eq!(pass, None);
                assert!(pass_stdin);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cred_save_rejects_both_pass_and_pass_stdin() {
        let r = Cli::try_parse_from([
            "uecm-cli", "cred", "save",
            "--alias", "a", "--user", "u",
            "--pass", "p", "--pass-stdin",
        ]);
        assert!(r.is_err());
    }

    #[test]
    fn env_set_rejects_both_host_and_hosts() {
        let r = Cli::try_parse_from([
            "uecm-cli", "env", "set",
            "--host", "a", "--hosts", "b,c",
            "--name", "X", "--value", "Y",
        ]);
        assert!(r.is_err());
    }

    #[test]
    fn env_set_accepts_hosts_list() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "env", "set",
            "--hosts", "a,b,c",
            "--name", "X", "--value", "Y",
        ]).unwrap();
        match cli.command {
            Domain::Env { action: EnvAction::Set { target, name, value, .. } } => {
                assert_eq!(target.hosts, Some(vec!["a".into(), "b".into(), "c".into()]));
                assert_eq!(name, "X");
                assert_eq!(value, "Y");
            }
            _ => panic!("wrong variant"),
        }
    }
}
