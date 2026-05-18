//! Stub for R025. M6.1 extends with alt key names and section variants.

use crate::core::ini_diagnostics::ParsedFile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorDdcPrefs {
    pub global_local: Option<String>,
    pub global_shared: Option<String>,
    pub project_local: Option<String>,
    pub project_shared: Option<String>,
}

pub fn extract(file: &ParsedFile) -> EditorDdcPrefs {
    let mut out = EditorDdcPrefs::default();
    for s in &file.sections {
        if !s.name.eq_ignore_ascii_case("/Script/UnrealEd.EditorSettings") { continue; }
        for k in &s.keys {
            let n = k.name.as_str();
            let v = || k.value.trim().to_string();
            if n.eq_ignore_ascii_case("GlobalLocalDDCPath") && out.global_local.is_none() { out.global_local = Some(v()); }
            else if n.eq_ignore_ascii_case("GlobalSharedDDCPath") && out.global_shared.is_none() { out.global_shared = Some(v()); }
            else if n.eq_ignore_ascii_case("ProjectLocalDDCPath") && out.project_local.is_none() { out.project_local = Some(v()); }
            else if n.eq_ignore_ascii_case("ProjectSharedDDCPath") && out.project_shared.is_none() { out.project_shared = Some(v()); }
        }
    }
    out
}
