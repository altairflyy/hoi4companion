//! Rust-only HUD transport. The backend is the authoritative active guide.
use serde_json::{Value, json};

pub struct Bridge { client:crate::native_protocol::Client, token:u64 }
fn validate(response:Value,input:&Value,token:u64)->Result<Value,String> {
    if response["schema_version"]!=1 || response["token"]!=token
        || response["kind"]!=input["kind"] || response["ok"]!=true {
        return Err("Outdated or incompatible native HUD response".into());
    }
    Ok(response)
}
impl Bridge {
    pub fn new(port:u16)->Result<Self,String> {
        let client=crate::native_protocol::Client::probe(port)?
            .ok_or("Native backend is not running; start HOI4 Companion first")?;
        Ok(Self{client,token:0})
    }
    pub fn request(&mut self,mut input:Value,raw:&[u8])->Result<Value,String> {
        self.token=self.token.wrapping_add(1);
        input["schema_version"]=json!(1);input["token"]=json!(self.token);
        validate(self.client.request(&input,raw)?,&input,self.token)
    }
    pub fn snapshot(&mut self,refresh:bool)->Result<Value,String> {
        self.request(json!({"kind":"snapshot","refresh_guides":refresh}),&[])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_response_rejects_wrong_revision_kind_or_failure() {
        let input=json!({"kind":"snapshot"});
        let response=json!({"schema_version":1,"token":7,"kind":"snapshot","ok":true});
        assert!(validate(response.clone(),&input,7).is_ok());
        for (field,value) in [("schema_version",json!(2)),("token",json!(8)),("kind",json!("frame")),("ok",json!(false))] {
            let mut invalid=response.clone();invalid[field]=value;
            assert!(validate(invalid,&input,7).is_err());
        }
    }
}
