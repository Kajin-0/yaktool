use crate::{
    error::{Error, Result},
    filesystem::{Root, Snapshot},
    intent::{Action, Age, Intent},
    plan::{Operation, Plan},
    policy,
};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::{os::unix::ffi::OsStrExt, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileQuery {
    pub scope: Vec<u8>,
    pub extensions: Vec<String>,
    pub min_size_bytes: Option<u64>,
    pub modified_before_ns: Option<i128>,
    pub modified_after_ns: Option<i128>,
    pub recursive: bool,
    pub include_hidden: bool,
}
pub struct Resolved {
    pub query: FileQuery,
    pub entries: Vec<(PathBuf, Snapshot)>,
    pub plan: Option<Plan>,
}
pub fn date_bounds(
    age: &Option<Age>,
    now: DateTime<Local>,
) -> Result<(Option<i128>, Option<i128>)> {
    let ns = |t: DateTime<Local>| {
        i128::from(t.timestamp()) * 1_000_000_000 + i128::from(t.timestamp_subsec_nanos())
    };
    match age {
        None => Ok((None, None)),
        Some(Age::OlderThan(n) | Age::NewerThan(n)) => {
            let t = now
                .checked_sub_signed(Duration::days(i64::from(*n)))
                .ok_or_else(|| Error::new("UNSUPPORTED_REQUEST", "Date out of range"))?;
            Ok(if matches!(age, Some(Age::OlderThan(_))) {
                (Some(ns(t)), None)
            } else {
                (None, Some(ns(t)))
            })
        }
        Some(Age::Today | Age::ThisWeek) => {
            let mut date = now.date_naive();
            if matches!(age, Some(Age::ThisWeek)) {
                date = date
                    .checked_sub_signed(Duration::days(i64::from(
                        date.weekday().num_days_from_monday(),
                    )))
                    .ok_or_else(|| Error::new("UNSUPPORTED_REQUEST", "Date out of range"))?;
            }
            let midnight = date
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| Error::new("UNSUPPORTED_REQUEST", "Invalid midnight"))?;
            let start = Local
                .from_local_datetime(&midnight)
                .single()
                .ok_or_else(|| {
                    Error::new(
                        "AMBIGUOUS_REQUEST",
                        "Local midnight is ambiguous or nonexistent",
                    )
                })?;
            // Query uses strict >. Subtract one ns to include midnight itself.
            Ok((Some(ns(now) + 1), Some(ns(start) - 1)))
        }
    }
}
pub fn resolve(root: &Root, intent: &Intent, now: DateTime<Local>) -> Result<Resolved> {
    if intent.schema_version != "yaktool.intent.v1" {
        return Err(Error::new("UNSUPPORTED_REQUEST", "Unknown intent version"));
    }
    if policy::decision(&intent.action) == policy::PolicyDecision::Deny {
        return Err(Error::new(
            if intent.action == Action::Clarify {
                "AMBIGUOUS_REQUEST"
            } else {
                "UNSUPPORTED_REQUEST"
            },
            intent.message.clone().unwrap_or_default(),
        ));
    }
    let source = root.location(
        intent
            .source
            .as_ref()
            .ok_or_else(|| Error::new("AMBIGUOUS_REQUEST", "Source required"))?,
    )?;
    if root.absent(&source)? {
        return Err(Error::new(
            "SOURCE_MISSING",
            "Source directory does not exist",
        ));
    }
    root.directory(&source)?;
    let (before, after) = date_bounds(&intent.filters.age, now)?;
    let query = FileQuery {
        scope: crate::filesystem::bytes(&source),
        extensions: intent.filters.extensions.clone(),
        min_size_bytes: intent.filters.min_size_bytes,
        modified_before_ns: before,
        modified_after_ns: after,
        recursive: false,
        include_hidden: false,
    };
    let mut entries = Vec::new();
    for (p, s) in root.entries(&source)? {
        if p.file_name()
            .is_some_and(|n| n.as_bytes().starts_with(b"."))
        {
            continue;
        }
        if !query.extensions.is_empty()
            && !p.extension().is_some_and(|ext| {
                query.extensions.iter().any(|e| {
                    ext.as_bytes()
                        .eq_ignore_ascii_case(e.trim_start_matches('.').as_bytes())
                })
            })
        {
            continue;
        }
        if query.min_size_bytes.is_some_and(|min| s.size <= min)
            || before.is_some_and(|t| s.mtime_ns >= t)
            || after.is_some_and(|t| s.mtime_ns <= t)
        {
            continue;
        }
        if intent.action == Action::CountFiles && !s.regular() {
            continue;
        }
        if intent.action == Action::CountDirectories && !s.directory() {
            continue;
        }
        if !matches!(intent.action, Action::List | Action::CountFiles | Action::CountDirectories) && !s.regular() {
            if intent.action == Action::Move {
                return Err(Error::new(
                    if s.symlink() {
                        "SYMLINK_REJECTED"
                    } else {
                        "UNSUPPORTED_FILE_TYPE"
                    },
                    "Matching move source is not a regular file",
                ));
            }
            if !s.symlink() {
                continue;
            }
        }
        entries.push((p, s));
    }
    let plan = if intent.action == Action::Move {
        let dest = root.location(
            intent
                .destination
                .as_ref()
                .ok_or_else(|| Error::new("AMBIGUOUS_REQUEST", "Destination required"))?,
        )?;
        if root.absent(&dest)? {
            return Err(Error::new(
                "DESTINATION_MISSING",
                "Destination directory does not exist; create it separately",
            ));
        }
        root.directory(&dest)?;
        let source_parent = root.snapshot(&source)?.identity();
        let destination_parent = root.snapshot(&dest)?.identity();
        let mut operations = Vec::new();
        let mut total = 0u64;
        for (p, snap) in &entries {
            let name = p
                .file_name()
                .ok_or_else(|| Error::new("UNSUPPORTED_FILE_TYPE", "Missing filename"))?;
            let target = dest.join(name);
            if !root.absent(&target)? {
                return Err(Error::new(
                    "DESTINATION_EXISTS",
                    format!("Destination occupied: {:?}", target),
                ));
            }
            if snap.device_id != destination_parent.device_id {
                return Err(Error::new(
                    "CROSS_FILESYSTEM",
                    "Cross-filesystem moves are not supported in YakTool V0.1.",
                ));
            }
            total = total
                .checked_add(snap.size)
                .ok_or_else(|| Error::new("BYTE_LIMIT_EXCEEDED", "Size overflow"))?;
            operations.push(Operation {
                source: crate::filesystem::bytes(p),
                destination: crate::filesystem::bytes(&target),
                source_snapshot: snap.clone(),
                source_parent: source_parent.clone(),
                destination_parent: destination_parent.clone(),
            });
        }
        policy::limits(operations.len(), total)?;
        Some(Plan::new(
            root,
            operations,
            Some(query.clone()),
            None,
            now.with_timezone(&Utc),
        )?)
    } else {
        None
    };
    Ok(Resolved {
        query,
        entries,
        plan,
    })
}
