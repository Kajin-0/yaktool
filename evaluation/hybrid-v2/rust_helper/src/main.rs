use std::io::{self, BufRead};
use yaktool::hybrid_interpret::{gate_model_move, route_without_model, RouteDecision};
use yaktool::intent::{Intent, Location};
use yaktool::model_intent::ModelIntent;

fn projection(i: &Intent) -> serde_json::Value {
    let loc = |x: &Option<Location>| match x {
        Some(Location::DirectoryAlias(v)) => serde_json::Value::String(v.clone()),
        _ => serde_json::Value::Null,
    };
    serde_json::json!({"action": i.action, "source": loc(&i.source), "destination": loc(&i.destination),
        "extensions": i.filters.extensions, "age": i.filters.age, "min_size_bytes": i.filters.min_size_bytes})
}

fn route_value(request: &str) -> serde_json::Value {
    match route_without_model(request) {
        Ok(RouteDecision::HardDeny(i)) => serde_json::json!({"route":"hard_deny", "intent":i, "projection":projection(&i)}),
        Ok(RouteDecision::Deterministic { stage, intent }) => serde_json::json!({"route":stage, "intent":intent, "projection":projection(&intent)}),
        Ok(RouteDecision::NeedsModel) => serde_json::json!({"route":"needs_model"}),
        Err(e) => serde_json::json!({"route":"error", "error":e.to_string()}),
    }
}

fn model_value(request: &str, raw: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::json!({"response_completed":true,"json_object":true,"model_intent_deserialized":false,
        "model_intent_valid":false,"normalization_valid":false,"gate_applied":false,"gate_accepted":false});
    let mi: ModelIntent = match serde_json::from_value(raw.clone()) {
        Ok(v) => { out["model_intent_deserialized"] = true.into(); v }
        Err(e) => { out["failure_stage"] = "deserialize".into(); out["failure_reason"] = e.to_string().into(); return out; }
    };
    if let Err(e) = mi.validate() {
        out["failure_stage"] = "validate".into(); out["failure_reason"] = e.to_string().into(); return out;
    }
    out["model_intent_valid"] = true.into();
    let normalized = match mi.normalize() {
        Ok(v) => v,
        Err(e) => { out["failure_stage"] = "normalize".into(); out["failure_reason"] = e.to_string().into(); return out; }
    };
    out["normalization_valid"] = true.into();
    out["normalized_intent"] = serde_json::to_value(&normalized).unwrap();
    out["normalized_projection"] = projection(&normalized);
    if mi.action == yaktool::model_intent::ModelAction::Move {
        out["gate_applied"] = true.into();
        match gate_model_move(request, &normalized) {
            Ok(()) => { out["gate_accepted"] = true.into(); out["final_intent"] = serde_json::to_value(&normalized).unwrap(); out["final_projection"] = projection(&normalized); }
            Err(e) => { out["failure_stage"] = "gate".into(); out["failure_reason"] = e.to_string().into(); }
        }
    } else {
        out["final_intent"] = serde_json::to_value(&normalized).unwrap(); out["final_projection"] = projection(&normalized);
    }
    out
}

fn main() {
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { println!("{}", "{\"response_completed\":true,\"json_object\":false,\"failure_stage\":\"json\"}"); continue };
        let request = v.get("request").and_then(|x| x.as_str()).unwrap_or("");
        let out = match v.get("model_intent") { Some(mi) => model_value(request, mi), None => route_value(request) };
        println!("{}", out);
    }
}
