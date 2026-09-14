use std::io::{self, BufRead};
use yaktool::hybrid_interpret::{route_without_model, RouteDecision};
use yaktool::model_client::SemanticModel;
use yaktool::model_intent::ModelIntent;
use yaktool::error::{Result, Error};
struct Injected(Option<ModelIntent>);
impl SemanticModel for Injected { fn interpret(&self, _: &str) -> Result<ModelIntent> { self.0.clone().ok_or_else(|| Error::new("MODEL_UNAVAILABLE","missing injected intent")) } }
fn main() {
 for line in io::stdin().lock().lines() {
  let v: serde_json::Value=serde_json::from_str(&line.unwrap()).unwrap(); let request=v["request"].as_str().unwrap().to_owned();
  let out = if let Some(mi)=v.get("model_intent") { match serde_json::from_value::<ModelIntent>(mi.clone()).ok().and_then(|m| yaktool::hybrid_interpret::interpret(&request,&Injected(Some(m))).ok()) { Some(i)=>serde_json::json!({"route":"model","accepted":true,"intent":i}), None=>serde_json::json!({"route":"model","accepted":false}) } } else { match route_without_model(&request).unwrap() { RouteDecision::HardDeny(i)=>serde_json::json!({"route":"hard_deny","intent":i}), RouteDecision::Deterministic{stage,intent}=>serde_json::json!({"route":stage,"intent":intent}), RouteDecision::NeedsModel=>serde_json::json!({"route":"needs_model"}) } };
  println!("{}", out);
 }
}
