use serde::Deserialize;
use std::{fs::File, io::{BufRead, BufReader}};
use yaktool::{hybrid_interpret, model_intent::{ModelAction, ModelIntent, ModelLocation, Category, AgeRelation, SizeRelation, SizeUnit}};

#[derive(Deserialize)] struct Row { request: String, expected: ModelIntent }
fn locs() -> [ModelLocation;6] { [ModelLocation::Home,ModelLocation::Desktop,ModelLocation::Documents,ModelLocation::Downloads,ModelLocation::Pictures,ModelLocation::Archive] }
fn cats() -> [Category;5] { [Category::Any,Category::Pdf,Category::Png,Category::Jpeg,Category::Text] }
fn reject(req:&str, mi:ModelIntent) -> bool { match mi.normalize() { Ok(i)=>hybrid_interpret::gate_model_move(req, &i).is_err(), Err(_)=>true } }

#[test]
fn exhaustive_gold_move_corruptions_are_rejected() {
    let f=File::open("evaluation/development-v2-fallback-v2/model-candidates.jsonl").unwrap();
    let rows:Vec<Row>=BufReader::new(f).lines().map(|l|serde_json::from_str(&l.unwrap()).unwrap()).filter(|r: &Row| r.expected.action==ModelAction::Move).collect();
    assert_eq!(rows.len(),95);
    let mut generated=0usize; let mut accepted=0usize; let mut by=[0usize;14];
    for r in rows {
        let g=r.expected; let src=g.source.clone(); let dst=g.destination.clone();
        for x in locs() { if x!=src { let mut m=g.clone(); m.source=x; generated+=1; by[0]+=1; if !reject(&r.request,m){accepted+=1;} } }
        for x in locs() { if x!=dst { let mut m=g.clone(); m.destination=x; generated+=1; by[1]+=1; if !reject(&r.request,m){accepted+=1;} } }
        { let mut m=g.clone(); m.source=dst.clone(); m.destination=src.clone(); generated+=1; by[2]+=1; if !reject(&r.request,m){accepted+=1;} }
        for c in cats() { if c!=g.category { let mut m=g.clone(); m.category=c.clone(); generated+=1; by[3 + if g.category==Category::Any {0}else if c==Category::Any {1}else{2}]+=1; if !reject(&r.request,m){accepted+=1;} } }
        let mut ages=vec![AgeRelation::None,AgeRelation::OlderThan,AgeRelation::NewerThan,AgeRelation::Today,AgeRelation::ThisWeek]; ages.dedup();
        for ar in ages { let mut m=g.clone(); m.age_relation=ar.clone(); m.age_days=if matches!(ar,AgeRelation::OlderThan|AgeRelation::NewerThan){if g.age_days==30{29}else{30}}else{0}; let idx=if g.age_relation==AgeRelation::None{6}else if ar==AgeRelation::None{7}else if matches!(ar,AgeRelation::OlderThan|AgeRelation::NewerThan)&&ar!=g.age_relation{8}else if matches!(ar,AgeRelation::OlderThan|AgeRelation::NewerThan)&&m.age_days!=g.age_days{9}else{10}; if ar!=g.age_relation || m.age_days!=g.age_days { generated+=1; by[idx]+=1; if !reject(&r.request,m){accepted+=1;} } }
        let units=[SizeUnit::KB,SizeUnit::MB,SizeUnit::GB,SizeUnit::KiB,SizeUnit::MiB,SizeUnit::GiB];
        if g.size_relation==SizeRelation::None { for u in units { let mut m=g.clone(); m.size_relation=SizeRelation::LargerThan; m.size_value=1; m.size_unit=u; generated+=1; by[11]+=1; if !reject(&r.request,m){accepted+=1;} } } else { let mut m=g.clone(); m.size_relation=SizeRelation::None; m.size_value=0; m.size_unit=SizeUnit::None; generated+=1; by[12]+=1; if !reject(&r.request,m){accepted+=1;} for u in units { let mut m=g.clone(); m.size_unit=u; m.size_value=g.size_value+1; generated+=1; by[13]+=1; if !reject(&r.request,m){accepted+=1;} } }
        for k in 0..5 { let mut m=g.clone(); m.source=locs()[(k+1)%6].clone(); m.destination=locs()[(k+2)%6].clone(); if m.source==m.destination {m.destination=locs()[(k+3)%6].clone();} m.category=Category::Pdf; m.age_relation=AgeRelation::OlderThan; m.age_days=1; m.size_relation=SizeRelation::LargerThan; m.size_value=1; m.size_unit=SizeUnit::MB; generated+=1; if !reject(&r.request,m){accepted+=1;} }
    }
    assert!(generated>2000); assert_eq!(accepted,0);
}

#[test]
fn evidence_side_adversaries_fail_closed() {
    let base=ModelIntent { schema_version:"yaktool.model_intent.v1".into(), action:ModelAction::Move, source:ModelLocation::Downloads, destination:ModelLocation::Archive, category:Category::Pdf, age_relation:AgeRelation::None, age_days:0, size_relation:SizeRelation::None, size_value:0, size_unit:SizeUnit::None };
    for req in ["move PDFs from downloads and documents to archive","move PDFs from downloads to archive or desktop","move PDFs older than 30 days and newer than 10 days from downloads to archive","move PDFs larger than 10 MB and larger than 20 MB from downloads to archive","don't move PDFs from downloads to archive","move PDFs from downloads to archive and delete originals","move PDFs from /tmp to archive"] { let ok=reject(req,base.clone()); eprintln!("side {req}: rejected={ok}"); assert!(ok,"accepted: {req}"); }
}
