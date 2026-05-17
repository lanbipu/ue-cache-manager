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
