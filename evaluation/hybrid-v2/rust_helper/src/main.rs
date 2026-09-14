use std::io::{self, BufRead};
use yaktool::hybrid_interpret::{route_without_model, RouteDecision};
fn main() {
 for line in io::stdin().lock().lines() {
  let request: String = serde_json::from_str::<serde_json::Value>(&line.unwrap()).unwrap()["request"].as_str().unwrap().to_owned();
  let out = match route_without_model(&request).unwrap() { RouteDecision::HardDeny(i)=>serde_json::json!({"route":"hard_deny","intent":i}), RouteDecision::Deterministic{stage,intent}=>serde_json::json!({"route":stage,"intent":intent}), RouteDecision::NeedsModel=>serde_json::json!({"route":"needs_model"}) };
  println!("{}", out);
 }
}
