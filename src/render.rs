use crate::{
    execute::{ExecutionResult, OperationState},
    filesystem::{path, Snapshot},
    plan::Plan,
};
use std::path::Path;
pub fn escaped(path: &Path) -> String {
    format!("{:?}", path.as_os_str())
}
pub fn timestamp(ns: i128) -> String {
    i64::try_from(ns.div_euclid(1_000_000_000))
        .ok()
        .and_then(|seconds| {
            chrono::DateTime::from_timestamp(seconds, ns.rem_euclid(1_000_000_000) as u32)
        })
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S%.9f %:z")
                .to_string()
        })
        .unwrap_or_else(|| format!("Unix ns {ns}"))
}
pub fn entry(path: &Path, s: &Snapshot) -> String {
    format!(
        "{}  {}  {} bytes  {}",
        escaped(path),
        if s.regular() {
            "file"
        } else if s.symlink() {
            "symlink"
        } else {
            "other"
        },
        s.size,
        timestamp(s.mtime_ns)
    )
}
pub fn preview(plan: &Plan, all: bool) -> String {
    let d = plan.data();
    let mut output = format!("{} {} files\nPlan: {}\nSHA-256: {}\nRoot: {}\nTotal: {} bytes\nConflicts: None at planning\nOverwrite: Disabled\nUndo: Available after verified move\n", d.action, d.operations.len(), d.plan_id, plan.hash(), escaped(&path(&d.root_path)), d.total_bytes);
    if let Some(q) = &d.query {
        output.push_str(&format!(
            "Extensions: {:?}\nModified before (exclusive): {}\nModified after (exclusive): {}\n",
            q.extensions,
            q.modified_before_ns
                .map(timestamp)
                .unwrap_or_else(|| "None".into()),
            q.modified_after_ns
                .map(timestamp)
                .unwrap_or_else(|| "None".into())
        ));
    }
    for op in d.operations.iter().take(if all { usize::MAX } else { 10 }) {
        output.push_str(&format!(
            "  {} → {}\n",
            escaped(&path(&op.source)),
            escaped(&path(&op.destination))
        ));
    }
    if !all && d.operations.len() > 10 {
        output.push_str(&format!(
            "  ... and {} more; use show-plan {} for every exact path\n",
            d.operations.len() - 10,
            d.plan_id
        ));
    }
    output
}
pub fn result(r: &ExecutionResult, id: &str) -> String {
    let n = |state| r.operations.iter().filter(|o| o.state == state).count();
    let verified = n(OperationState::Verified);
    let mut text = format!("Planned: {}\nAttempted: {}\nMoved: {}\nVerified: {}\nFailed: {}\nNot attempted: {}\nStatus: {}\nTransaction: {}\n", r.operations.len(), r.operations.len() - n(OperationState::NotAttempted), verified + n(OperationState::Completed), verified, r.operations.iter().filter(|o| o.error.is_some()).count(), n(OperationState::NotAttempted), r.status, id);
    for o in &r.operations {
        if let Some(e) = &o.error {
            text.push_str(&format!("{e}\n"));
        }
    }
    text
}
