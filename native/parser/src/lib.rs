//! Jomini-backed, ordered compatibility AST. No game/process/file access.
use serde::Serialize;
use std::ffi::{CString, c_char};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_BYTES: usize = 80_000_000;
const MAX_DEPTH: usize = 180;

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Scalar(String),
    Block(Node),
}

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Node {
    pub entries: Vec<(String, Value)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<models::Projection>,
}

pub mod analysis;
mod cwtools_input;
mod recovery;

/// # Safety
/// `data` must address `len` readable UTF-8 bytes. Free with `hoi4_parser_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_cwtools_prepare(data: *const u8, len: usize) -> *mut c_char {
    if data.is_null() || len > MAX_BYTES {
        return std::ptr::null_mut();
    }
    let result = std::panic::catch_unwind(|| {
        cwtools_input::prepare(unsafe { std::slice::from_raw_parts(data, len) })
    });
    let bytes = result.unwrap_or_else(|_| {
        br#"{"schema_version":1,"error":"CWTools preparation panic"}"#.to_vec()
    });
    CString::new(bytes).expect("JSON has no raw NUL").into_raw()
}
mod document;
mod input;
mod models;

/// # Safety
/// Both pointers must address readable byte ranges for the supplied lengths.
/// `host` is an absolute UTF-8 executable path. Free with `hoi4_parser_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_cwtools_analyze(
    host: *const u8,
    host_len: usize,
    data: *const u8,
    len: usize,
) -> *mut c_char {
    if host.is_null() || data.is_null() || host_len > 32768 || len > 16_000_000 {
        return std::ptr::null_mut();
    }
    let result = std::panic::catch_unwind(|| {
        let path = std::str::from_utf8(unsafe { std::slice::from_raw_parts(host, host_len) })
            .map_err(|e| e.to_string())?;
        analysis::run(
            std::path::Path::new(path),
            unsafe { std::slice::from_raw_parts(data, len) },
            std::time::Duration::from_secs(300),
        )
    })
    .unwrap_or_else(|_| Err("CWTools bridge panic".into()));
    let bytes = result.unwrap_or_else(|error| {
        serde_json::to_vec(&serde_json::json!({"schema_version":1,"error":error})).unwrap()
    });
    CString::new(bytes).expect("JSON has no raw NUL").into_raw()
}

pub fn parse(source: &[u8]) -> Result<Node, String> {
    document::parse(source)
}

pub fn response(source: &[u8]) -> Vec<u8> {
    process(source, 1, 0)
}

pub fn process(source: &[u8], encoding: u32, mode: u32) -> Vec<u8> {
    let result=(|| {
        let (decoded,encoding)=input::decode(source,encoding)?;
        let mut result=serde_json::json!({"schema_version":SCHEMA_VERSION,"engine":"jomini","engine_version":"0.37.2",
            "document_model":"TextTape","typed_models":2,"reviewed_recovery":1,"source_encoding":encoding,
            "capabilities":["windows-1252","jomini-json","typed-focus-relations"]});
        if mode==0 || mode==4 {
            let node=match parse(decoded.as_bytes()) {
                Ok(node)=>node,
                Err(original) if mode==4=>match recovery::recover(&decoded)? {
                    Some((source,evidence))=>{
                        result["source_recovery"]=serde_json::to_value(evidence).map_err(|e|e.to_string())?;
                        parse(source.as_bytes())?
                    }
                    None=>return Err(original),
                },
                Err(error)=>return Err(error),
            };
            result["node"]=serde_json::to_value(node).map_err(|e|e.to_string())?;
        }
        else {
            result["json"]=document::export_json(decoded.as_bytes(),mode)?.into();
            result["format"]="jomini-json".into();
            result["duplicate_keys"]=match mode { 1=>"key_value_pairs",2=>"group",3=>"preserve",_=>return Err("Modalità JSON non supportata".into()) }.into();
            result["type_narrowing"]="none".into();
        }
        Ok::<_,String>(result)
    })().unwrap_or_else(|error|serde_json::json!({"schema_version":SCHEMA_VERSION,"error":error}));
    serde_json::to_vec(&result).expect("serializable parser result")
}

#[unsafe(no_mangle)]
pub extern "C" fn hoi4_parser_schema_version() -> u32 {
    SCHEMA_VERSION
}

/// # Safety
/// `data` must point to `len` readable bytes (or be null for empty input).
/// Free the returned allocation exactly once with `hoi4_parser_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_parser_parse(data: *const u8, len: usize) -> *mut c_char {
    unsafe { hoi4_parser_process(data, len, 1, 0) }
}

/// # Safety
/// `data` must point to `len` readable bytes (or be null for empty input).
/// Encoding: 0 auto, 1 UTF-8, 2 Windows-1252. Mode: 0 strict AST, 1 pairs,
/// 2 group, 3 preserve, 4 AST with content-pinned reviewed recovery.
/// Free the returned allocation exactly once with `hoi4_parser_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_parser_process(
    data: *const u8,
    len: usize,
    encoding: u32,
    mode: u32,
) -> *mut c_char {
    if len > MAX_BYTES || (data.is_null() && len != 0) {
        return std::ptr::null_mut();
    }
    let outcome = std::panic::catch_unwind(|| {
        let bytes = if len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(data, len) }
        };
        process(bytes, encoding, mode)
    });
    let bytes =
        outcome.unwrap_or_else(|_| br#"{"schema_version":1,"error":"Parser panic"}"#.to_vec());
    CString::new(bytes).expect("JSON has no raw NUL").into_raw()
}

/// # Safety
/// `data` must be null or an unfreed pointer returned by `hoi4_parser_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_parser_free(data: *mut c_char) {
    if !data.is_null() {
        drop(unsafe { CString::from_raw(data) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows1252_decoding_and_json_export_preserve_text_and_duplicate_order() {
        let result: serde_json::Value = serde_json::from_slice(&process(
            b"a=001 b=yes a=002 name=\"Citt\xe0 \x80\" path=\"C:\\mods\\n \"",
            0,
            1,
        ))
        .unwrap();
        assert_eq!(result["source_encoding"], "windows-1252");
        let output: serde_json::Value =
            serde_json::from_str(result["json"].as_str().unwrap()).unwrap();
        assert_eq!(
            output["val"],
            serde_json::json!([
                ["a", "001"],
                ["b", "yes"],
                ["a", "002"],
                ["name", "Città €"],
                ["path", "C:\\mods\\n "]
            ])
        );
    }

    #[test]
    fn strict_encoding_errors_are_not_hidden_by_fallback() {
        for (source, encoding) in [
            (&b"x=\"\xe0\""[..], 1),
            (&b"\xef\xbb\xbfx=\"\xe0\""[..], 0),
            (&b"HOI4bin"[..], 0),
            (&b"x=\"\x81\""[..], 2),
            (&b"x=\0"[..], 0),
        ] {
            let result: serde_json::Value =
                serde_json::from_slice(&process(source, encoding, 0)).unwrap();
            assert!(result.get("error").is_some(), "{source:?}");
        }
        let result: serde_json::Value =
            serde_json::from_slice(&process(b"# comment \x16\nx=yes", 0, 0)).unwrap();
        assert!(result.get("node").is_some());
    }

    #[test]
    fn order_duplicates_operators_and_lists_are_preserved() {
        let n =
            parse(br#"a=1 a=2 p={focus=A focus=B} p={focus=C} x >= 3 items={yes no 001 1936.1.1}"#)
                .unwrap();
        assert_eq!(
            n.entries.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
            ["a", "a", "p", "p", "x >=", "items"]
        );
        assert_eq!(n.entries[0].1, Value::Scalar("1".into()));
        if let Value::Block(v) = &n.entries[5].1 {
            assert_eq!(v.entries[2], ("".into(), Value::Scalar("001".into())));
        } else {
            panic!();
        }
    }
    #[test]
    fn quoted_reserved_characters_paths_unicode_and_escapes() {
        let n = parse("x=\"{\" y=\"=\" z=\"C:\\mods\\a\\n \" t=\"Łódź\"".as_bytes()).unwrap();
        assert_eq!(n.entries[0].1, Value::Scalar("{".into()));
        assert_eq!(n.entries[2].1, Value::Scalar("C:\\mods\\a\\n ".into()));
        assert_eq!(n.entries[3].1, Value::Scalar("Łódź".into()));
    }
    #[test]
    fn malformed_and_deep_input_fail_without_partial_nodes() {
        for text in ["x={a=1", "x=}", "x=\"unterminated", "x=", "}"] {
            assert!(parse(text.as_bytes()).is_err(), "{text}");
        }
        assert!(parse(format!("{}{}", "x={".repeat(181), "}".repeat(181)).as_bytes()).is_err());
        assert!(parse(&[255]).is_err());
    }
    #[test]
    fn ffi_allocation_roundtrip_and_version() {
        assert_eq!(hoi4_parser_schema_version(), 1);
        let data = b"x=yes";
        let ptr = unsafe { hoi4_parser_parse(data.as_ptr(), data.len()) };
        assert!(!ptr.is_null());
        let json = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str().unwrap();
        assert!(json.contains("jomini"));
        unsafe {
            hoi4_parser_free(ptr);
            hoi4_parser_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn hoi4_inline_ids_and_dynamic_values_remain_opaque_and_whole() {
        let n = parse(
            b"add_ideas=CZE_example_[COUNTRY] title=[ROOT.GetName] cost=war_bonds?50 x=@[a+2]",
        )
        .unwrap();
        let expected = [
            "CZE_example_[COUNTRY]",
            "[ROOT.GetName]",
            "war_bonds?50",
            "@[a+2]",
        ];
        for ((_, v), text) in n.entries.iter().zip(expected) {
            assert_eq!(*v, Value::Scalar(text.into()));
        }
        assert_eq!(n.entries.len(), 4);
    }

    #[test]
    fn anonymous_root_lists_and_quoted_neighbours_are_not_merged() {
        let n = parse(b"one two three").unwrap();
        assert_eq!(n.entries.len(), 3);
        let n = parse(br#"items={"one""two" A B} key="a""#).unwrap();
        if let Value::Block(items) = &n.entries[0].1 {
            assert_eq!(items.entries.len(), 4);
        } else {
            panic!();
        }
        assert_eq!(n.entries[1].1, Value::Scalar("a".into()));
    }

    #[test]
    fn texttape_preserves_mixed_order_and_anonymous_empty_blocks() {
        let n = parse(b"x={a=1 empty={} two x=2 three} {} end=yes").unwrap();
        if let Value::Block(body) = &n.entries[0].1 {
            assert_eq!(
                body.entries
                    .iter()
                    .map(|e| e.0.as_str())
                    .collect::<Vec<_>>(),
                ["a", "empty", "", "x", ""]
            );
        } else {
            panic!();
        }
        assert!(matches!(&n.entries[1].1, Value::Block(_)));
        assert_eq!(n.entries[2], ("end".into(), Value::Scalar("yes".into())));
    }

    #[test]
    fn typed_models_use_jomini_properties_without_losing_unknown_data() {
        let n = parse(b"focus_tree={focus={id=A x=1 x=2 y>=3 cost=@cost unknown={x=1}}}").unwrap();
        if let Value::Block(tree) = &n.entries[0].1 {
            if let Value::Block(focus) = &tree.entries[0].1 {
                assert!(
                    matches!(&focus.projection,Some(models::Projection::Focus{x:Some(x),y:None,..}) if x=="1")
                );
                assert!(focus.entries.iter().any(|entry| entry.0 == "unknown"));
            } else {
                panic!();
            }
        } else {
            panic!();
        }
    }
}
