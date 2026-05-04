//! Three-level project identity matcher: by-name, manual alias, manual path.
//! The v1 automatic matcher groups by `.uproject` filename stem.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredUproject {
    pub machine_id: i64,
    pub abs_path: String,
    pub uproject_path: String,
    pub uproject_filename: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    Auto,
    ManualAlias,
    ManualPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatchedProject {
    pub stem_lower: String,
    pub canonical_filename: String,
    pub locations: Vec<DiscoveredUproject>,
    pub match_kind: MatchKind,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatchOutcome {
    pub matched: Vec<MatchedProject>,
    pub ambiguous: Vec<DiscoveredUproject>,
}

pub fn stem_lower(filename: &str) -> String {
    let stem = filename
        .strip_suffix(".uproject")
        .or_else(|| filename.strip_suffix(".UPROJECT"))
        .unwrap_or(filename);
    stem.to_lowercase()
}

pub fn match_by_filename(items: Vec<DiscoveredUproject>) -> MatchOutcome {
    let mut groups: BTreeMap<String, Vec<DiscoveredUproject>> = BTreeMap::new();
    for item in items {
        let key = stem_lower(&item.uproject_filename);
        groups.entry(key).or_default().push(item);
    }

    let matched = groups
        .into_iter()
        .map(|(stem, locations)| MatchedProject {
            canonical_filename: locations[0].uproject_filename.clone(),
            stem_lower: stem,
            locations,
            match_kind: MatchKind::Auto,
        })
        .collect();

    MatchOutcome {
        matched,
        ambiguous: Vec::new(),
    }
}

pub fn manual_alias(
    stem_lower: String,
    canonical_filename: String,
    locations: Vec<DiscoveredUproject>,
) -> MatchedProject {
    MatchedProject {
        stem_lower,
        canonical_filename,
        locations,
        match_kind: MatchKind::ManualAlias,
    }
}

pub fn manual_path(
    stem_lower: String,
    canonical_filename: String,
    locations: Vec<DiscoveredUproject>,
) -> MatchedProject {
    MatchedProject {
        stem_lower,
        canonical_filename,
        locations,
        match_kind: MatchKind::ManualPath,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovered(machine_id: i64, abs_path: &str, filename: &str) -> DiscoveredUproject {
        DiscoveredUproject {
            machine_id,
            abs_path: abs_path.into(),
            uproject_path: format!("{}\\{}", abs_path, filename),
            uproject_filename: filename.into(),
        }
    }

    #[test]
    fn stem_lower_strips_extension_and_lowercases() {
        assert_eq!(stem_lower("Plurality.uproject"), "plurality");
        assert_eq!(stem_lower("MyProj.UPROJECT"), "myproj");
        assert_eq!(stem_lower("Already_lower.uproject"), "already_lower");
    }

    #[test]
    fn matches_two_machines_with_same_filename() {
        let out = match_by_filename(vec![
            discovered(1, "D:\\Work\\Plurality", "Plurality.uproject"),
            discovered(2, "E:\\Projects\\Plurality", "Plurality.uproject"),
        ]);
        assert_eq!(out.matched.len(), 1);
        assert_eq!(out.matched[0].locations.len(), 2);
        assert_eq!(out.matched[0].stem_lower, "plurality");
        assert_eq!(out.matched[0].match_kind, MatchKind::Auto);
    }

    #[test]
    fn separates_distinct_filenames() {
        let out = match_by_filename(vec![
            discovered(1, "D:\\X", "X.uproject"),
            discovered(2, "D:\\Y", "Y.uproject"),
        ]);
        assert_eq!(out.matched.len(), 2);
    }

    #[test]
    fn case_insensitive_grouping() {
        let out = match_by_filename(vec![
            discovered(1, "D:\\X", "MyProj.uproject"),
            discovered(2, "E:\\Y", "myproj.uproject"),
        ]);
        assert_eq!(out.matched.len(), 1);
        assert_eq!(out.matched[0].locations.len(), 2);
    }

    #[test]
    fn empty_input_yields_empty_outcome() {
        let out = match_by_filename(vec![]);
        assert!(out.matched.is_empty());
        assert!(out.ambiguous.is_empty());
    }

    #[test]
    fn manual_helpers_stamp_match_kind() {
        let loc = discovered(1, "D:\\X", "X.uproject");
        assert_eq!(
            manual_alias("x".into(), "X.uproject".into(), vec![loc.clone()]).match_kind,
            MatchKind::ManualAlias
        );
        assert_eq!(
            manual_path("x".into(), "X.uproject".into(), vec![loc]).match_kind,
            MatchKind::ManualPath
        );
    }
}
