use crate::{error::{Error, Result}, intent::{Action, Intent, Location}, interpret::{Interpreter, RuleInterpreter, parse_move_frame}, model_client::SemanticModel};
use regex::Regex;

fn empty(action: Action) -> Intent { Intent::new(action) }
fn hard_deny(s: &str) -> bool { let l=s.to_ascii_lowercase(); Regex::new(r"\b(delete|erase|wipe|copy|duplicate|overwrite|sudo|chmod|chown|install|uninstall|shell|command|touch)\b|\b(?:run|execute)\s+(?:the\s+)?(?:shell\s+)?(?:command|[a-z][a-z0-9_-]*(?:\s+-[-a-z0-9]+)?)|\b(?:don't|do not|never)\s+move\b|\b(?:except|excluding|but not)\b").unwrap().is_match(&l) }
fn read_only(s: &str) -> Option<Intent> {
 let l=s.to_ascii_lowercase(); let re=Regex::new(r"\b(?:in|from|for)\s+(home|desktop|documents|downloads|pictures|archive)\b|\b(?:list|show|find|search)\s+(home|desktop|documents|downloads|pictures|archive)\b").ok()?; let c=re.captures(&l)?; let source=c.get(1).or_else(||c.get(2))?.as_str(); let action=if Regex::new(r"\b(find|search)\b").ok()?.is_match(&l){Action::Search}else if Regex::new(r"\b(list|show)\b").ok()?.is_match(&l){Action::List}else{return None}; let mut i=empty(action); i.source=Some(Location::DirectoryAlias(source.into())); Some(i)
}
pub fn interpret<M: SemanticModel>(request: &str, model: &M) -> Result<Intent> {
 if hard_deny(request) { return Ok(empty(Action::Unsupported)); }
 if let Ok(i)=RuleInterpreter.interpret(request) { if !matches!(i.action,Action::Clarify|Action::Unsupported) { return Ok(i); } }
 if let Some(i)=read_only(request) { return Ok(i); }
 if let Some(i)=parse_move_frame(request)? { return Ok(i); }
 let mi=model.interpret(request)?; let normalized=mi.normalize()?;
 if mi.action==crate::model_intent::ModelAction::Move {
   let Some(frame)=parse_move_frame(request)? else { return Err(Error::new("AMBIGUOUS_REQUEST","Model move lacks deterministic evidence")); };
   if normalized.action!=frame.action || normalized.source.as_ref().map(|x|format!("{:?}",x)) != frame.source.as_ref().map(|x|format!("{:?}",x)) || normalized.destination.as_ref().map(|x|format!("{:?}",x)) != frame.destination.as_ref().map(|x|format!("{:?}",x)) || normalized.filters.extensions!=frame.filters.extensions || normalized.filters.min_size_bytes!=frame.filters.min_size_bytes { return Err(Error::new("AMBIGUOUS_REQUEST","Model move failed evidence gate")); }
 }
 Ok(normalized)
}

pub enum RouteDecision { HardDeny(Intent), Deterministic { stage: &'static str, intent: Intent }, NeedsModel }
pub fn route_without_model(request: &str) -> Result<RouteDecision> {
 if hard_deny(request) { return Ok(RouteDecision::HardDeny(empty(Action::Unsupported))); }
 if let Ok(i)=RuleInterpreter.interpret(request) { if !matches!(i.action,Action::Clarify|Action::Unsupported) { return Ok(RouteDecision::Deterministic{stage:"rule_interpreter",intent:i}); } }
 if let Some(i)=read_only(request) { return Ok(RouteDecision::Deterministic{stage:"read_only",intent:i}); }
 if let Some(i)=parse_move_frame(request)? { return Ok(RouteDecision::Deterministic{stage:"move_frame",intent:i}); }
 Ok(RouteDecision::NeedsModel)
}
