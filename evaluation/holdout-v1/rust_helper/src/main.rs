use serde::{Deserialize, Serialize};
use std::io::{self, BufRead};
use yaktool::{interpret::{Interpreter, RuleInterpreter}, model_intent::ModelIntent};
#[derive(Deserialize)] struct Input { request: String, output: Option<serde_json::Value> }
#[derive(Serialize)] struct Output { rule: String, rule_intent: Option<serde_json::Value>, model_valid: Option<bool>, model_error: Option<String>, normalized: Option<serde_json::Value> }
fn main() {
    let rules=RuleInterpreter;
    for line in io::stdin().lock().lines() {
        let x:Input=serde_json::from_str(&line.unwrap()).unwrap();
        let (rule,ri)=match rules.interpret(&x.request) { Ok(i)=>(format!("{:?}",i.action),serde_json::to_value(i).ok()), Err(e)=>(format!("error:{}",e.code),None) };
        let (valid,error,norm)=match x.output { None=>(None,None,None), Some(v)=>match serde_json::from_value::<ModelIntent>(v) { Ok(m)=>match m.normalize() { Ok(i)=>(Some(true),None,serde_json::to_value(i).ok()), Err(e)=>(Some(false),Some(format!("{}:{}",e.code,e.message)),None) }, Err(e)=>(Some(false),Some(format!("DESERIALIZE:{}",e)),None) } };
        println!("{}",serde_json::to_string(&Output{rule,rule_intent:ri,model_valid:valid,model_error:error,normalized:norm}).unwrap());
    }
}
