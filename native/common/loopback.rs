//! Bounded loopback HTTP shared by the native desktop and HUD.
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    time::Duration,
};
type Result<T> = std::result::Result<T, String>;
pub struct Api {
    pub port: u16,
}
fn header(reader: &mut impl BufRead) -> Result<String> {
    let mut bytes = vec![];
    let n = reader
        .take(16385)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if n == 0 || n > 16384 || !bytes.ends_with(b"\n") {
        return Err("Invalid native HTTP header".into());
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
impl Api {
    pub fn request(
        &self,
        path: &str,
        body: Option<&Value>,
        limit: usize,
    ) -> Result<(u16, Vec<u8>)> {
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.bytes().any(|b| b.is_ascii_whitespace() || b < 32)
            || path.len() > 8192
        {
            return Err("Invalid local API path".into());
        }
        let payload = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|e| e.to_string())?;
        if payload.as_ref().is_some_and(|b| b.len() > 1024 * 1024) {
            return Err("Native HUD header exceeds limit".into());
        }
        let mut socket =
            TcpStream::connect_timeout(&([127, 0, 0, 1], self.port).into(), Duration::from_secs(2))
                .map_err(|e| e.to_string())?;
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        socket
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        let method = if body.is_some() { "POST" } else { "GET" };
        let len = payload.as_ref().map_or(0, Vec::len);
        write!(socket,"{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n",self.port).map_err(|e|e.to_string())?;
        if let Some(bytes) = payload {
            socket.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        socket.flush().map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(socket);
        let status = header(&mut reader)?;
        let status = status
            .split_whitespace()
            .nth(1)
            .ok_or("Missing native HTTP status")?
            .parse::<u16>()
            .map_err(|_| "Invalid native HTTP status")?;
        let mut length = None;
        let mut total = 0;
        loop {
            let line = header(&mut reader)?;
            total += line.len();
            if total > 32768 {
                return Err("Native HTTP headers exceed limit".into());
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            let (key, value) = line.split_once(':').ok_or("Invalid native HTTP header")?;
            if key.eq_ignore_ascii_case("content-length") {
                if length.is_some() {
                    return Err("Duplicate native HTTP length".into());
                }
                length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| "Invalid native HTTP length")?,
                );
            }
            if key.eq_ignore_ascii_case("transfer-encoding")
                || key.eq_ignore_ascii_case("content-encoding") && value.trim() != "identity"
            {
                return Err("Unsupported native HTTP encoding".into());
            }
        }
        let length = length.ok_or("Missing native HTTP length")?;
        if length > limit {
            return Err("Native HTTP response exceeds limit".into());
        }
        let mut bytes = vec![0; length];
        reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        Ok((status, bytes))
    }
    pub fn json(&self, path: &str, body: Option<&Value>) -> Result<Value> {
        let (status, data) = self.request(path, body, 16_000_000)?;
        let value: Value = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
        if status != 200 || value.get("error").is_some() && value.get("schema_version").is_none() {
            return Err(value["error"]
                .as_str()
                .unwrap_or("Native HUD request failed")
                .into());
        }
        Ok(value)
    }
    pub fn overlay(&self, input: &Value) -> Result<Value> {
        self.json("/api/overlay", Some(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncated_and_overlong_headers() {
        assert!(header(&mut std::io::Cursor::new(b"truncated")).is_err());
        assert!(header(&mut std::io::Cursor::new(vec![b'x'; 16385])).is_err());
        assert_eq!(
            header(&mut std::io::Cursor::new(b"OK\r\n")).unwrap(),
            "OK\r\n"
        );
    }
}
