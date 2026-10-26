//! UI text only: never translate entity identities or authored guide captions.
use std::sync::{OnceLock,atomic::{AtomicBool,Ordering}};
use serde_json::Value;
static ITALIAN:AtomicBool=AtomicBool::new(false);
static CATALOG:OnceLock<Value>=OnceLock::new();
pub fn set_language(value:&str)->Result<(),String>{
    if !matches!(value,"english"|"italian"){return Err("Unsupported interface language".into());}
    ITALIAN.store(value=="italian",Ordering::Release);Ok(())
}
pub fn text(message:&str)->String{
    text_in(message,ITALIAN.load(Ordering::Acquire))
}
fn text_in(message:&str,italian:bool)->String{
    let catalog=CATALOG.get_or_init(||serde_json::from_str(include_str!("../../../src/hoi4companion/translations.json")).expect("packaged UI catalog"));
    let english=catalog["legacy"][message].as_str().unwrap_or(message);
    if italian{catalog["messages"][english].as_str().unwrap_or(english).to_owned()}else{english.to_owned()}
}
pub fn format(message:&str,values:&[&str])->String{
    substitute(text(message),values)
}
fn substitute(source:String,values:&[&str])->String{
    // Replace each token in a single pass; arguments are never translated or rescanned.
    let mut output=String::new();let mut rest=source.as_str();
    while let Some(start)=rest.find('{'){
        output.push_str(&rest[..start]);rest=&rest[start..];
        if let Some(end)=rest.find('}'){
            if let Ok(index)=rest[1..end].parse::<usize>(){if let Some(value)=values.get(index){output.push_str(value);rest=&rest[end+1..];continue;}}
        }
        output.push('{');rest=&rest[1..];
    }
    output.push_str(rest);output
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn english_default_and_optional_italian(){
        assert_eq!(text("Pause"),"Pause");
        assert_eq!(text_in("Pause",true),"Pausa");
        assert_eq!(substitute(text_in("GUI reading suspended: {0}",true),&["{1} GER_focus"]),"Lettura GUI sospesa: {1} GER_focus");
        assert_eq!(text("GER_oppose_hitler_ww"),"GER_oppose_hitler_ww");
        assert!(set_language("unsupported").is_err());
    }
}
