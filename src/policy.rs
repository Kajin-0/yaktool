use crate::{
    error::{Error, Result},
    intent::Action,
};
pub const MAX_FILES: usize = 100;
pub const MAX_BYTES: u64 = 1_073_741_824;
#[derive(Debug, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Confirm,
    Deny,
}
pub fn decision(action: &Action) -> PolicyDecision {
    match action {
        Action::List | Action::Search | Action::FindLarge => PolicyDecision::Allow,
        Action::Move => PolicyDecision::Confirm,
        _ => PolicyDecision::Deny,
    }
}
pub fn limits(count: usize, bytes: u64) -> Result<()> {
    if count > MAX_FILES {
        return Err(Error::new(
            "FILE_LIMIT_EXCEEDED",
            "Maximum 100 files; no results were truncated",
        ));
    }
    if bytes > MAX_BYTES {
        return Err(Error::new(
            "BYTE_LIMIT_EXCEEDED",
            "Maximum 1 GiB (1,073,741,824 bytes)",
        ));
    }
    Ok(())
}
