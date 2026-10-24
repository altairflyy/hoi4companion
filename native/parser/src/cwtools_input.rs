//! Lexical compatibility for CWTools, using Jomini's tokenizer. Structural
//! adaptations are separately audited and pinned to exact source hashes.
use jomini::text::{Token, TokenReader};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Edit {
    line: usize,
    column: usize,
    original_length: usize,
    replacement_length: usize,
    prefix_added: usize,
    kind: &'static str,
}

#[derive(PartialEq)]
enum Kind {
    Scalar,
    Operator,
    Open,
    Other,
}
struct Span {
    start: usize,
    end: usize,
    kind: Kind,
    bare: bool,
}

fn location(source: &str, byte: usize) -> (usize, usize) {
    let prefix = &source[..byte];
    (
        prefix.bytes().filter(|b| *b == b'\n').count() + 1,
        prefix
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .encode_utf16()
            .count(),
    )
}

fn normalize(source: &str) -> Result<(String, Vec<Edit>), String> {
    let bytes = source.as_bytes();
    let base = bytes.as_ptr() as usize;
    let mut reader = TokenReader::from_slice(bytes);
    let mut spans = Vec::new();
    while let Some(token) = reader.next().map_err(|e| e.to_string())? {
        let (start, width, kind, bare) = match token {
            Token::Unquoted(s) => (
                Some(s.as_bytes().as_ptr() as usize - base),
                s.as_bytes().len(),
                Kind::Scalar,
                true,
            ),
            Token::Quoted(s) => (
                Some(s.as_bytes().as_ptr() as usize - base - 1),
                s.as_bytes().len() + 2,
                Kind::Scalar,
                false,
            ),
            Token::Operator(op) => (None, op.to_string().len(), Kind::Operator, false),
            Token::Open => (None, 1, Kind::Open, false),
            _ => (None, 1, Kind::Other, false),
        };
        let position = reader.position();
        let (start, end) = match start {
            Some(start) => (start, start + width),
            None => (position - width, position),
        };
        spans.push(Span {
            start,
            end,
            kind,
            bare,
        });
    }
    let mut changes = Vec::new();
    for (index, span) in spans.iter().enumerate() {
        if index > 0 && spans[index - 1].kind == Kind::Operator {
            let start = spans[index - 1].end;
            let gap = &source[start..span.start];
            if gap.contains('#') {
                // CWTools accepts comments as statements but not after '='.
                // Replace only that trivia, retaining UTF-16 columns and lines.
                let replacement: String = gap
                    .chars()
                    .flat_map(|c| match c {
                        '\r' | '\n' | '\t' => vec![c],
                        _ => vec![' '; c.len_utf16()],
                    })
                    .collect();
                changes.push((start, span.start, replacement, "operator_comment", 0));
            }
        }
        if !span.bare {
            continue;
        }
        let raw = &source[span.start..span.end];
        let next = spans.get(index + 1);
        let hsv_360 = matches!(raw, "HSV" | "hsv")
            && next.is_some_and(|n| &source[n.start..n.end] == "360")
            && spans.get(index + 2).is_some_and(|n| n.kind == Kind::Open);
        let reserved_prefix = ["RGB", "rgb", "HSV", "hsv"]
            .iter()
            .any(|prefix| raw.starts_with(prefix));
        let real_color = matches!(raw, "RGB" | "rgb" | "HSV" | "hsv")
            && next.is_some_and(|n| n.kind == Kind::Open);
        let color_word = reserved_prefix
            && !real_color
            && !hsv_360
            && !next.is_some_and(|n| n.kind == Kind::Operator);
        if !raw.is_ascii() || raw.contains('"') || color_word {
            // Quoting opaque scalars avoids CWTools' restricted character set
            // and its eager interpretation of the country tag RGB as a color.
            let replacement = format!("\"{}\"", raw.replace('"', "\\\""));
            changes.push((
                span.start,
                span.end,
                replacement,
                if color_word {
                    "scalar_color_name"
                } else if raw.contains('"') {
                    "embedded_quote"
                } else {
                    "unicode_scalar"
                },
                1,
            ));
        }
    }
    changes.sort_by_key(|e| e.0);
    let mut result = String::with_capacity(source.len());
    let mut position = 0;
    let mut edits = Vec::new();
    for (start, end, replacement, kind, prefix_added) in changes {
        result.push_str(&source[position..start]);
        result.push_str(&replacement);
        position = end;
        let (line, column) = location(source, start);
        let (original_length, replacement_length) = if kind == "operator_comment" {
            (0, 0)
        } else {
            (
                source[start..end].encode_utf16().count(),
                replacement.encode_utf16().count(),
            )
        };
        edits.push(Edit {
            line,
            column,
            original_length,
            replacement_length,
            prefix_added,
            kind,
        });
    }
    result.push_str(&source[position..]);
    Ok((result, edits))
}

pub fn prepare(data: &[u8]) -> Vec<u8> {
    let result = match std::str::from_utf8(data) {
        Ok(original) => match crate::recovery::recover(original) {
          Ok(recovery) => {
            let source=recovery.as_ref().map_or(original,|(text,_)|text.as_str());
            match normalize(source) {
            Ok((source, edits)) => {
                serde_json::json!({"schema_version":1,"parser_compatibility":1,"reviewed_recovery":1,
                  "source":source,"edits":edits,"recovery":recovery.map(|(_,evidence)|evidence)})
            }
            // A broken string remains broken and is diagnosed by CWTools.
            Err(error) => {
                serde_json::json!({"schema_version":1,"parser_compatibility":1,"reviewed_recovery":1,"source":original,"edits":[],"lexer_error":error})
            }
          }
          },
          Err(error)=>serde_json::json!({"schema_version":1,"error":error}),
        },
        Err(error) => serde_json::json!({"schema_version":1,"error":error.to_string()}),
    };
    serde_json::to_vec(&result).expect("serializable lexical result")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tag_color_trivia_unicode_and_strings_keep_their_values() {
        let source = "original_tag=RGB color=RGB {1 2 3} HSV_key=yes x=\"RGB\" a= # note\n {n=250§ name=“Shipka”}";
        let (result, edits) = normalize(source).unwrap();
        assert!(
            result.contains("original_tag=\"RGB\" color=RGB {1 2 3}"),
            "{result}"
        );
        assert!(result.contains("x=\"RGB\" a=       \n {n=\"250§\" name=\"“Shipka”\"}"));
        assert_eq!(result.lines().count(), source.lines().count());
        assert_eq!(edits.len(), 4);
        assert_eq!(
            crate::parse(source.as_bytes()).unwrap().entries,
            crate::parse(result.as_bytes()).unwrap().entries
        );
    }
    #[test]
    fn never_fills_values_closes_blocks_or_changes_comments_inside_strings() {
        for source in ["x={a=1", "x=}", "x=", "x=\"# literal\"", "x=\"unterminated"] {
            let result: serde_json::Value =
                serde_json::from_slice(&prepare(source.as_bytes())).unwrap();
            assert_eq!(result["source"], source);
            assert_eq!(result["edits"], serde_json::json!([]));
        }
    }

    #[test]
    fn color_prefixed_ids_and_embedded_quotes_remain_opaque() {
        let source =
            "key=RGB_democratic_RU_Gen color=hsv 360 {1 2 3} names={Bare tail\" } #comment\"";
        let (result, _) = normalize(source).unwrap();
        assert!(result.contains("key=\"RGB_democratic_RU_Gen\" color=hsv 360 {1 2 3}"));
        assert!(result.contains("names={Bare \"tail\\\"\" } #comment\""));
        assert_eq!(
            crate::parse(source.as_bytes()).unwrap().entries,
            crate::parse(result.as_bytes()).unwrap().entries
        );
    }
}
