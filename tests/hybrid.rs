use std::cell::Cell;
use yaktool::{hybrid_interpret, model_intent::{ModelAction, ModelIntent, ModelLocation, Category, AgeRelation, SizeRelation, SizeUnit}, model_client::SemanticModel, error::{Error, Result}};

struct Mock { calls: Cell<u32> }
impl SemanticModel for Mock { fn interpret(&self, _: &str) -> Result<ModelIntent> { self.calls.set(self.calls.get()+1); Ok(model_move()) } }
fn model_move() -> ModelIntent { ModelIntent { schema_version:"yaktool.model_intent.v1".into(), action:ModelAction::Move, source:ModelLocation::Downloads, destination:ModelLocation::Archive, category:Category::Pdf, age_relation:AgeRelation::None, age_days:0, size_relation:SizeRelation::None, size_value:0, size_unit:SizeUnit::None } }
#[test] fn deterministic_routes_never_call_model() { let m=Mock{calls:Cell::new(0)}; for r in ["show Downloads","find PDFs in Documents","move PDFs from downloads to archive"] { let _=hybrid_interpret::interpret(r,&m); } assert_eq!(m.calls.get(),0); }
#[test] fn model_move_requires_evidence() { let m=Mock{calls:Cell::new(0)}; assert!(hybrid_interpret::interpret("please organize my files",&m).is_err()); assert_eq!(m.calls.get(),1); }
#[test] fn fallback_move_uses_independent_evidence() { let m=Mock{calls:Cell::new(0)}; let i=hybrid_interpret::interpret("I'd like the PDF files in Downloads moved into Archive",&m).unwrap(); assert_eq!(i.action, yaktool::intent::Action::Move); assert_eq!(m.calls.get(),1); }
#[test] fn fallback_move_age_is_verified() { let good=ModelIntent { age_relation:AgeRelation::OlderThan, age_days:30, ..model_move() }; struct M(ModelIntent); impl SemanticModel for M { fn interpret(&self,_:&str)->Result<ModelIntent>{Ok(self.0.clone())} } let req="The PDF files in Downloads, older than 30 days, should be moved into Archive"; assert!(hybrid_interpret::interpret(req,&M(good)).is_ok()); let bad=ModelIntent { age_days:29, ..ModelIntent { age_relation:AgeRelation::OlderThan, age_days:30, ..model_move() } }; assert!(hybrid_interpret::interpret(req,&M(bad)).is_err()); }
struct Offline;
impl SemanticModel for Offline { fn interpret(&self, _: &str) -> Result<ModelIntent> { Err(Error::new("MODEL_UNAVAILABLE","offline")) } }
#[test] fn unavailable_model_fails_closed() { assert!(hybrid_interpret::interpret("please organize my files",&Offline).is_err()); }

#[test]
fn fallback_candidate_oracle_and_mutation_rejection() {
    use std::fs::File; use std::io::{BufRead,BufReader}; use serde::Deserialize;
    #[derive(Deserialize)] struct Row { request:String, expected:ModelIntent }
    struct Gold(ModelIntent); impl SemanticModel for Gold { fn interpret(&self,_:&str)->Result<ModelIntent>{Ok(self.0.clone())} }
    let f=File::open("evaluation/development-v2-fallback-v2/model-candidates.jsonl").unwrap(); let mut total=0; let mut accepted=0; let mut moves=0; let mut move_ok=0; let mut ro_ok=0; let mut cl_ok=0; let mut un_ok=0;
    for line in BufReader::new(f).lines() { let r:Row=serde_json::from_str(&line.unwrap()).unwrap(); assert!(matches!(hybrid_interpret::route_without_model(&r.request).unwrap(), hybrid_interpret::RouteDecision::NeedsModel)); total+=1; let action=r.expected.action.clone(); if action==ModelAction::Move { moves+=1; } if hybrid_interpret::interpret(&r.request,&Gold(r.expected)).is_ok(){accepted+=1; match action { ModelAction::Move=>move_ok+=1, ModelAction::Search|ModelAction::List|ModelAction::FindLarge=>ro_ok+=1, ModelAction::Clarify=>cl_ok+=1, ModelAction::Unsupported=>un_ok+=1 } } }
    assert_eq!(total,176); assert_eq!(moves,95); assert_eq!(accepted,176); assert_eq!(move_ok,95); assert_eq!(ro_ok,33); assert_eq!(cl_ok,30); assert_eq!(un_ok,18);
}
