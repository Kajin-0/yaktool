//! The deliberately small, closed semantic contract emitted by a future model.
//! It contains no paths, filesystem metadata, policy, or executable operations.
use crate::{
    error::{Error, Result},
    intent::{Action, Age, Filters, Intent, Location},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAction {
    List,
    Search,
    FindLarge,
    Move,
    Clarify,
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLocation {
    None,
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Archive,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Any,
    Pdf,
    Png,
    Jpeg,
    Text,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgeRelation {
    None,
    OlderThan,
    NewerThan,
    Today,
    ThisWeek,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeRelation {
    None,
    LargerThan,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizeUnit {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "KB")]
    KB,
    #[serde(rename = "MB")]
    MB,
    #[serde(rename = "GB")]
    GB,
    #[serde(rename = "KiB")]
    KiB,
    #[serde(rename = "MiB")]
    MiB,
    #[serde(rename = "GiB")]
    GiB,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelIntent {
    pub schema_version: String,
    pub action: ModelAction,
    pub source: ModelLocation,
    pub destination: ModelLocation,
    pub category: Category,
    pub age_relation: AgeRelation,
    pub age_days: u32,
    pub size_relation: SizeRelation,
    pub size_value: u64,
    pub size_unit: SizeUnit,
}
impl Default for ModelIntent {
    fn default() -> Self {
        Self {
            schema_version: "yaktool.model_intent.v1".into(),
            action: ModelAction::Unsupported,
            source: ModelLocation::None,
            destination: ModelLocation::None,
            category: Category::Any,
            age_relation: AgeRelation::None,
            age_days: 0,
            size_relation: SizeRelation::None,
            size_value: 0,
            size_unit: SizeUnit::None,
        }
    }
}
impl ModelIntent {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != "yaktool.model_intent.v1" {
            return Err(Error::new(
                "MODEL_INTENT_INVALID",
                "Unknown ModelIntent schema version",
            ));
        }
        match self.age_relation {
            AgeRelation::OlderThan | AgeRelation::NewerThan if self.age_days == 0 => {
                return Err(Error::new(
                    "MODEL_INTENT_INVALID",
                    "Age day count must be positive",
                ))
            }
            AgeRelation::None | AgeRelation::Today | AgeRelation::ThisWeek
                if self.age_days != 0 =>
            {
                return Err(Error::new(
                    "MODEL_INTENT_INVALID",
                    "This age relation requires age_days=0",
                ))
            }
            _ => (),
        }
        match self.size_relation {
            SizeRelation::None if self.size_value != 0 || self.size_unit != SizeUnit::None => {
                return Err(Error::new(
                    "MODEL_INTENT_INVALID",
                    "No size filter may carry a value or unit",
                ))
            }
            SizeRelation::LargerThan
                if self.size_value == 0 || self.size_unit == SizeUnit::None =>
            {
                return Err(Error::new(
                    "MODEL_INTENT_INVALID",
                    "A size filter requires a positive value and unit",
                ))
            }
            _ => (),
        }
        let empty = self.source == ModelLocation::None
            && self.destination == ModelLocation::None
            && self.category == Category::Any
            && self.age_relation == AgeRelation::None
            && self.age_days == 0
            && self.size_relation == SizeRelation::None
            && self.size_value == 0
            && self.size_unit == SizeUnit::None;
        match self.action {
            ModelAction::List => {
                if self.source == ModelLocation::None
                    || self.destination != ModelLocation::None
                    || self.category != Category::Any
                    || self.age_relation != AgeRelation::None
                    || self.size_relation != SizeRelation::None
                {
                    return Err(Error::new(
                        "MODEL_INTENT_INVALID",
                        "List requires only a source",
                    ));
                }
            }
            ModelAction::Search => {
                if self.source == ModelLocation::None || self.destination != ModelLocation::None {
                    return Err(Error::new(
                        "MODEL_INTENT_INVALID",
                        "Search requires a source and no destination",
                    ));
                }
            }
            ModelAction::FindLarge => {
                if self.source == ModelLocation::None
                    || self.destination != ModelLocation::None
                    || self.size_relation != SizeRelation::LargerThan
                {
                    return Err(Error::new(
                        "MODEL_INTENT_INVALID",
                        "FindLarge requires a source and larger-than size filter",
                    ));
                }
            }
            ModelAction::Move => {
                if self.source == ModelLocation::None
                    || self.destination == ModelLocation::None
                    || self.source == self.destination
                {
                    return Err(Error::new(
                        "MODEL_INTENT_INVALID",
                        "Move requires distinct source and destination",
                    ));
                }
            }
            ModelAction::Clarify | ModelAction::Unsupported => {
                if !empty {
                    return Err(Error::new(
                        "MODEL_INTENT_INVALID",
                        "Clarify and unsupported intents require empty semantic slots",
                    ));
                }
            }
        }
        Ok(())
    }
    pub fn normalize(&self) -> Result<Intent> {
        self.validate()?;
        let action = match self.action {
            ModelAction::List => Action::List,
            ModelAction::Search => Action::Search,
            ModelAction::FindLarge => Action::FindLarge,
            ModelAction::Move => Action::Move,
            ModelAction::Clarify => Action::Clarify,
            ModelAction::Unsupported => Action::Unsupported,
        };
        let mut intent = Intent::new(action);
        intent.source = location(&self.source);
        intent.destination = location(&self.destination);
        intent.filters = Filters {
            extensions: match self.category {
                Category::Any => vec![],
                Category::Pdf => vec![".pdf".into()],
                Category::Png => vec![".png".into()],
                Category::Jpeg => vec![".jpg".into(), ".jpeg".into()],
                Category::Text => vec![".txt".into()],
            },
            min_size_bytes: size_bytes(
                self.size_relation.clone(),
                self.size_value,
                &self.size_unit,
            )?,
            age: match self.age_relation {
                AgeRelation::None => None,
                AgeRelation::OlderThan => Some(Age::OlderThan(self.age_days)),
                AgeRelation::NewerThan => Some(Age::NewerThan(self.age_days)),
                AgeRelation::Today => Some(Age::Today),
                AgeRelation::ThisWeek => Some(Age::ThisWeek),
            },
        };
        if matches!(self.action, ModelAction::Clarify) {
            intent.message =
                Some("Clarification is required before YakTool can form an operation.".into());
        }
        if matches!(self.action, ModelAction::Unsupported) {
            intent.message =
                Some("This request is outside YakTool's supported capabilities.".into());
        }
        Ok(intent)
    }
}
fn location(value: &ModelLocation) -> Option<Location> {
    let alias = match value {
        ModelLocation::None => return None,
        ModelLocation::Home => "home",
        ModelLocation::Desktop => "desktop",
        ModelLocation::Documents => "documents",
        ModelLocation::Downloads => "downloads",
        ModelLocation::Pictures => "pictures",
        ModelLocation::Archive => "archive",
    };
    Some(Location::DirectoryAlias(alias.into()))
}
fn size_bytes(relation: SizeRelation, value: u64, unit: &SizeUnit) -> Result<Option<u64>> {
    if relation == SizeRelation::None {
        return Ok(None);
    }
    let multiplier = match unit {
        SizeUnit::KB => 1_000,
        SizeUnit::MB => 1_000_000,
        SizeUnit::GB => 1_000_000_000,
        SizeUnit::KiB => 1_024,
        SizeUnit::MiB => 1_048_576,
        SizeUnit::GiB => 1_073_741_824,
        SizeUnit::None => return Err(Error::new("MODEL_INTENT_INVALID", "Missing size unit")),
    };
    value
        .checked_mul(multiplier)
        .map(Some)
        .ok_or_else(|| Error::new("MODEL_INTENT_INVALID", "Size conversion overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid(action: ModelAction) -> ModelIntent {
        ModelIntent {
            action,
            ..ModelIntent::default()
        }
    }
    #[test]
    fn all_enums_serialize_closed() {
        for value in [
            ModelAction::List,
            ModelAction::Search,
            ModelAction::FindLarge,
            ModelAction::Move,
            ModelAction::Clarify,
            ModelAction::Unsupported,
        ] {
            let _ = serde_json::to_string(&value).unwrap();
        }
        for value in [
            ModelLocation::None,
            ModelLocation::Home,
            ModelLocation::Desktop,
            ModelLocation::Documents,
            ModelLocation::Downloads,
            ModelLocation::Pictures,
            ModelLocation::Archive,
        ] {
            let _ = serde_json::to_string(&value).unwrap();
        }
    }
    #[test]
    fn semantic_validation_is_fail_closed() {
        let mut i = valid(ModelAction::List);
        assert!(i.validate().is_err());
        i.source = ModelLocation::Downloads;
        assert!(i.validate().is_ok());
        i.destination = ModelLocation::Archive;
        assert!(i.validate().is_err());
        let mut i = valid(ModelAction::Clarify);
        i.category = Category::Png;
        assert!(i.validate().is_err());
    }
    #[test]
    fn normalization_has_no_path_bytes_route() {
        let mut i = valid(ModelAction::Move);
        i.source = ModelLocation::Downloads;
        i.destination = ModelLocation::Archive;
        i.category = Category::Jpeg;
        i.age_relation = AgeRelation::OlderThan;
        i.age_days = 30;
        i.size_relation = SizeRelation::LargerThan;
        i.size_value = 500;
        i.size_unit = SizeUnit::MB;
        let n = i.normalize().unwrap();
        assert!(matches!(n.source, Some(Location::DirectoryAlias(_))));
        assert!(matches!(n.destination, Some(Location::DirectoryAlias(_))));
        assert_eq!(n.filters.extensions, vec![".jpg", ".jpeg"]);
        assert_eq!(n.filters.min_size_bytes, Some(500_000_000));
        assert!(matches!(n.filters.age, Some(Age::OlderThan(30))));
    }
    #[test]
    fn size_units_and_overflow() {
        let mut i = valid(ModelAction::FindLarge);
        i.source = ModelLocation::Downloads;
        i.size_relation = SizeRelation::LargerThan;
        for (unit, expected) in [
            (SizeUnit::KB, 1_000),
            (SizeUnit::MB, 1_000_000),
            (SizeUnit::GB, 1_000_000_000),
            (SizeUnit::KiB, 1_024),
            (SizeUnit::MiB, 1_048_576),
            (SizeUnit::GiB, 1_073_741_824),
        ] {
            i.size_value = 1;
            i.size_unit = unit;
            assert_eq!(
                i.normalize().unwrap().filters.min_size_bytes,
                Some(expected)
            );
        }
        i.size_value = u64::MAX;
        assert!(i.normalize().is_err());
    }
    #[test]
    fn empty_action_messages_are_deterministic() {
        for action in [ModelAction::Clarify, ModelAction::Unsupported] {
            let i = valid(action);
            let n = i.normalize().unwrap();
            assert!(n.message.is_some());
            assert!(n.source.is_none());
            assert!(n.destination.is_none());
        }
    }
}
