//! Qualified archetype logistics facts. Raw stockpile rows are separate.
use super::*;
pub(super) const SOURCE:&str="native_logistics_ui_formula";
pub(super) fn key(key:&str)->bool {key=="equipment.logistics"||key.starts_with("country.equipment.logistics.")}
pub(super) fn valid_fact(key:&str,v:&Value)->bool {
 if key=="equipment.logistics" {return valid(v)}
 let Some((token,field))=key.strip_prefix("country.equipment.logistics.items.").and_then(|s|s.rsplit_once('.')) else {return false};
 !token.is_empty()&&token.len()<=160&&token.bytes().all(|b|b.is_ascii_alphanumeric()||b"_.-".contains(&b))&&["gross_stockpile","logistics_stockpile","needed","available","balance_per_day"].contains(&field)&&number(v)&&v.as_f64().is_some_and(|n|n.abs()<=922_337_202.&& (!["gross_stockpile","logistics_stockpile"].contains(&field)||n>=0.))
}
pub(super) fn valid(v:&Value)->bool {
 if v["schema_version"]!=1||!["observed","partial"].contains(&text(&v["status"]))||v["source"]!=SOURCE||v["aggregation"]!="equipment_archetype"||!integer_range(&v["interval_days"],1,30){return false}
 let Some(rows)=v["items"].as_array().filter(|a|a.len()<=4096) else{return false};let mut seen=BTreeSet::new();
 rows.iter().all(|r|{
  if !id(&r["id"])||!seen.insert(text(&r["id"]))||r["aggregation"]!="equipment_archetype"||!["observed","partial"].contains(&text(&r["status"]))||!r.get("reason").is_some_and(|v|v.is_null()||v.is_string()){return false}
  if !["gross_stockpile","logistics_stockpile","needed","available","balance_per_day"].iter().all(|f|r.get(*f).is_some_and(|x|(*f!="gross_stockpile")&&x.is_null()||valid_fact(&format!("country.equipment.logistics.items.{}.{}",text(&r["id"]),f),x))){return false}
  if (r["needed"].is_null()||r["logistics_stockpile"].is_null())!=r["available"].is_null(){return false}
  if let(Some(g),Some(n),Some(a))=(r["logistics_stockpile"].as_f64(),r["needed"].as_f64(),r["available"].as_f64()) {if (a-(g-n).clamp(-922_337_202.,922_337_202.)).abs()>0.000011{return false}}
  r["status"]!="observed"||["logistics_stockpile","needed","available","balance_per_day"].iter().all(|f|!r[*f].is_null())
 })
}
pub(super) fn project(overview:&Value)->Map<String,Value>{
 let mut facts=Map::new();let v=&overview["equipment"]["logistics"];
 let qualified=["observed","partial"].contains(&text(&overview["equipment"]["status"]))&&valid(v);
 facts.insert("equipment.logistics".into(),if qualified{known(v.clone(),"calculated",SOURCE)}else{unknown("National logistics formula not qualified")});
 if qualified {for row in list(&v["items"]){for field in ["gross_stockpile","logistics_stockpile","needed","available","balance_per_day"]{let key=format!("country.equipment.logistics.items.{}.{}",text(&row["id"]),field);facts.insert(key,if row[field].is_null(){unknown("Native logistics component unavailable")}else{known(row[field].clone(),"calculated",SOURCE)});}}}facts
}



