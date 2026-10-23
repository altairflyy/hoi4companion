//! Typed source fields deserialized by Jomini itself. Expressions remain strings:
//! unknown/dynamic values must not turn into observed facts or invented numbers.
use jomini::text::{ObjectReader, Operator, Property};
use jomini::{Encoding, JominiDeserialize, TextTape, TextToken};
use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

#[derive(Debug)]
struct ScriptScalar(Option<String>);

struct ScriptBlock<T>(Option<T>);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for ScriptBlock<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BlockVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for BlockVisitor<T> {
            type Value = ScriptBlock<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a script block")
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(ScriptBlock(None))
            }
            fn visit_map<A: MapAccess<'de>>(self, values: A) -> Result<Self::Value, A::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(values))
                    .map(|item| ScriptBlock(Some(item)))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
                while values.next_element::<IgnoredAny>()?.is_some() {}
                Ok(ScriptBlock(None))
            }
        }
        deserializer.deserialize_any(BlockVisitor(std::marker::PhantomData))
    }
}

#[derive(JominiDeserialize)]
struct FocusReferences {
    #[jomini(duplicated)]
    focus: Vec<Property<ScriptScalar>>,
}

impl<'de> Deserialize<'de> for ScriptScalar {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ScalarVisitor;
        impl<'de> Visitor<'de> for ScalarVisitor {
            type Value = ScriptScalar;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a script scalar or a non-scalar block")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(ScriptScalar(Some(value.to_owned())))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
                while values.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(ScriptScalar(None))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
                while values.next_element::<IgnoredAny>()?.is_some() {}
                Ok(ScriptScalar(None))
            }
        }
        deserializer.deserialize_any(ScalarVisitor)
    }
}

#[derive(JominiDeserialize)]
struct FocusFields {
    #[jomini(duplicated)]
    id: Vec<Property<ScriptScalar>>,
    #[jomini(duplicated)]
    x: Vec<Property<ScriptScalar>>,
    #[jomini(duplicated)]
    y: Vec<Property<ScriptScalar>>,
    #[jomini(duplicated)]
    cost: Vec<Property<ScriptScalar>>,
    #[jomini(duplicated)]
    relative_position_id: Vec<Property<ScriptScalar>>,
    #[jomini(duplicated)]
    icon: Vec<Property<ScriptScalar>>,
    #[jomini(duplicated)]
    prerequisite: Vec<Property<ScriptBlock<FocusReferences>>>,
    #[jomini(duplicated)]
    mutually_exclusive: Vec<Property<ScriptBlock<FocusReferences>>>,
}

#[derive(JominiDeserialize)]
struct TechnologyFields {
    #[jomini(duplicated)]
    research_cost: Vec<Property<ScriptScalar>>,
    #[jomini(duplicated)]
    start_year: Vec<Property<ScriptScalar>>,
    #[jomini(duplicated)]
    is_special_project_tech: Vec<Property<ScriptScalar>>,
}

// The existing framework uses the first equal-key value, including a first
// block masking later scalars. Other operators must not masquerade as '='.
fn first(values: &[Property<ScriptScalar>]) -> Option<String> {
    values
        .iter()
        .find(|value| value.operator() == Operator::Equal)
        .and_then(|value| value.value().0.clone())
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Projection {
    Focus {
        id: Option<String>,
        x: Option<String>,
        y: Option<String>,
        cost: Option<String>,
        relative_position_id: Option<String>,
        icon: Option<String>,
        prerequisites: Vec<Vec<String>>,
        mutually_exclusive: Vec<String>,
    },
    Technology {
        research_cost: Option<String>,
        start_year: Option<String>,
        is_special_project_tech: Option<String>,
    },
}

// Jomini's default decoder removes backslashes and trims trailing spaces.
// Preserve the framework's string contract while using its typed deserializer.
#[derive(Clone, Copy)]
pub(crate) struct PreserveEncoding<'a>(pub(crate) &'a HashSet<usize>);
impl Encoding for PreserveEncoding<'_> {
    fn decode<'a>(&self, raw: &'a [u8]) -> Cow<'a, str> {
        Cow::Owned(
            crate::document::scalar(raw, self.0.contains(&(raw.as_ptr() as usize)))
                .expect("validated UTF-8 tape"),
        )
    }
}

fn focus(reader: ObjectReader<PreserveEncoding>) -> Result<Projection, String> {
    let raw: FocusFields = reader.deserialize().map_err(|e| e.to_string())?;
    Ok(Projection::Focus {
        id: first(&raw.id),
        x: first(&raw.x),
        y: first(&raw.y),
        cost: first(&raw.cost),
        relative_position_id: first(&raw.relative_position_id),
        icon: first(&raw.icon),
        prerequisites: references(&raw.prerequisite),
        mutually_exclusive: {
            let mut items = Vec::new();
            for item in references(&raw.mutually_exclusive).into_iter().flatten() {
                if !items.contains(&item) {
                    items.push(item);
                }
            }
            items
        },
    })
}

fn references(blocks: &[Property<ScriptBlock<FocusReferences>>]) -> Vec<Vec<String>> {
    blocks
        .iter()
        .filter(|block| block.operator() == Operator::Equal)
        .filter_map(|block| block.value().0.as_ref())
        .map(|block| {
            block
                .focus
                .iter()
                .filter(|item| item.operator() == Operator::Equal)
                .filter_map(|item| item.value().0.clone())
                .collect::<Vec<_>>()
        })
        .filter(|group| !group.is_empty())
        .collect()
}

fn technology(reader: ObjectReader<PreserveEncoding>) -> Result<Projection, String> {
    let raw: TechnologyFields = reader.deserialize().map_err(|e| e.to_string())?;
    Ok(Projection::Technology {
        research_cost: first(&raw.research_cost),
        start_year: first(&raw.start_year),
        is_special_project_tech: first(&raw.is_special_project_tech),
    })
}

fn index(tape: &TextTape, token: &TextToken) -> usize {
    // Both references belong to the same borrowed tape; no dereference or
    // reconstructed pointer is used. Index only attaches metadata to that block.
    (token as *const TextToken as usize - tape.tokens().as_ptr() as usize)
        / std::mem::size_of::<TextToken>()
}

pub(crate) fn quoted_positions(tape: &TextTape) -> HashSet<usize> {
    tape.tokens()
        .iter()
        .filter_map(|token| {
            if let TextToken::Quoted(raw) = token {
                Some(raw.as_bytes().as_ptr() as usize)
            } else {
                None
            }
        })
        .collect()
}

pub fn extract(tape: &TextTape) -> Result<HashMap<usize, Projection>, String> {
    let mut result = HashMap::new();
    let quoted = quoted_positions(tape);
    let reader = ObjectReader::new(tape, PreserveEncoding(&quoted));
    let (_, _, root) = reader.fields().next().ok_or("Missing root")?;
    if !matches!(root.token(), TextToken::Object { mixed: false, .. }) {
        return Ok(result);
    }
    for (key, operator, group) in root.read_object().map_err(|e| e.to_string())?.fields() {
        if !matches!(operator, None | Some(Operator::Equal)) {
            continue;
        }
        match key.read_scalar().as_bytes() {
            b"shared_focus" | b"joint_focus"
                if matches!(group.token(), TextToken::Object { mixed: false, .. }) =>
            {
                result.insert(
                    index(tape, group.token()),
                    focus(group.read_object().map_err(|e| e.to_string())?)?,
                );
            }
            b"focus_tree" | b"technologies"
                if matches!(group.token(), TextToken::Object { mixed: false, .. }) =>
            {
                let technologies = key.read_scalar().as_bytes() == b"technologies";
                for (key, operator, item) in
                    group.read_object().map_err(|e| e.to_string())?.fields()
                {
                    if !matches!(operator, None | Some(Operator::Equal))
                        || !matches!(item.token(), TextToken::Object { mixed: false, .. })
                    {
                        continue;
                    }
                    if technologies && !key.read_scalar().as_bytes().starts_with(b"@") {
                        result.insert(
                            index(tape, item.token()),
                            technology(item.read_object().map_err(|e| e.to_string())?)?,
                        );
                    } else if !technologies && key.read_scalar().as_bytes() == b"focus" {
                        result.insert(
                            index(tape, item.token()),
                            focus(item.read_object().map_err(|e| e.to_string())?)?,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    Ok(result)
}
