//! clap-derive structures for all `uecm-cli` subcommands.

use clap::{Parser, Subcommand};

/// Operator-facing override for the cache backend (T3.6).
///
/// `Auto`   — defer to `core::cache_backend::resolve_for` decision table.
/// `Legacy` — force the legacy `.ddp` pak workflow (skip the router).
/// `Zen`    — force the zen no-op path. zen handles caching natively, so
///            `generate` / `verify` / `distribute` are no-ops that emit a
///            structured "skipped" summary and exit 0.
///
/// Exposed at the CLI layer only — `core::ddc_pak` / `core::pak_distribute`
/// are intentionally unaware of this gate so they can keep being unit-tested
/// without the routing surface.
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[clap(rename_all = "snake_case")]
pub enum BackendChoice {
    Auto,
    Legacy,
    Zen,
}

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
    /// uproject discovery + cross-machine identity.
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    /// Cluster health check (probes + derived consistency checks).
    Health {
        #[command(subcommand)]
        action: HealthAction,
    },
    /// GPU consistency matrix across the cluster.
    Gpu {
        #[command(subcommand)]
        action: GpuAction,
    },
    /// DDC pak workflow (generate / verify / distribute).
    Ddc {
        #[command(subcommand)]
        action: DdcAction,
    },
    /// PSO cache workflow (verify / collect / list / distribute).
    Pso {
        #[command(subcommand)]
        action: PsoAction,
    },
    /// Zen daemon inventory + probes + baselines (Plan 7 M1).
    Zen {
        #[command(subcommand)]
        action: ZenAction,
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
    /// Dump the full clap command tree + exit-code table as JSON. Intended
    /// for AI agents / automation to introspect this CLI's surface without
    /// scraping help text.
    Schema,
    /// Print the documented process exit-code table.
    ExitCodes,
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
    /// Plan 3: now accepts credentials. When supplied, all three remote
    /// calls (probe / detect_ue / detect_gpus) authenticate as the given
    /// user. Without credentials, the caller's Kerberos/NTLM context is used.
    Refresh {
        /// Machine row id.
        id: i64,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Show machine detail (UE installs, GPUs, last-seen).
    Detail { id: i64 },
    /// Delete a machine row.
    Delete {
        id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
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
    /// Preflight check whether Path B (remote PsExec bootstrap) is viable for a host.
    /// Default mode: TCP 135/445 + ADMIN$ mount + write probe (zero-trace on target).
    /// With --probe: also actually runs PsExec to test SCM service registration
    /// (writes one service install/remove pair to target Event Log).
    Preflight {
        /// Target host (IP or hostname).
        host: String,
        /// Local administrator username on target (e.g. Administrator).
        #[arg(long)]
        user: String,
        /// Password (leaks into shell history; prefer --pass-stdin).
        #[arg(long, group = "preflight_secret", conflicts_with = "pass_stdin")]
        pass: Option<String>,
        /// Read password from stdin (one line).
        #[arg(long, group = "preflight_secret", conflicts_with = "pass")]
        pass_stdin: bool,
        /// Run the actual PsExec SCM probe. Without this flag, preflight stops at
        /// ADMIN$ mount + write test (cannot detect UAC remote token filter blocks).
        #[arg(long)]
        probe: bool,
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
    Delete {
        alias: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
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
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
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
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
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
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Run cluster INI scan across one or more machines.
    Scan {
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',')]
        machine_ids: Vec<i64>,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// List recent INI scan runs.
    Runs {
        #[arg(long, default_value_t = 10)]
        limit: i64,
    },
    /// List findings for a given scan run.
    Findings {
        scan_run_id: i64,
        /// Filter by severity (critical / warning / healthy / info).
        #[arg(long)]
        severity: Option<String>,
    },
    /// Get one finding by id.
    GetFinding { finding_id: i64 },
    /// Apply (auto-fix) a finding's recommendation on the remote machine.
    Apply {
        finding_id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Mark a finding as skipped (won't apply).
    Skip { finding_id: i64 },
    /// Verify PSO precaching CVars (R008-R010) in a project's ConsoleVariables.ini.
    VerifyPsoPrecaching {
        #[arg(long)]
        project_id: i64,
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
        #[arg(long)]
        dry_run: bool,
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
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
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
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

// ---------- project ----------
#[derive(Subcommand, Debug)]
pub enum ProjectAction {
    /// List all projects.
    List,
    /// List all locations (machine + abs_path) for a project.
    Locations { project_id: i64 },
    /// Discover .uproject files on a remote machine under given search roots.
    Discover {
        #[arg(long)]
        machine_id: i64,
        #[arg(long, value_name = "R1,R2,...", value_delimiter = ',')]
        roots: Vec<String>,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Create a project manually (no discovery); yields a project_id.
    CreateManual {
        #[arg(long)]
        uproject_name: String,
        #[arg(long)]
        display_name: Option<String>,
    },
    /// Add or update a location for an existing project.
    SetLocation {
        #[arg(long)]
        project_id: i64,
        #[arg(long)]
        machine_id: i64,
        /// Absolute path to the directory containing the .uproject file.
        #[arg(long)]
        abs_path: String,
        /// Relative path (from abs_path root) to the .uproject file.
        #[arg(long)]
        uproject_path: String,
        /// Use ManualPath status instead of ManualAlias.
        #[arg(long)]
        manual_path: bool,
    },
    /// Delete a project (and cascade its locations).
    Delete {
        id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete a single project_location row.
    DeleteLocation {
        id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

// ---------- health ----------
#[derive(Subcommand, Debug)]
pub enum HealthAction {
    /// Run health probes — L1 port + L2 bootstrap + L3 business checkup with remediation hints.
    ///
    /// Target selection (exactly one of three modes):
    ///   --machine-ids 1,2,3     diagnose specific inventoried machines (persists results)
    ///   --cidr 192.168.10.0/24  L1 port-layer scan, stdout-only, no DB persistence
    ///   --all                   diagnose every machine in inventory (persists results)
    ///
    /// Credentials are optional. Without --cred-alias/--user, L2 + L3 probes are
    /// reported as `status=na` and counted in a separate `skipped` summary counter
    /// (not `healthy`/`critical`). L1 ports always run.
    Run {
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',',
              conflicts_with_all = ["cidr", "all"])]
        machine_ids: Vec<i64>,
        #[arg(long, conflicts_with_all = ["machine_ids", "all"])]
        cidr: Option<String>,
        #[arg(long, conflicts_with_all = ["machine_ids", "cidr"])]
        all: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// List recent health scan runs.
    Runs {
        #[arg(long, default_value_t = 10)]
        limit: i64,
    },
    /// List per-row health results for a scan run.
    Results { scan_run_id: i64 },
}

// ---------- gpu ----------
#[derive(Subcommand, Debug)]
pub enum GpuAction {
    /// Show GPU consistency matrix across all machines in inventory.
    Matrix,
}

// ---------- ddc ----------
#[derive(Subcommand, Debug)]
pub enum DdcAction {
    /// Generate a DDC pak file by running UE with -DDC=CreatePak.
    Generate {
        #[arg(long)]
        project_id: i64,
        #[arg(long)]
        source_machine: i64,
        /// Cache backend gate (T3.6). `auto` consults the routing table;
        /// `legacy` forces the .ddp pak workflow; `zen` is a no-op.
        #[arg(long, default_value = "auto", value_enum)]
        backend: BackendChoice,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Verify a previously generated .ddp pak file exists and has non-zero size.
    Verify {
        #[arg(long)]
        project_id: i64,
        #[arg(long)]
        source_machine: i64,
        /// Cache backend gate (T3.6). See `ddc generate --help` for semantics.
        #[arg(long, default_value = "auto", value_enum)]
        backend: BackendChoice,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Distribute the DDC pak to one or more target machines via Robocopy.
    Distribute {
        #[arg(long)]
        project_id: i64,
        #[arg(long)]
        source_machine: i64,
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',')]
        targets: Vec<i64>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        /// Cache backend gate (T3.6). See `ddc generate --help` for semantics.
        #[arg(long, default_value = "auto", value_enum)]
        backend: BackendChoice,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

// ---------- pso ----------
#[derive(Subcommand, Debug)]
pub enum PsoAction {
    /// Verify PSO precaching CVars (R008-R010) are set in the project's ConsoleVariables.ini.
    Verify {
        #[arg(long)]
        project_id: i64,
    },
    /// Run UE `-game` to collect PSO cache files. Long-running NDJSON stream.
    Collect {
        #[arg(long)]
        project_id: i64,
        #[arg(long)]
        source_machine: i64,
        #[arg(long, value_name = "WxH", default_value = "1920x1080")]
        resolution: String,
        #[arg(long, default_value_t = true)]
        windowed: bool,
        #[arg(long, default_value_t = 10)]
        max_minutes: u32,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// List collected PSO cache files for a project.
    List {
        #[arg(long)]
        project_id: i64,
    },
    /// Distribute PSO cache files to one or more target machines (with GPU mismatch preflight guard).
    Distribute {
        #[arg(long)]
        project_id: i64,
        #[arg(long)]
        source_machine: i64,
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',')]
        targets: Vec<i64>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

// ---------- zen (Plan 7 M1) ----------
#[derive(Subcommand, Debug)]
pub enum ZenAction {
    /// Read-only view of latest probe per endpoint.
    Status {
        /// Limit to one machine's endpoints (mutually exclusive with --all).
        #[arg(long, conflicts_with = "all")]
        machine: Option<i64>,
        /// Show endpoints across every machine (default).
        #[arg(long)]
        all: bool,
    },
    /// Probe one or more endpoints right now and persist a row each.
    Probe {
        #[arg(long, conflicts_with = "all")]
        machine: Option<i64>,
        #[arg(long)]
        all: bool,
        /// Per-endpoint timeout in seconds (HTTP connect + read).
        #[arg(long, default_value_t = 5)]
        timeout: u64,
        /// Reserved for future WinRM-tunneled probe — accepted but currently ignored.
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Fetch /stats + /stats/z$ now and persist a row.
    CacheStats {
        /// Limit to one endpoint by id (mutually exclusive with --all).
        #[arg(long, conflicts_with = "all")]
        endpoint_id: Option<i64>,
        #[arg(long)]
        all: bool,
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },
    /// Run the zen-detect-binary.ps1 sidecar against a machine and persist.
    DetectBinary {
        #[arg(long, conflicts_with = "all")]
        machine: Option<i64>,
        #[arg(long)]
        all: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Read-only list of registered zen endpoints.
    ListEndpoints {
        /// Limit to one machine's endpoints.
        #[arg(long)]
        machine: Option<i64>,
    },
    /// Baseline (zen_binary_expected) inspection and lock/unlock.
    Baseline {
        #[command(subcommand)]
        action: ZenBaselineAction,
    },
    /// Register a zen endpoint for a machine (idempotent on (machine, port)).
    Register {
        /// Machine row id this endpoint runs on.
        #[arg(long, value_name = "ID")]
        machine: i64,
        /// Port the endpoint advertises (Plan §1.1 default 8558).
        #[arg(long, value_name = "PORT", default_value_t = 8558)]
        declared_port: i64,
        /// URL scheme (plan §1.1 default `http`; HTTPS unsupported in M2).
        #[arg(long, value_name = "SCHEME", default_value = "http")]
        scheme: String,
        /// Endpoint role: `local` (this machine's own zen) or `shared_upstream`
        /// (cluster master other locals forward to).
        #[arg(long, value_name = "ROLE")]
        role: String,
        /// Existing `shared_upstream` endpoint id this endpoint forwards to.
        /// Required only when `--role local` should join a cluster.
        #[arg(long, value_name = "ID")]
        upstream_endpoint_id: Option<i64>,
        /// Absolute zen data directory on the target machine. Defaults to
        /// `D:\\UECM\\ZenData` if not given — operator should override per
        /// machine to match the real disk layout.
        #[arg(long, value_name = "PATH", default_value = r"D:\UECM\ZenData")]
        data_dir: String,
        /// zen HTTP server backend (asio default, httpsys for kernel-mode).
        #[arg(long, value_name = "CLASS", default_value = "asio")]
        httpserverclass: String,
        /// Lifecycle mode. Defaults derived from role per Plan §1.1:
        /// `shared_upstream` → `installed_service` (T2.1 enforces);
        /// `local` → `editor_owned`. Pass `--lifecycle` to override.
        #[arg(long, value_name = "MODE")]
        lifecycle: Option<String>,
    },
    /// Delete a registered endpoint. Refuses if other endpoints reference it
    /// as their upstream — un-point them first.
    Unregister {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Render zen.lua from the endpoint row + optional upstream and write it
    /// to the target host. `--dry-run` previews without invoking PowerShell.
    ApplyConfig {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        /// Absolute destination on the remote host (e.g.
        /// `C:\Users\<svc>\AppData\Local\UnrealEngine\Common\Zen\Install\zen.lua`).
        /// REQUIRED: T2.9 will derive this from the binary install dir; until
        /// then the caller must supply the real path so we never silently
        /// write to a placeholder while zen continues using a different file.
        #[arg(long, value_name = "PATH")]
        dest_path: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Render zen.lua to stdout (read-only). Same engine as apply-config
    /// `--dry-run`, but no destination path is required.
    LuaPreview {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
    },
    /// Windows-service management for the endpoint's zenserver.
    Service {
        #[command(subcommand)]
        action: ZenServiceAction,
    },
    /// URL ACL (`netsh http`) management for the endpoint.
    Urlacl {
        #[command(subcommand)]
        action: ZenUrlaclAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ZenBaselineAction {
    /// List baseline rows, optionally filtered.
    List {
        #[arg(long)]
        zen_build_version: Option<String>,
        /// Filter by binary kind (zen_cli | zenserver).
        #[arg(long, value_name = "KIND")]
        kind: Option<String>,
    },
    /// Set the `locked_by` marker on an existing baseline row.
    Lock {
        #[arg(long)]
        zen_build_version: String,
        #[arg(long, value_name = "KIND")]
        kind: String,
        #[arg(long)]
        locked_by: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Clear the `locked_by` marker on an existing baseline row.
    Unlock {
        #[arg(long)]
        zen_build_version: String,
        #[arg(long, value_name = "KIND")]
        kind: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

// ---------- zen service ----------
#[derive(Subcommand, Debug)]
pub enum ZenServiceAction {
    /// Install zenserver as a Windows service on the endpoint's host.
    Install {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Uninstall the zenserver Windows service.
    Uninstall {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Start the zenserver Windows service (idempotent).
    Start {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Stop the zenserver Windows service (idempotent). Destructive —
    /// stopping a `shared_upstream` cuts the whole cluster off, so the
    /// CLI requires `--yes` (or `--dry-run` to preview).
    Stop {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Report Windows-service status for zenserver.
    Status {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

// ---------- zen urlacl ----------
#[derive(Subcommand, Debug)]
pub enum ZenUrlaclAction {
    /// Reserve `<scheme>://+:<port>/` for the given user account.
    Add {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        /// Principal that may bind the prefix (e.g. `NT SERVICE\ZenServer`).
        /// Note: this is the URL ACL owner, NOT the WinRM auth user — clap
        /// would refuse to register both as `--user` on the same subcommand
        /// (`CredentialArgs` already owns that flag).
        #[arg(long, value_name = "PRINCIPAL")]
        principal: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// List zen-shaped URL reservations on a machine.
    List {
        #[arg(long, value_name = "ID")]
        machine: i64,
        /// Optional substring port filter (e.g. `8558`).
        #[arg(long, value_name = "PORT")]
        port_filter: Option<String>,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Remove the reservation for the endpoint's `<scheme>://+:<port>/`.
    Remove {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
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
            Domain::Machine { action: MachineAction::Refresh { id, cred } } => {
                assert_eq!(id, 3);
                assert!(cred.cred_alias.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn refresh_rejects_unknown_flag() {
        let res = Cli::try_parse_from([
            "uecm-cli", "machine", "refresh", "3", "--bogus-flag", "value",
        ]);
        assert!(res.is_err());
    }

    #[test]
    fn parses_machine_refresh_with_cred_alias() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "machine", "refresh", "3", "--cred-alias", "winrm-admin",
        ])
        .unwrap();
        match cli.command {
            Domain::Machine { action: MachineAction::Refresh { id, cred } } => {
                assert_eq!(id, 3);
                assert_eq!(cred.cred_alias.as_deref(), Some("winrm-admin"));
            }
            _ => panic!("wrong variant"),
        }
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

    #[test]
    fn parses_health_run_with_machine_ids() {
        let cli = Cli::try_parse_from(["uecm-cli", "health", "run", "--machine-ids", "1,2,3"]).unwrap();
        match cli.command {
            Domain::Health { action: HealthAction::Run { machine_ids, cidr, all, .. } } => {
                assert_eq!(machine_ids, vec![1, 2, 3]);
                assert_eq!(cidr, None);
                assert_eq!(all, false);
            }
            _ => panic!("expected Health::Run"),
        }
    }

    #[test]
    fn parses_health_run_with_cidr() {
        let cli = Cli::try_parse_from(["uecm-cli", "health", "run", "--cidr", "192.168.10.0/24"]).unwrap();
        match cli.command {
            Domain::Health { action: HealthAction::Run { cidr, .. } } => {
                assert_eq!(cidr.as_deref(), Some("192.168.10.0/24"));
            }
            _ => panic!("expected Health::Run"),
        }
    }

    #[test]
    fn parses_health_run_with_all_flag() {
        let cli = Cli::try_parse_from(["uecm-cli", "health", "run", "--all"]).unwrap();
        match cli.command {
            Domain::Health { action: HealthAction::Run { all, .. } } => assert!(all),
            _ => panic!("expected Health::Run"),
        }
    }

    #[test]
    fn parses_health_run_with_no_target_mode() {
        let cli = Cli::try_parse_from(["uecm-cli", "health", "run"]).unwrap();
        match cli.command {
            Domain::Health { action: HealthAction::Run { machine_ids, cidr, all, .. } } => {
                assert!(machine_ids.is_empty());
                assert_eq!(cidr, None);
                assert_eq!(all, false);
            }
            _ => panic!("expected Health::Run"),
        }
    }

    #[test]
    fn rejects_cidr_and_machine_ids_together() {
        let r = Cli::try_parse_from(["uecm-cli", "health", "run", "--cidr", "10.0.0.0/24", "--machine-ids", "1"]);
        assert!(r.is_err(), "should reject --cidr + --machine-ids");
    }

    #[test]
    fn rejects_all_and_cidr_together() {
        let r = Cli::try_parse_from(["uecm-cli", "health", "run", "--all", "--cidr", "10.0.0.0/24"]);
        assert!(r.is_err(), "should reject --all + --cidr");
    }

    // ---------- T3.6: ddc --backend flag ----------

    #[test]
    fn ddc_generate_backend_defaults_to_auto() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "ddc", "generate",
            "--project-id", "1",
            "--source-machine", "1",
        ]).unwrap();
        match cli.command {
            Domain::Ddc { action: DdcAction::Generate { backend, .. } } => {
                assert_eq!(backend, BackendChoice::Auto);
            }
            _ => panic!("expected Ddc::Generate"),
        }
    }

    #[test]
    fn ddc_generate_accepts_backend_zen() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "ddc", "generate",
            "--project-id", "1",
            "--source-machine", "1",
            "--backend", "zen",
        ]).unwrap();
        match cli.command {
            Domain::Ddc { action: DdcAction::Generate { backend, .. } } => {
                assert_eq!(backend, BackendChoice::Zen);
            }
            _ => panic!("expected Ddc::Generate"),
        }
    }

    #[test]
    fn ddc_verify_accepts_backend_legacy() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "ddc", "verify",
            "--project-id", "1",
            "--source-machine", "1",
            "--backend", "legacy",
        ]).unwrap();
        match cli.command {
            Domain::Ddc { action: DdcAction::Verify { backend, .. } } => {
                assert_eq!(backend, BackendChoice::Legacy);
            }
            _ => panic!("expected Ddc::Verify"),
        }
    }

    #[test]
    fn ddc_distribute_accepts_backend_zen() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "ddc", "distribute",
            "--project-id", "1",
            "--source-machine", "1",
            "--targets", "2,3",
            "--backend", "zen",
            "--yes",
        ]).unwrap();
        match cli.command {
            Domain::Ddc { action: DdcAction::Distribute { backend, .. } } => {
                assert_eq!(backend, BackendChoice::Zen);
            }
            _ => panic!("expected Ddc::Distribute"),
        }
    }

    #[test]
    fn ddc_generate_rejects_unknown_backend_value() {
        let r = Cli::try_parse_from([
            "uecm-cli", "ddc", "generate",
            "--project-id", "1",
            "--source-machine", "1",
            "--backend", "garbage",
        ]);
        assert!(r.is_err(), "clap must reject unknown --backend values");
    }
}
