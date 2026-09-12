use crate::{
    error::{Error, Result},
    filesystem::{absent_at, bytes, entry_at, path, rename_noreplace, Root, Snapshot},
    journal::Journal,
    plan::{ConfirmedPlan, Operation, Plan},
    policy,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, ffi::OsString, fs::File, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    NotAttempted,
    Attempted,
    Completed,
    Failed,
    Verified,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationResult {
    pub state: OperationState,
    pub post_snapshot: Option<Snapshot>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub status: String,
    pub operations: Vec<OperationResult>,
}
fn invalid(message: impl Into<String>) -> Error {
    Error::new("PLAN_INVALIDATED", message)
}
fn parent(p: &Path) -> Result<&Path> {
    p.parent().ok_or_else(|| invalid("Missing parent"))
}
struct AnchoredOperation {
    source_parent: File,
    destination_parent: File,
    source: OsString,
    destination: OsString,
}
fn anchor_op(root: &Root, op: &Operation) -> Result<AnchoredOperation> {
    root.validate_live()?;
    let src = path(&op.source);
    let dst = path(&op.destination);
    let sp = root.directory(parent(&src)?)?;
    let dp = root.directory(parent(&dst)?)?;
    if Snapshot::from_meta(&sp.metadata()?).identity() != op.source_parent
        || Snapshot::from_meta(&dp.metadata()?).identity() != op.destination_parent
    {
        return Err(invalid("Parent directory changed"));
    }
    Ok(AnchoredOperation {
        source_parent: sp,
        destination_parent: dp,
        source: src
            .file_name()
            .ok_or_else(|| invalid("Missing source basename"))?
            .to_owned(),
        destination: dst
            .file_name()
            .ok_or_else(|| invalid("Missing destination basename"))?
            .to_owned(),
    })
}
fn validate_entries(root: &Root, op: &Operation, held: &AnchoredOperation) -> Result<File> {
    if Snapshot::from_meta(&held.source_parent.metadata()?).identity() != op.source_parent
        || Snapshot::from_meta(&held.destination_parent.metadata()?).identity()
            != op.destination_parent
    {
        return Err(invalid("Held parent metadata changed"));
    }
    root.contains_directory(&held.source_parent)?;
    root.contains_directory(&held.destination_parent)?;
    if !absent_at(&held.destination_parent, &held.destination)? {
        return Err(invalid("Destination is occupied"));
    }
    // Last filesystem check before rename: exactly the entry in the directory
    // used by renameat2. Keep this O_PATH FD alive through verification, preventing
    // inode recycling from making a replacement look like the confirmed object.
    let source = entry_at(&held.source_parent, &held.source)?;
    let snapshot = Snapshot::from_meta(&source.metadata()?);
    if snapshot != op.source_snapshot || !snapshot.regular() {
        return Err(invalid("Source object changed"));
    }
    if snapshot.device_id != op.destination_parent.device_id {
        return Err(Error::new(
            "CROSS_FILESYSTEM",
            "Cross-filesystem moves are not supported in YakTool V0.1.",
        ));
    }
    Ok(source)
}
fn validate_op(root: &Root, op: &Operation) -> Result<()> {
    let held = anchor_op(root, op)?;
    validate_entries(root, op, &held)?;
    Ok(())
}
pub fn preflight(root: &Root, plan: &Plan) -> Result<()> {
    let d = plan.data();
    root.validate_live()?;
    if !matches!(d.action.as_str(), "move" | "undo") || (d.action == "undo") != d.undo_of.is_some()
    {
        return Err(invalid("Invalid plan action"));
    }
    if d.root_path != bytes(root.path())
        || d.root_identity != root.identity()?
        || root.snapshot(root.path())?.identity() != d.root_identity
    {
        return Err(invalid("Approved root changed"));
    }
    policy::limits(d.operations.len(), d.total_bytes)?;
    let mut sources = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    let mut total = 0u64;
    for op in &d.operations {
        if !sources.insert(&op.source) || !destinations.insert(&op.destination) {
            return Err(invalid("Duplicate operation path"));
        }
        total = total
            .checked_add(op.source_snapshot.size)
            .ok_or_else(|| invalid("Size overflow"))?;
        validate_op(root, op).map_err(|e| invalid(e.to_string()))?;
    }
    if total != d.total_bytes || sources.iter().any(|s| destinations.contains(s)) {
        return Err(invalid("Inconsistent plan"));
    }
    Ok(())
}
fn verify(held: &AnchoredOperation, op: &Operation) -> Result<Snapshot> {
    let dest =
        Snapshot::from_meta(&entry_at(&held.destination_parent, &held.destination)?.metadata()?);
    if !dest.regular()
        || dest.identity() != op.source_snapshot.identity()
        || dest.size != op.source_snapshot.size
        || dest.mtime_ns != op.source_snapshot.mtime_ns
    {
        return Err(Error::new(
            "VERIFICATION_FAILED",
            "Destination identity or contents metadata changed",
        ));
    }
    if !absent_at(&held.source_parent, &held.source)? {
        return Err(Error::new(
            "VERIFICATION_FAILED",
            "Source pathname is occupied after rename",
        ));
    }
    Ok(dest)
}
pub fn execute(
    root: &Root,
    confirmed: &ConfirmedPlan,
    journal: &mut Journal,
) -> Result<ExecutionResult> {
    execute_inner(root, confirmed, journal, |_, _| {})
}
// Private observation points permit deterministic race/failure tests. The production
// entry point supplies a no-op and never accepts callbacks from an interpreter.
fn execute_inner(
    root: &Root,
    confirmed: &ConfirmedPlan,
    journal: &mut Journal,
    mut observe: impl FnMut(usize, &str),
) -> Result<ExecutionResult> {
    let plan = confirmed.plan();
    journal.confirm(plan)?;
    let mut result = ExecutionResult {
        status: "confirmed".into(),
        operations: plan
            .data()
            .operations
            .iter()
            .map(|_| OperationResult {
                state: OperationState::NotAttempted,
                post_snapshot: None,
                error: None,
            })
            .collect(),
    };
    if let Err(e) = preflight(root, plan) {
        result.status = "failed".into();
        journal.save_result(plan, &result)?;
        return Err(e);
    }
    result.status = "executing".into();
    journal.save_result(plan, &result)?;
    for (index, op) in plan.data().operations.iter().enumerate() {
        result.operations[index].state = OperationState::Attempted;
        journal.save_result(plan, &result)?; // Durable write-ahead evidence, before rename.
        let outcome = (|| -> Result<Snapshot> {
            observe(index, "before_validation");
            let held = anchor_op(root, op).map_err(|e| invalid(e.to_string()))?;
            observe(index, "parents_opened");
            let _source = validate_entries(root, op, &held).map_err(|e| invalid(e.to_string()))?;
            observe(index, "before_rename");
            rename_noreplace(
                &held.source_parent,
                &held.source,
                &held.destination_parent,
                &held.destination,
            )?;
            result.operations[index].state = OperationState::Completed;
            observe(index, "after_rename");
            journal.save_result(plan, &result)?;
            held.source_parent.sync_all()?;
            held.destination_parent.sync_all()?;
            verify(&held, op)
        })();
        match outcome {
            Ok(snapshot) => {
                result.operations[index].state = OperationState::Verified;
                result.operations[index].post_snapshot = Some(snapshot);
            }
            Err(e) => {
                if result.operations[index].state != OperationState::Completed {
                    result.operations[index].state = OperationState::Failed;
                }
                result.operations[index].error = Some(e.to_string());
                result.status = if result.operations.iter().any(|o| {
                    matches!(
                        o.state,
                        OperationState::Completed | OperationState::Verified
                    )
                }) {
                    "partial"
                } else {
                    "failed"
                }
                .into();
                journal.save_result(plan, &result)?;
                if let Some(id) = &plan.data().undo_of {
                    journal.finish_undo(id, false)?;
                }
                return Ok(result);
            }
        }
        journal.save_result(plan, &result)?;
    }
    result.status = "verified".into();
    journal.save_result(plan, &result)?;
    if let Some(id) = &plan.data().undo_of {
        journal.finish_undo(id, true)?;
    }
    Ok(result)
}

pub fn prepare_undo(root: &Root, journal: &Journal) -> Result<Plan> {
    let record = journal.undo_candidate()?;
    if record.plan.data().root_path != bytes(root.path())
        || record.plan.data().root_identity != root.identity()?
    {
        return Err(Error::new("UNDO_STATE_CHANGED", "Approved root differs"));
    }
    let result = record
        .result
        .ok_or_else(|| Error::new("UNDO_STATE_CHANGED", "Missing execution results"))?;
    if result.operations.len() != record.plan.data().operations.len() {
        return Err(Error::new("UNDO_STATE_CHANGED", "Inconsistent journal"));
    }
    let mut operations = Vec::new();
    for (op, outcome) in record.plan.data().operations.iter().zip(&result.operations) {
        if outcome.state != OperationState::Verified {
            continue;
        }
        operations.push(Operation {
            source: op.destination.clone(),
            destination: op.source.clone(),
            source_snapshot: outcome
                .post_snapshot
                .clone()
                .ok_or_else(|| Error::new("UNDO_STATE_CHANGED", "Missing verified snapshot"))?,
            source_parent: op.destination_parent.clone(),
            destination_parent: op.source_parent.clone(),
        });
    }
    let plan = Plan::new(
        root,
        operations,
        None,
        Some(record.plan.data().plan_id.clone()),
        chrono::Utc::now(),
    )?;
    preflight(root, &plan).map_err(|e| Error::new("UNDO_STATE_CHANGED", e.to_string()))?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        interpret::{Interpreter, RuleInterpreter},
        resolve::resolve,
    };
    use std::{fs, os::unix::fs::DirBuilderExt};

    fn setup() -> (tempfile::TempDir, Root, Plan, Journal) {
        let temp = tempfile::tempdir().unwrap();
        for d in ["Downloads", "Archive", "journal"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(temp.path().join(d))
                .unwrap();
        }
        for name in ["a.png", "b.png", "c.png"] {
            fs::write(temp.path().join("Downloads").join(name), name).unwrap();
        }
        let root = Root::new(temp.path()).unwrap();
        let plan = resolve(
            &root,
            &RuleInterpreter
                .interpret("move PNG files from Downloads to Archive")
                .unwrap(),
            chrono::Local::now(),
        )
        .unwrap()
        .plan
        .unwrap();
        let journal = Journal::open(&temp.path().join("journal/yaktool.db")).unwrap();
        journal.store(&plan).unwrap();
        (temp, root, plan, journal)
    }
    #[test]
    fn runtime_race_stops_and_partial_forward_is_undoable() {
        let (temp, root, plan, mut journal) = setup();
        let id = plan.data().plan_id.clone();
        let result = execute_inner(
            &root,
            &plan.confirm("y").unwrap(),
            &mut journal,
            |i, stage| {
                if i == 1 && stage == "before_rename" {
                    fs::write(temp.path().join("Archive/b.png"), b"raced").unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(result.status, "partial");
        assert_eq!(
            result
                .operations
                .iter()
                .map(|o| &o.state)
                .collect::<Vec<_>>(),
            vec![
                &OperationState::Verified,
                &OperationState::Failed,
                &OperationState::NotAttempted
            ]
        );
        assert_eq!(
            fs::read(temp.path().join("Archive/b.png")).unwrap(),
            b"raced"
        );
        assert!(temp.path().join("Downloads/b.png").exists());
        assert!(temp.path().join("Downloads/c.png").exists());
        assert_eq!(journal.get(&id).unwrap().status, "partial");
        let undo = prepare_undo(&root, &journal).unwrap();
        assert_eq!(undo.data().operations.len(), 1);
        journal.store(&undo).unwrap();
        execute(&root, &undo.confirm("yes").unwrap(), &mut journal).unwrap();
        assert!(temp.path().join("Downloads/a.png").exists());
        assert!(!temp.path().join("Archive/a.png").exists());
    }
    #[test]
    fn partial_undo_stops_without_overwriting_and_is_not_replayed() {
        let (temp, root, plan, mut journal) = setup();
        let id = plan.data().plan_id.clone();
        execute(&root, &plan.confirm("yes").unwrap(), &mut journal).unwrap();
        let undo = prepare_undo(&root, &journal).unwrap();
        let undo_id = undo.data().plan_id.clone();
        journal.store(&undo).unwrap();
        let result = execute_inner(
            &root,
            &undo.confirm("yes").unwrap(),
            &mut journal,
            |i, stage| {
                if i == 1 && stage == "before_rename" {
                    fs::write(temp.path().join("Downloads/b.png"), b"undo race").unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(result.status, "partial");
        assert_eq!(journal.get(&id).unwrap().status, "undo_failed");
        assert_eq!(journal.get(&undo_id).unwrap().status, "partial");
        assert_eq!(
            fs::read(temp.path().join("Downloads/b.png")).unwrap(),
            b"undo race"
        );
        assert!(temp.path().join("Archive/b.png").exists());
        assert!(temp.path().join("Archive/c.png").exists());
        assert!(temp.path().join("Downloads/a.png").exists());
        assert!(prepare_undo(&root, &journal).is_err());
    }
    #[test]
    fn postcondition_failure_is_not_reported_verified() {
        let (temp, root, plan, mut journal) = setup();
        let result = execute_inner(
            &root,
            &plan.confirm("yes").unwrap(),
            &mut journal,
            |i, stage| {
                if i == 0 && stage == "after_rename" {
                    fs::write(temp.path().join("Archive/a.png"), b"external modification").unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(result.status, "partial");
        assert_eq!(result.operations[0].state, OperationState::Completed);
        assert!(result.operations[0]
            .error
            .as_ref()
            .unwrap()
            .contains("VERIFICATION_FAILED"));
        assert_eq!(result.operations[1].state, OperationState::NotAttempted);
        assert!(prepare_undo(&root, &journal).is_err());
    }

    #[test]
    fn parent_names_cannot_redirect_validation_rename_or_verification() {
        use std::os::unix::fs::symlink;
        for directory in ["Downloads", "Archive"] {
            for stage_to_change in ["parents_opened", "before_rename", "after_rename"] {
                let (temp, root, plan, mut journal) = setup();
                let outside = tempfile::tempdir().unwrap();
                fs::write(outside.path().join("a.png"), b"outside sentinel").unwrap();
                let result = execute_inner(
                    &root,
                    &plan.confirm("yes").unwrap(),
                    &mut journal,
                    |i, stage| {
                        if i == 0 && stage == stage_to_change {
                            fs::rename(
                                temp.path().join(directory),
                                temp.path().join("held-directory"),
                            )
                            .unwrap();
                            symlink(outside.path(), temp.path().join(directory)).unwrap();
                        }
                    },
                )
                .unwrap();
                assert_eq!(result.operations[0].state, OperationState::Verified);
                // The next operation refuses the substituted parent name.
                assert_eq!(result.operations[1].state, OperationState::Failed);
                assert_eq!(result.status, "partial");
                let dest = if directory == "Archive" {
                    "held-directory/a.png"
                } else {
                    "Archive/a.png"
                };
                let src = if directory == "Downloads" {
                    "held-directory/a.png"
                } else {
                    "Downloads/a.png"
                };
                assert_eq!(fs::read(temp.path().join(dest)).unwrap(), b"a.png");
                assert!(!temp.path().join(src).exists());
                assert_eq!(
                    fs::read(outside.path().join("a.png")).unwrap(),
                    b"outside sentinel"
                );
                assert_eq!(
                    prepare_undo(&root, &journal).unwrap_err().code,
                    "UNDO_STATE_CHANGED"
                );
            }
        }
    }

    #[test]
    fn source_parent_symlink_before_resolution_is_rejected() {
        let (temp, root, plan, mut journal) = setup();
        let result = execute_inner(
            &root,
            &plan.confirm("yes").unwrap(),
            &mut journal,
            |i, stage| {
                if i == 0 && stage == "before_validation" {
                    fs::rename(
                        temp.path().join("Downloads"),
                        temp.path().join("held-directory"),
                    )
                    .unwrap();
                    std::os::unix::fs::symlink("held-directory", temp.path().join("Downloads"))
                        .unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(result.operations[0].state, OperationState::Failed);
        assert!(temp.path().join("held-directory/a.png").exists());
        assert_eq!(
            fs::read_dir(temp.path().join("Archive")).unwrap().count(),
            0
        );
    }

    #[test]
    fn final_relative_check_rejects_replaced_inode_for_move_and_undo() {
        for undo in [false, true] {
            let (temp, root, mut plan, mut journal) = setup();
            if undo {
                execute(&root, &plan.confirm("yes").unwrap(), &mut journal).unwrap();
                plan = prepare_undo(&root, &journal).unwrap();
                journal.store(&plan).unwrap();
            }
            let source_dir = if undo { "Archive" } else { "Downloads" };
            let dest_dir = if undo { "Downloads" } else { "Archive" };
            let source = temp.path().join(source_dir).join("a.png");
            let original = fs::File::open(&source).unwrap();
            let original_time = original.metadata().unwrap().modified().unwrap();
            let result = execute_inner(
                &root,
                &plan.confirm("yes").unwrap(),
                &mut journal,
                |i, stage| {
                    if i == 0 && stage == "parents_opened" {
                        fs::remove_file(&source).unwrap();
                        fs::write(&source, b"fake!").unwrap();
                        fs::File::options()
                            .write(true)
                            .open(&source)
                            .unwrap()
                            .set_times(fs::FileTimes::new().set_modified(original_time))
                            .unwrap();
                    }
                },
            )
            .unwrap();
            assert_eq!(result.operations[0].state, OperationState::Failed);
            assert!(result.operations[0]
                .error
                .as_ref()
                .unwrap()
                .contains("Source object changed"));
            assert_eq!(fs::read(&source).unwrap(), b"fake!");
            assert!(!temp.path().join(dest_dir).join("a.png").exists());
        }
    }

    #[test]
    fn held_parent_moved_outside_root_before_final_check_is_rejected() {
        for directory in ["Downloads", "Archive"] {
            let (temp, root, plan, mut journal) = setup();
            let outside = tempfile::tempdir().unwrap();
            let result = execute_inner(
                &root,
                &plan.confirm("yes").unwrap(),
                &mut journal,
                |i, stage| {
                    if i == 0 && stage == "parents_opened" {
                        fs::rename(
                            temp.path().join(directory),
                            outside.path().join("relocated"),
                        )
                        .unwrap();
                    }
                },
            )
            .unwrap();
            assert_eq!(result.operations[0].state, OperationState::Failed);
            let error = result.operations[0].error.as_ref().unwrap();
            // The ancestry walk can reach a mount boundary before filesystem root;
            // either refusal means it could not prove membership in the approved root.
            assert!(
                error.contains("OUTSIDE_ALLOWED_ROOT") || error.contains("CROSS_FILESYSTEM"),
                "{error}"
            );
            let source = if directory == "Downloads" {
                outside.path().join("relocated/a.png")
            } else {
                temp.path().join("Downloads/a.png")
            };
            assert_eq!(fs::read(source).unwrap(), b"a.png");
            let dest = if directory == "Archive" {
                outside.path().join("relocated")
            } else {
                temp.path().join("Archive")
            };
            assert_eq!(fs::read_dir(dest).unwrap().count(), 0);
        }
    }

    #[test]
    fn final_relative_source_check_rejects_symlink_without_following_target() {
        let (temp, root, plan, mut journal) = setup();
        fs::write(temp.path().join("target.png"), b"untouched").unwrap();
        let result = execute_inner(
            &root,
            &plan.confirm("yes").unwrap(),
            &mut journal,
            |i, stage| {
                if i == 0 && stage == "parents_opened" {
                    fs::remove_file(temp.path().join("Downloads/a.png")).unwrap();
                    std::os::unix::fs::symlink(
                        "../target.png",
                        temp.path().join("Downloads/a.png"),
                    )
                    .unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(result.operations[0].state, OperationState::Failed);
        assert!(fs::symlink_metadata(temp.path().join("Downloads/a.png"))
            .unwrap()
            .is_symlink());
        assert_eq!(
            fs::read(temp.path().join("target.png")).unwrap(),
            b"untouched"
        );
        assert!(!temp.path().join("Archive/a.png").exists());
    }

    #[test]
    fn substitution_in_residual_window_is_never_verified_or_guessed_for_undo() {
        let (temp, root, plan, mut journal) = setup();
        let id = plan.data().plan_id.clone();
        let result = execute_inner(
            &root,
            &plan.confirm("yes").unwrap(),
            &mut journal,
            |i, stage| {
                if i == 0 && stage == "before_rename" {
                    // Demonstrate the API limitation, then assert the surrounding fail-closed
                    // verification/journal boundary, not a claim of atomic source identity.
                    fs::rename(
                        temp.path().join("Downloads/a.png"),
                        temp.path().join("saved-original"),
                    )
                    .unwrap();
                    fs::write(temp.path().join("Downloads/a.png"), b"substitute").unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(result.operations[0].state, OperationState::Completed);
        assert!(result.operations[0]
            .error
            .as_ref()
            .unwrap()
            .contains("VERIFICATION_FAILED"));
        assert_eq!(result.operations[1].state, OperationState::NotAttempted);
        assert_eq!(
            fs::read(temp.path().join("saved-original")).unwrap(),
            b"a.png"
        );
        assert!(!journal.get(&id).unwrap().reversible);
        assert!(prepare_undo(&root, &journal).is_err());
    }
}
