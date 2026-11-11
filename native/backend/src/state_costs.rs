//! Strict projection of admitted, calculated native purchase costs.
use super::*;
pub(super) fn valid(value:&Value)->bool {
    value.as_object().is_some_and(|rows|rows.len()<=4096 && rows.iter().all(|(key,row)| {
        key.split_once('/').is_some_and(|(group,id)|[group,id].iter().all(|s|!s.is_empty()&&s.len()<=160&&s.bytes().all(|c|c.is_ascii_alphanumeric()||b"_.-".contains(&c))))
            && row.is_object() && row["currency"]=="political_power"
            && ["base","effective","factor"].iter().all(|key|row[*key].as_f64().is_some_and(|n|n.is_finite()&&(0. ..=1_000_000.).contains(&n)))
    }))
}
pub(super) fn project(overview:&Value)->Value {
    let costs=&overview["politics"]["costs"];
    let Some(rows)=costs["rows"].as_array().filter(|a|a.len()<=4096) else {return unknown("Native political costs unavailable")};
    if costs["schema_version"]!=1||costs["scope"]!="qualified_native_purchase_targets"||!["observed","partial"].contains(&text(&costs["status"]))||costs["source"]!="native_political_purchase_formula" {return unknown("Native political costs unavailable");}
    let mut values=Map::new();let mut seen=BTreeSet::new();
    for row in rows {
        if !id(&row["id"])||!id(&row["group"]) {return unknown("Invalid political cost identity");}
        let key=format!("{}/{}",text(&row["group"]),text(&row["id"]));
        if !seen.insert(key.clone()) {return unknown("Duplicate political cost identity");}
        if row["status"]=="unknown" {
            if row["reason"].as_str().is_none_or(|s|s.is_empty()||s.len()>512||s.chars().any(char::is_control)) {return unknown("Invalid political cost reason");}
            continue;
        }
        if row["status"]!="calculated" {return unknown("Invalid political cost status");}
        values.insert(key,json!({"base":row["base"],"effective":row["effective"],"factor":row["factor"],"currency":row["currency"]}));
    }
    let value=json!(values);
    if !valid(&value) {return unknown("Invalid native political costs");}
    known(value,"calculated","native_political_purchase_formula")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn native_cost_projection_rejects_unknown_identity_and_unqualified_scope() {
        let row=json!({"id":"free_trade","group":"trade_laws","currency":"political_power","base":150,"effective":120,"factor":0.8,"status":"calculated"});
        let mut v=json!({"politics":{"costs":{"schema_version":1,"scope":"qualified_native_purchase_targets","status":"observed","source":"native_political_purchase_formula","rows":[row]}}});
        assert_eq!(project(&v)["value"]["trade_laws/free_trade"]["effective"],120);
        v["politics"]["costs"]["rows"].as_array_mut().unwrap().push(json!({"id":"../bad","group":"trade_laws","status":"unknown"}));
        assert_eq!(project(&v)["status"],"unknown");
        v["politics"]["costs"]["rows"]=json!([row]);v["politics"]["costs"]["scope"]=json!("unqualified");assert_eq!(project(&v)["status"],"unknown");
    }
}
