//! Bounded IPC with the existing guide and qualified sprite worker.
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;
use serde_json::{Value, json};

const MAX_LINE: usize = 16_000_000;
pub struct Bridge { child:Child, input:ChildStdin, output:Receiver<Result<Value,String>>, token:u64 }
impl Bridge {
    pub fn new(python:&str,port:u16,guide:&str)->Result<Self,String> {
        #[cfg(windows)] use std::os::windows::process::CommandExt;
        let mut command=Command::new(python);
        command.args(["-m","hoi4companion.overlay_bridge","--port",&port.to_string(),"--guide",guide]);
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
        #[cfg(windows)] command.creation_flags(0x08000000);
        let mut child=command.spawn().map_err(|e|e.to_string())?;
        let input=child.stdin.take().ok_or("Missing bridge stdin")?;
        let stdout=child.stdout.take().ok_or("Missing bridge stdout")?;
        let (send,output)=mpsc::sync_channel(1);
        thread::Builder::new().name("guide-responses".into()).spawn(move || {
            let mut reader=BufReader::new(stdout);
            loop {
                let result=read_response(&mut reader);
                let ended=result.is_err();
                if send.send(result).is_err()||ended {break;}
            }
        }).map_err(|e|e.to_string())?;
        Ok(Self {child,input,output,token:0})
    }
    pub fn request(&mut self,mut request:Value,raw:&[u8])->Result<Value,String> {
        self.token=self.token.wrapping_add(1);
        request["schema_version"]=1.into();request["token"]=self.token.into();
        let header=serde_json::to_vec(&request).map_err(|e|e.to_string())?;
        if header.len()>1_048_576 || raw.len()>80_000_000 {return Err("Overlay request exceeds bounds".into());}
        self.input.write_all(&header).and_then(|_|self.input.write_all(b"\n"))
            .and_then(|_|self.input.write_all(raw)).and_then(|_|self.input.flush()).map_err(|e|e.to_string())?;
        let response=self.output.recv_timeout(Duration::from_secs(10)).map_err(|e|e.to_string())??;
        if response["schema_version"]!=1 || response["token"]!=self.token
            || response["kind"]!=request["kind"] {return Err("Outdated or incompatible overlay response".into());}
        if response["ok"]!=true {return Err(response["error"].as_str().unwrap_or("Guide worker failed").into());}
        Ok(response)
    }
    pub fn snapshot(&mut self,refresh:bool)->Result<Value,String> {
        self.request(json!({"kind":"snapshot","refresh_guides":refresh}),&[])
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {let _=self.child.kill();let _=self.child.wait();}
}
fn read_response(reader:&mut impl BufRead)->Result<Value,String> {
    let mut data=Vec::new();
    loop {
        let buffer=reader.fill_buf().map_err(|e|e.to_string())?;
        if buffer.is_empty() {return Err("Guide worker closed its connection".into());}
        let width=buffer.iter().position(|b|*b==b'\n').map_or(buffer.len(),|n|n+1);
        if data.len()+width>MAX_LINE {return Err("Guide worker response exceeds bounds".into());}
        let ended=buffer[width-1]==b'\n';data.extend_from_slice(&buffer[..width]);reader.consume(width);
        if ended {return serde_json::from_slice(&data).map_err(|e|e.to_string());}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn framing_rejects_truncation_and_preserves_next_line() {
        let mut reader=std::io::Cursor::new(b"{\"a\":1}\n{\"b\":2}\n");
        assert_eq!(read_response(&mut reader).unwrap()["a"],1);
        assert_eq!(read_response(&mut reader).unwrap()["b"],2);
        assert!(read_response(&mut reader).is_err());
        assert!(read_response(&mut std::io::Cursor::new(b"{\"a\":1}")).is_err());
    }
}
