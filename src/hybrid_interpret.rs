use crate::{error::{Error, Result}, intent::{Action, Age, Intent, Location}, interpret::{Interpreter, RuleInterpreter, parse_count_query, parse_move_frame}, model_client::SemanticModel};
use regex::Regex;

fn empty(action: Action) -> Intent { Intent::new(action) }
fn hard_deny(s: &str) -> bool { let l=s.to_ascii_lowercase(); Regex::new(r"\b(delete|erase|wipe|copy|duplicate|overwrite|sudo|chmod|chown|install|uninstall|shell|command|touch)\b|\b(?:run|execute)\s+(?:the\s+)?(?:shell\s+)?(?:command|[a-z][a-z0-9_-]*(?:\s+-[-a-z0-9]+)?)|\b(?:don't|do not|never)\s+move\b|\b(?:except|excluding|but not)\b").unwrap().is_match(&l) }
fn read_only(s: &str) -> Option<Intent> {
 let l=s.to_ascii_lowercase(); let re=Regex::new(r"\b(?:in|from|for)\s+(home|desktop|documents|downloads|pictures|archive)\b|\b(?:list|show|find|search)\s+(home|desktop|documents|downloads|pictures|archive)\b").ok()?; let c=re.captures(&l)?; let source=c.get(1).or_else(||c.get(2))?.as_str(); let action=if Regex::new(r"\b(find|search)\b").ok()?.is_match(&l){Action::Search}else if Regex::new(r"\b(list|show)\b").ok()?.is_match(&l){Action::List}else{return None}; let mut i=empty(action); i.source=Some(Location::DirectoryAlias(source.into())); Some(i)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Evidence<T> { Absent, Exact(T), Conflict }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveEvidence {
    pub action_explicit: bool,
    pub source: Evidence<String>,
    pub destination: Evidence<String>,
    pub category: Evidence<String>,
    pub age: Evidence<Age>,
    pub size: Evidence<u64>,
}

fn unique<T: Clone + Eq>(xs: Vec<T>) -> Evidence<T> {
    if xs.is_empty() { Evidence::Absent }
    else if xs.windows(2).all(|w| w[0] == w[1]) { Evidence::Exact(xs[0].clone()) }
    else if xs.iter().all(|x| *x == xs[0]) { Evidence::Exact(xs[0].clone()) }
    else { Evidence::Conflict }
}

/// Extracts independently verifiable fragments from a request. This is
/// intentionally less strict than `parse_move_frame`: it never constructs an
/// executable intent and may succeed for a sentence requiring model parsing.
pub fn extract_move_evidence(request: &str) -> Result<MoveEvidence> {
    let s = request.trim().trim_end_matches('.').to_ascii_lowercase();
    let unsupported = Regex::new(r"\b(copy|duplicate|sync|backup|delete|erase|remove|rename|compress|upload|download|install|execute|run|chmod|sudo|overwrite|touch)\b|\b(?:don't|do not|never)\s+move\b|\b(?:except|excluding|but not)\b|\b(?:and|then)\s+(?:delete|copy|rename|overwrite|chmod|run|execute)\b").unwrap();
    if unsupported.is_match(&s) { return Ok(MoveEvidence { action_explicit:false, source:Evidence::Conflict, destination:Evidence::Conflict, category:Evidence::Conflict, age:Evidence::Conflict, size:Evidence::Conflict }); }
    let loc_words = r"(home|desktop|documents|downloads|pictures|archive)";
    if Regex::new(&format!(r"\bfrom\s+{loc_words}\s+and\s+{loc_words}\b")).unwrap().is_match(&s)
        || Regex::new(&format!(r"\b(?:to|into|over\s+to)\s+{loc_words}\s+or\s+{loc_words}\b")).unwrap().is_match(&s) {
        return Ok(MoveEvidence { action_explicit:true, source:Evidence::Conflict, destination:Evidence::Conflict, category:Evidence::Conflict, age:Evidence::Conflict, size:Evidence::Conflict });
    }
    let action_explicit = Regex::new(r"\b(?:move|moved|moving|relocate|relocated|put|send|sent|transfer|take|place|placing|arrange|have|belong)\b").unwrap().is_match(&s);
    let loc = r"(home|desktop|documents|downloads|pictures|archive)";
    let from = Regex::new(&format!(r"\b(?:from|out\s+of)\s+{loc}\b")).unwrap();
    let mut sources: Vec<String> = from.captures_iter(&s).map(|c| c[1].to_string()).collect();
    if sources.is_empty() {
        let in_loc = Regex::new(&format!(r"\b(?:in|under|at)\s+{loc}\b")).unwrap();
        let all: Vec<String> = in_loc.captures_iter(&s).map(|c| c[1].to_string()).collect();
        sources = if all.len() > 1 { vec![all[0].clone()] } else { all };
    }
    let dest_re = Regex::new(&format!(r"\b(?:over\s+to|to|into|within)\s+{loc}\b")).unwrap();
    let mut destinations: Vec<String> = dest_re.captures_iter(&s).map(|c| c[1].to_string()).collect();
    if destinations.is_empty() {
        let named = Regex::new(&format!(r"\bdestination\s+for\b.*?\b(?:is|=)\s+{loc}\b")).unwrap();
        destinations = named.captures_iter(&s).filter_map(|c| c.get(1).map(|m|m.as_str().to_string())).collect();
    }
    if destinations.is_empty() {
        let belong = Regex::new(&format!(r"\bbelong(?:s)?\s+in\s+{loc}\b")).unwrap();
        destinations = belong.captures_iter(&s).filter_map(|c| c.get(1).map(|m|m.as_str().to_string())).collect();
    }
    if destinations.is_empty() && !sources.is_empty() {
        let in_locs: Vec<String> = Regex::new(&format!(r"\bin\s+{loc}\b")).unwrap().captures_iter(&s).map(|c| c[1].to_string()).collect();
        destinations = in_locs.into_iter().filter(|x| !sources.contains(x)).collect();
    }
    let mut categories = Vec::new();
    for (name, pat) in [("pdf",r"\bpdfs?(?:\s+files?)?\b"),("png",r"\bpngs?(?:\s+files?)?\b"),("jpeg",r"\b(?:jpegs?|jpgs?)(?:\s+files?)?\b"),("text",r"\btext(?:\s+files?)?\b")] {
        if Regex::new(pat).unwrap().is_match(&s) { categories.push(name.to_string()); }
    }
    let age_re = Regex::new(r"\b(older|newer)\s+than\s+([0-9]+)\s+days?\b").unwrap();
    let mut ages = Vec::new();
    for c in age_re.captures_iter(&s) { let n=c[2].parse::<u32>().map_err(|_| Error::new("MODEL_INTENT_INVALID","invalid age"))?; if n==0 { return Ok(MoveEvidence{action_explicit:false,source:Evidence::Conflict,destination:Evidence::Conflict,category:Evidence::Conflict,age:Evidence::Conflict,size:Evidence::Conflict}); } ages.push(if &c[1]=="older" {Age::OlderThan(n)} else {Age::NewerThan(n)}); }
    if s.contains("modified today") { ages.push(Age::Today); }
    if s.contains("modified this week") { ages.push(Age::ThisWeek); }
    let size_re=Regex::new(r"\blarger\s+than\s+([0-9]+)\s+(kb|mb|gb|kib|mib|gib)\b").unwrap();
    let mut sizes=Vec::new(); for c in size_re.captures_iter(&s) { sizes.push(crate::interpret::parse_size(&c[1],&c[2])?); }
    Ok(MoveEvidence { action_explicit, source:unique(sources), destination:unique(destinations), category:unique(categories), age:unique(ages), size:unique(sizes) })
}

pub fn gate_model_move(request: &str, proposed: &Intent) -> Result<()> {
    if proposed.action != Action::Move { return Ok(()); }
    let e = extract_move_evidence(request)?;
    if !e.action_explicit { return Err(Error::new("AMBIGUOUS_REQUEST","missing explicit move evidence")); }
    let loc_name = |l: &Option<Location>| match l { Some(Location::DirectoryAlias(x))=>Some(x.to_ascii_lowercase()), _=>None };
    let check_loc = |ev: &Evidence<String>, actual: &Option<Location>, label: &str| -> Result<()> { match ev { Evidence::Exact(x) if loc_name(actual).as_deref()==Some(x)=>Ok(()), Evidence::Absent if actual.is_none()=>Ok(()), Evidence::Conflict=>Err(Error::new("AMBIGUOUS_REQUEST",format!("{label} evidence conflict"))), _=>Err(Error::new("AMBIGUOUS_REQUEST",format!("{label} evidence mismatch"))) } };
    check_loc(&e.source,&proposed.source,"source")?; check_loc(&e.destination,&proposed.destination,"destination")?;
    let actual_cat=if proposed.filters.extensions.is_empty(){None}else if proposed.filters.extensions==[".pdf"]{Some("pdf".to_string())}else if proposed.filters.extensions==[".png"]{Some("png".to_string())}else if proposed.filters.extensions==[".txt"]{Some("text".to_string())}else if proposed.filters.extensions==[".jpg",".jpeg"]{Some("jpeg".to_string())}else{return Err(Error::new("AMBIGUOUS_REQUEST","category mismatch"));};
    match &e.category { Evidence::Absent => { if actual_cat.is_some() { return Err(Error::new("AMBIGUOUS_REQUEST","category invented")); } }, Evidence::Exact(x) => { if actual_cat.as_deref()!=Some(x) { return Err(Error::new("AMBIGUOUS_REQUEST","category mismatch")); } }, Evidence::Conflict => return Err(Error::new("AMBIGUOUS_REQUEST","category conflict")) }
    match (&e.age,&proposed.filters.age) { (Evidence::Absent,None)=>(), (Evidence::Exact(a),Some(b)) if a==b=>(), (Evidence::Conflict,_)=>return Err(Error::new("AMBIGUOUS_REQUEST","age conflict")), _=>return Err(Error::new("AMBIGUOUS_REQUEST","age invented, dropped, or mismatched")) }
    match (&e.size,&proposed.filters.min_size_bytes) { (Evidence::Absent,None)=>(), (Evidence::Exact(v),Some(b)) if v==b=>(), (Evidence::Conflict,_)=>return Err(Error::new("AMBIGUOUS_REQUEST","size conflict")), _=>return Err(Error::new("AMBIGUOUS_REQUEST","size invented, dropped, or mismatched")) }
    Ok(())
}
pub fn interpret<M: SemanticModel>(request: &str, model: &M) -> Result<Intent> {
    interpret_with_fallback_notice(request, model, || {})
}
pub fn interpret_with_fallback_notice<M: SemanticModel, F: FnOnce()>(request: &str, model: &M, notice: F) -> Result<Intent> {
 let route = route_without_model(request)?;
 let mi = match route {
  RouteDecision::HardDeny(i) | RouteDecision::Deterministic { intent: i, .. } => return Ok(i),
  RouteDecision::NeedsModel => { notice(); model.interpret(request)? }
 };
 let normalized=mi.normalize()?;
 if mi.action==crate::model_intent::ModelAction::Move {
   gate_model_move(request, &normalized)?;
 }
 Ok(normalized)
}

pub enum RouteDecision { HardDeny(Intent), Deterministic { stage: &'static str, intent: Intent }, NeedsModel }
pub fn route_without_model(request: &str) -> Result<RouteDecision> {
 if hard_deny(request) { return Ok(RouteDecision::HardDeny(empty(Action::Unsupported))); }
 if let Some(i)=parse_count_query(request)? { return Ok(RouteDecision::Deterministic{stage:"count",intent:i}); }
 if let Ok(i)=RuleInterpreter.interpret(request) { if !matches!(i.action,Action::Clarify|Action::Unsupported) { return Ok(RouteDecision::Deterministic{stage:"rule_interpreter",intent:i}); } }
 if let Some(i)=read_only(request) { return Ok(RouteDecision::Deterministic{stage:"read_only",intent:i}); }
 if let Some(i)=parse_move_frame(request)? { return Ok(RouteDecision::Deterministic{stage:"move_frame",intent:i}); }
 Ok(RouteDecision::NeedsModel)
}
