use jomini::Windows1252Encoding;
use std::borrow::Cow;

pub fn decode(data: &[u8], encoding: u32) -> Result<(Cow<'_, str>, &'static str), String> {
    if data.len() > crate::MAX_BYTES {
        return Err("Script oltre 80 MB".into());
    }
    if encoding > 2 {
        return Err("Codifica non supportata".into());
    }
    if data.starts_with(b"HOI4bin") || data.starts_with(b"PK\x03\x04") || data.contains(&0) {
        return Err("Contenuto binario non accettato come script testuale".into());
    }
    let bom = data.starts_with(b"\xef\xbb\xbf");
    if encoding != 2 || bom {
        let source = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
        match std::str::from_utf8(source) {
            Ok(text) => return Ok((Cow::Borrowed(text), "utf-8")),
            Err(error) if encoding == 1 || bom => return Err(format!("UTF-8 non valido: {error}")),
            Err(_) => {}
        }
    }
    if data
        .iter()
        .any(|byte| matches!(byte, 0x81 | 0x8d | 0x8f | 0x90 | 0x9d))
    {
        return Err("Byte non definito in Windows-1252: indicare una codifica valida".into());
    }
    if data
        .iter()
        .any(|b| *b < 32 && !matches!(*b, b'\t' | b'\r' | b'\n'))
    {
        return Err("Dati non testuali: conversione Windows-1252 rifiutata".into());
    }
    // Decode non-ASCII bytes with Jomini itself, leaving ASCII escapes and
    // whitespace untouched. Its whole-string decoder also unescapes/trims,
    // which must happen only on scalars, not on the complete source file.
    let mut output = String::with_capacity(data.len());
    for byte in data {
        if byte.is_ascii() {
            output.push(*byte as char);
        } else {
            output.push_str(&Windows1252Encoding::decode(std::slice::from_ref(byte)));
        }
    }
    if output.len() > crate::MAX_BYTES {
        return Err("Script decodificato oltre 80 MB".into());
    }
    Ok((Cow::Owned(output), "windows-1252"))
}
