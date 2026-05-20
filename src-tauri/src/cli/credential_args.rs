//! Shared `--cred-alias` / `--user --pass[--pass-stdin]` argument set, used by
//! every subcommand that authenticates against a remote host.

use crate::data::credentials as data_creds;
use crate::data::Db;
use crate::error::{UecmError, UecmResult};
use clap::Args;
use std::io::{self, BufRead};

#[derive(Args, Debug, Clone)]
pub struct CredentialArgs {
    /// Resolve credentials from a saved DPAPI alias.
    #[arg(long, value_name = "ALIAS", group = "cred")]
    pub cred_alias: Option<String>,

    /// Inline username; use with --pass or --pass-stdin.
    #[arg(long, value_name = "USER", group = "cred", requires = "secret")]
    pub user: Option<String>,

    /// Inline password. Leaks into shell history — prefer --pass-stdin
    /// or --cred-alias.
    #[arg(
        long,
        value_name = "PASS",
        group = "secret",
        conflicts_with_all = ["pass_stdin", "cred_alias"]
    )]
    pub pass: Option<String>,

    /// Read password from stdin (one line, \r\n trimmed).
    #[arg(
        long,
        group = "secret",
        conflicts_with_all = ["pass", "cred_alias"]
    )]
    pub pass_stdin: bool,
}

impl CredentialArgs {
    /// Validate the flag combination without reading stdin or DPAPI. Used by
    /// destructive-command dry-run / preflight paths so calling `resolve` on
    /// the real `--yes` path is not preempted (which would consume the
    /// `--pass-stdin` line and leave the second `resolve` hanging or empty).
    ///
    /// Catches:
    /// - `--cred-alias <X>` where X doesn't exist in SQLite metadata
    /// - inconsistent flag combinations (`--pass` without `--user`, etc.)
    ///
    /// Does NOT read stdin and does NOT call DPAPI — those run only on the
    /// real-execution path inside `resolve`.
    pub fn preflight(&self, db: &Db) -> UecmResult<()> {
        if let Some(alias) = &self.cred_alias {
            data_creds::find_by_alias(db, alias)?.ok_or_else(|| {
                UecmError::InvalidInput(format!("credential alias '{}' not found", alias))
            })?;
            return Ok(());
        }
        match (&self.user, &self.pass, self.pass_stdin) {
            (Some(_), Some(_), false) => Ok(()),
            (Some(_), None, true) => Ok(()), // stdin password read later by `resolve`
            (None, None, false) => Ok(()),
            _ => Err(UecmError::InvalidInput(
                "inconsistent credential flags".into(),
            )),
        }
    }

    /// Build a stdin-free `CredentialArgs` from an already-resolved credential.
    /// Used by orchestration commands that resolve once then fan out to many
    /// sub-handlers — calling `resolve` repeatedly would re-read `--pass-stdin`
    /// (only readable once) or re-hit DPAPI per sub-call.
    pub fn inline(resolved: Option<(String, String)>) -> Self {
        match resolved {
            Some((user, pass)) => CredentialArgs {
                cred_alias: None,
                user: Some(user),
                pass: Some(pass),
                pass_stdin: false,
            },
            None => CredentialArgs {
                cred_alias: None,
                user: None,
                pass: None,
                pass_stdin: false,
            },
        }
    }

    /// Resolve to `(username, password)` if any credential was supplied;
    /// `None` means inherit the caller's Kerberos/NTLM context.
    pub fn resolve(&self, db: &Db) -> UecmResult<Option<(String, String)>> {
        if let Some(alias) = &self.cred_alias {
            let user = data_creds::find_by_alias(db, alias)?
                .ok_or_else(|| {
                    UecmError::InvalidInput(format!("credential alias '{}' not found", alias))
                })?
                .username;
            let pass = crate::core::credentials::resolve_password(alias)?;
            return Ok(Some((user, pass)));
        }
        match (&self.user, &self.pass, self.pass_stdin) {
            (Some(u), Some(p), false) => Ok(Some((u.clone(), p.clone()))),
            (Some(u), None, true) => {
                let mut line = String::new();
                io::stdin().lock().read_line(&mut line).map_err(|e| {
                    UecmError::InvalidInput(format!("read password from stdin: {}", e))
                })?;
                let pass = line.trim_end_matches(['\r', '\n']).to_string();
                Ok(Some((u.clone(), pass)))
            }
            (None, None, false) => Ok(None),
            _ => Err(UecmError::InvalidInput(
                "inconsistent credential flags".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{open_in_memory, schema};

    fn fresh_db() -> Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    #[test]
    fn resolve_returns_none_when_no_flags_given() {
        let args = CredentialArgs {
            cred_alias: None,
            user: None,
            pass: None,
            pass_stdin: false,
        };
        let db = fresh_db();
        assert!(args.resolve(&db).unwrap().is_none());
    }

    #[test]
    fn inline_from_resolved_roundtrips_without_stdin() {
        let reused = CredentialArgs::inline(Some(("alice".into(), "pw".into())));
        let db = fresh_db();
        // resolve must not read stdin nor hit DPAPI — it just returns the inline pair.
        assert_eq!(reused.resolve(&db).unwrap(), Some(("alice".into(), "pw".into())));

        let none = CredentialArgs::inline(None);
        assert!(none.resolve(&db).unwrap().is_none());
    }

    #[test]
    fn resolve_inline_user_pass() {
        let args = CredentialArgs {
            cred_alias: None,
            user: Some("alice".into()),
            pass: Some("hunter2".into()),
            pass_stdin: false,
        };
        let db = fresh_db();
        assert_eq!(args.resolve(&db).unwrap(), Some(("alice".into(), "hunter2".into())));
    }

    #[test]
    fn resolve_unknown_alias_returns_invalid_input() {
        let args = CredentialArgs {
            cred_alias: Some("nope".into()),
            user: None,
            pass: None,
            pass_stdin: false,
        };
        let db = fresh_db();
        let r = args.resolve(&db);
        assert!(matches!(r, Err(UecmError::InvalidInput(_))));
    }
}
