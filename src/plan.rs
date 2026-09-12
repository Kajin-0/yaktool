use crate::{
    error::{Error, Result},
    filesystem::{bytes, Identity, Root, Snapshot},
    resolve::FileQuery,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub source: Vec<u8>,
    pub destination: Vec<u8>,
    pub source_snapshot: Snapshot,
    pub source_parent: Identity,
    pub destination_parent: Identity,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanData {
    pub schema_version: String,
    pub plan_id: String,
    pub created_at: String,
    pub action: String,
    pub root_path: Vec<u8>,
    pub root_identity: Identity,
    pub query: Option<FileQuery>,
    pub undo_of: Option<String>,
    pub operations: Vec<Operation>,
    pub total_bytes: u64,
}
#[derive(Clone, Debug)]
pub struct Plan {
    data: PlanData,
    hash: String,
}
pub struct ConfirmedPlan {
    plan: Plan,
}
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
impl Plan {
    pub fn new(
        root: &Root,
        mut operations: Vec<Operation>,
        query: Option<FileQuery>,
        undo_of: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self> {
        operations.sort_by(|a, b| a.source.cmp(&b.source));
        let total_bytes = operations
            .iter()
            .try_fold(0u64, |sum, op| sum.checked_add(op.source_snapshot.size))
            .ok_or_else(|| Error::new("BYTE_LIMIT_EXCEEDED", "Size overflow"))?;
        crate::policy::limits(operations.len(), total_bytes)?;
        let data = PlanData {
            schema_version: "yaktool.plan.v1".into(),
            plan_id: format!(
                "txn_{}_{}_{}",
                now.format("%Y%m%dT%H%M%S%.9f"),
                rustix::process::getpid().as_raw_nonzero(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ),
            created_at: now.to_rfc3339(),
            action: if undo_of.is_some() { "undo" } else { "move" }.into(),
            root_path: bytes(root.path()),
            root_identity: root.identity()?,
            query,
            undo_of,
            operations,
            total_bytes,
        };
        let hash = digest(&serde_json::to_string(&data)?);
        Ok(Self { data, hash })
    }
    pub fn data(&self) -> &PlanData {
        &self.data
    }
    pub fn hash(&self) -> &str {
        &self.hash
    }
    pub fn json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self.data)?)
    }
    pub fn stored(json: &str, hash: &str) -> Result<Self> {
        let data: PlanData = serde_json::from_str(json)?;
        let canonical = serde_json::to_string(&data)?;
        if data.schema_version != "yaktool.plan.v1" || canonical != json || digest(json) != hash {
            return Err(Error::new(
                "PLAN_INVALIDATED",
                "Stored plan integrity check failed",
            ));
        }
        Ok(Self {
            data,
            hash: hash.into(),
        })
    }
    pub fn confirm(self, response: &str) -> Option<ConfirmedPlan> {
        matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes")
            .then_some(ConfirmedPlan { plan: self })
    }
}
impl ConfirmedPlan {
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
}
fn digest(json: &str) -> String {
    format!("{:x}", Sha256::digest(json.as_bytes()))
}
