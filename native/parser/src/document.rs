//! Jomini owns container parsing; this module only normalizes lexical edge cases
//! in memory and translates its public DOM into Companion's ordered wire shape.
use crate::{MAX_BYTES, MAX_DEPTH, Node, Value};
use jomini::text::{Operator, Token, TokenReader};
use jomini::{TextTape, TextToken};

fn quoted(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + 2);
    out.push(b'"');
    for &byte in raw {
        if matches!(byte, b'"' | b'\\') {
            out.push(b'\\');
        }
        out.push(byte);
    }
    out.push(b'"');
    out
}

pub(crate) fn scalar(raw: &[u8], quoted: bool) -> Result<String, String> {
    let text = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    if !quoted {
        return Ok(text.to_owned());
    }
    let mut chars = text.chars().peekable();
    let mut output = String::with_capacity(text.len());
    while let Some(c) = chars.next() {
        if c == '\\' && matches!(chars.peek(), Some('"' | '\\')) {
            output.push(chars.next().unwrap());
        } else {
            output.push(c);
        }
    }
    Ok(output)
}

// Only byte-preserving quotation of inline [...] IDs and retention of anonymous
// empty blocks are needed. No fields, containers, expressions or values are
// interpreted here. Braces are validated because TextTape tolerates some EOFs.
fn normalise(source: &[u8], anonymous: &str) -> Result<Vec<u8>, String> {
    let mut reader = TokenReader::from_slice(source);
    let mut edits: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    let mut pending: Option<(usize, usize)> = None;
    let mut depth = 0usize;
    let mut open = None;
    let mut after_operator = false;
    let flush = |pending: &mut Option<(usize, usize)>, edits: &mut Vec<_>| {
        if let Some((start, end)) = pending.take() {
            let raw = &source[start..end];
            if raw.contains(&b'[') || raw.contains(&b']') {
                edits.push((start, end, quoted(raw)));
            }
        }
    };
    while let Some(token) = reader.next().map_err(|e| e.to_string())? {
        if let Token::Unquoted(s) = token {
            let raw = s.as_bytes();
            let start = (raw.as_ptr() as usize)
                .checked_sub(source.as_ptr() as usize)
                .ok_or("Invalid source span")?;
            let end = start + raw.len();
            if source.get(start..end) != Some(raw) {
                return Err("Invalid source span".into());
            }
            if let Some((_, previous_end)) = pending.as_mut()
                && *previous_end == start
            {
                *previous_end = end;
                continue;
            }
            flush(&mut pending, &mut edits);
            pending = Some((start, end));
            open = None;
            after_operator = false;
            continue;
        }
        flush(&mut pending, &mut edits);
        let next_after_operator = matches!(token, Token::Operator(_));
        match token {
            Token::Open => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err("ProfonditÃ  eccessiva nello script".into());
                }
                open = Some((reader.position() - 1, !after_operator));
            }
            Token::Close => {
                depth = depth
                    .checked_sub(1)
                    .ok_or("Parentesi graffa chiusa senza apertura")?;
                if let Some((position, true)) = open {
                    edits.push((position, position, format!("{anonymous}=").into_bytes()));
                }
                open = None;
            }
            _ => open = None,
        }
        after_operator = next_after_operator;
    }
    flush(&mut pending, &mut edits);
    if depth != 0 {
        return Err("Blocco aperto non chiuso".into());
    }
    edits.sort_by_key(|edit| edit.0);
    let mut out = Vec::with_capacity(source.len() + 20);
    // A single wrapper also lets Jomini represent anonymous root lists.
    out.extend_from_slice(b"root={\n");
    let mut position = 0;
    for (start, end, replacement) in edits {
        out.extend_from_slice(&source[position..start]);
        out.extend_from_slice(&replacement);
        position = end;
    }
    out.extend_from_slice(&source[position..]);
    out.extend_from_slice(b"\n}");
    Ok(out)
}

// Traverse only Jomini's completed document spans. Object/array decisions and
// matched ends come from TextTape; there is no raw-token block parser here.
fn container(
    tokens: &[TextToken],
    start: usize,
    end: usize,
    object: bool,
    anonymous: &str,
    depth: usize,
    projections: &mut std::collections::HashMap<usize, crate::models::Projection>,
) -> Result<Node, String> {
    if depth > MAX_DEPTH + 1 {
        return Err("ProfonditÃ  eccessiva nello script".into());
    }
    let mut node = Node::default();
    let mut pos = start;
    let mut mixed = false;
    while pos < end {
        if matches!(tokens[pos], TextToken::MixedContainer) {
            mixed = true;
            pos += 1;
            continue;
        }
        let paired = if mixed {
            matches!(tokens.get(pos + 1), Some(TextToken::Operator(_)))
        } else {
            object
        };
        let mut key = String::new();
        if paired {
            let raw = tokens[pos].as_scalar().ok_or("Non-scalar DOM key")?;
            key = scalar(raw.as_bytes(), matches!(tokens[pos], TextToken::Quoted(_)))?;
            pos += 1;
            if let Some(TextToken::Operator(op)) = tokens.get(pos) {
                if *op != Operator::Equal {
                    key = format!("{key} {op}");
                }
                pos += 1;
            }
            if key == anonymous {
                key.clear();
            }
        }
        if pos >= end {
            return Err("Valore mancante nel documento".into());
        }
        let header = matches!(tokens[pos], TextToken::Header(_));
        let item = value(tokens, &mut pos, anonymous, depth, projections)?;
        node.entries.push((key, item));
        if header {
            let item = value(tokens, &mut pos, anonymous, depth, projections)?;
            node.entries.push((String::new(), item));
        }
    }
    Ok(node)
}

fn value(
    tokens: &[TextToken],
    pos: &mut usize,
    anonymous: &str,
    depth: usize,
    projections: &mut std::collections::HashMap<usize, crate::models::Projection>,
) -> Result<Value, String> {
    let token = tokens.get(*pos).ok_or("Missing DOM value")?;
    match token {
        TextToken::Object { end, .. } | TextToken::Array { end, .. } => {
            let object = matches!(token, TextToken::Object { .. });
            let mut node = container(
                tokens,
                *pos + 1,
                *end,
                object,
                anonymous,
                depth + 1,
                projections,
            )?;
            node.projection = projections.remove(pos);
            *pos = end + 1;
            Ok(Value::Block(node))
        }
        TextToken::Quoted(raw) | TextToken::Unquoted(raw) | TextToken::Header(raw) => {
            let text = scalar(raw.as_bytes(), matches!(token, TextToken::Quoted(_)))?;
            *pos += 1;
            Ok(Value::Scalar(text))
        }
        other => Err(format!("Unsupported DOM value: {other:?}")),
    }
}

fn with_document<T>(
    source: &[u8],
    consume: impl FnOnce(&mut TextTape, &str) -> Result<T, String>,
) -> Result<T, String> {
    if source.len() > MAX_BYTES {
        return Err("Script oltre 80 MB".into());
    }
    std::str::from_utf8(source).map_err(|e| e.to_string())?;
    let source = source.strip_prefix(b"\xef\xbb\xbf").unwrap_or(source);
    let mut anonymous = String::from("__companion_anonymous_block__");
    while source
        .windows(anonymous.len())
        .any(|part| part == anonymous.as_bytes())
    {
        anonymous.push('_');
    }
    let normalised = normalise(source, &anonymous)?;
    let mut tape = TextTape::from_slice(&normalised).map_err(|e| e.to_string())?;
    consume(&mut tape, &anonymous)
}

pub fn parse(source: &[u8]) -> Result<Node, String> {
    with_document(source, |tape, anonymous| {
        let mut projections = crate::models::extract(tape)?;
        match value(tape.tokens(), &mut 1, anonymous, 0, &mut projections)? {
            Value::Block(root) => Ok(root),
            _ => Err("Invalid wrapper".into()),
        }
    })
}

pub fn export_json(source: &[u8], mode: u32) -> Result<String, String> {
    use jomini::json::{DuplicateKeyMode, JsonOptions, TypeNarrowing};
    use jomini::text::ObjectReader;
    let duplicates = match mode {
        1 => DuplicateKeyMode::KeyValuePairs,
        2 => DuplicateKeyMode::Group,
        3 => DuplicateKeyMode::Preserve,
        _ => return Err("Modalità JSON non supportata".into()),
    };
    with_document(source, |tape, anonymous| {
        // The synthetic empty-block key is represented as an anonymous/empty
        // key in the external export, never exposed as an internal marker.
        for token in tape.tokens_mut() {
            if let TextToken::Unquoted(raw) = token
                && raw.as_bytes() == anonymous.as_bytes()
            {
                *raw = jomini::Scalar::new(b"");
            }
        }
        let positions = crate::models::quoted_positions(tape);
        let reader = ObjectReader::new(tape, crate::models::PreserveEncoding(&positions));
        let (_, _, root) = reader.fields().next().ok_or("Missing wrapper")?;
        let options = JsonOptions::new()
            .with_type_narrowing(TypeNarrowing::None)
            .with_duplicate_keys(duplicates);
        Ok(root.json().with_options(options).to_string())
    })
}
