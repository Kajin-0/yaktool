use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
};
use yaktool::{
    intent::Location,
    model_intent::{ModelAction, ModelIntent, ModelLocation},
};

#[test]
fn gold_corpus_is_frozen_valid_and_complete() {
    let corpus = fs::read_to_string("evaluation/model-intent-v1.jsonl").unwrap();
    let mut ids = HashSet::new();
    let mut requests = HashSet::new();
    let mut classes = HashMap::<String, usize>::new();
    for line in corpus.lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let id = row["id"].as_str().unwrap().to_string();
        let request = row["request"].as_str().unwrap().to_string();
        assert!(ids.insert(id));
        assert!(requests.insert(request));
        let expected: ModelIntent = serde_json::from_value(row["expected"].clone()).unwrap();
        expected.validate().unwrap();
        expected.normalize().unwrap();
        if expected.action == ModelAction::Move {
            assert_ne!(expected.source, ModelLocation::None);
            assert_ne!(expected.destination, ModelLocation::None);
        }
        if matches!(
            expected.action,
            ModelAction::Clarify | ModelAction::Unsupported
        ) {
            assert_eq!(
                expected,
                ModelIntent::default_with_action(expected.action.clone())
            );
        }
        *classes
            .entry(row["class"].as_str().unwrap().to_string())
            .or_default() += 1;
    }
    assert_eq!(ids.len(), 300);
    assert_eq!(requests.len(), 300);
    assert_eq!(
        classes,
        HashMap::from([
            ("straightforward_read_only".into(), 50),
            ("straightforward_move".into(), 50),
            ("paraphrase_case_punctuation_typos".into(), 45),
            ("multi_constraint".into(), 35),
            ("ambiguous_missing_information".into(), 35),
            ("unsupported_capabilities".into(), 35),
            ("negation_exclusions_corrections".into(), 25),
            ("adversarial_mixed_safe_unsafe".into(), 25)
        ])
    );
    let digest = format!("{:x}", Sha256::digest(corpus.as_bytes()));
    let manifest = fs::read_to_string("evaluation/model-intent-v1.sha256").unwrap();
    assert!(manifest.starts_with(&digest));
}

#[test]
fn model_schema_is_closed_and_has_no_authority_fields() {
    let schema = fs::read_to_string("schemas/model-intent-v1.json").unwrap();
    for forbidden in ["path", "path_bytes", "extensions", "shell", "command"] {
        assert!(
            !schema.contains(forbidden),
            "forbidden model field: {forbidden}"
        );
    }
    let value: serde_json::Value = serde_json::from_str(&schema).unwrap();
    assert_eq!(value["additionalProperties"], false);
    let fields = value["properties"].as_object().unwrap();
    assert_eq!(fields.len(), 10);
    assert!(fields.keys().all(|k| [
        "schema_version",
        "action",
        "source",
        "destination",
        "category",
        "age_relation",
        "age_days",
        "size_relation",
        "size_value",
        "size_unit"
    ]
    .contains(&k.as_str())));
}

#[test]
fn normalization_never_constructs_path_bytes() {
    let corpus = fs::read_to_string("evaluation/model-intent-v1.jsonl").unwrap();
    for line in corpus.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let intent: ModelIntent = serde_json::from_value(value["expected"].clone()).unwrap();
        let normalized = intent.normalize().unwrap();
        for location in [normalized.source, normalized.destination]
            .into_iter()
            .flatten()
        {
            assert!(
                !matches!(location, Location::PathBytes(_)),
                "ModelIntent produced PathBytes"
            );
        }
    }
}

trait DefaultWithAction {
    fn default_with_action(action: ModelAction) -> Self;
}
impl DefaultWithAction for ModelIntent {
    fn default_with_action(action: ModelAction) -> Self {
        ModelIntent {
            action,
            ..ModelIntent::default()
        }
    }
}
