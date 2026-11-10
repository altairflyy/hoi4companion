//! Event discovery is provisional; selected owners, bindings and visible rows
//! are always checked again through the coherent witnessed decoder.
use super::{Reader, Result, hex};
use crate::live::Memory;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
impl<M: Memory> Reader<'_, M> {
    fn discover_windows(&mut self, root: u64) -> Result<Vec<u64>> {
        let mut headers = vec![];
        let registry = self.attributes["gui_layout"]["registry"]
            .as_array()
            .map(|v| v.iter().filter_map(Value::as_u64).collect::<Vec<_>>())
            .unwrap_or(vec![264, 288]);
        if registry.len() != 2 {
            return Err("Invalid event registry layout".into());
        }
        for &offset in &registry {
            headers.push(hex(&self.track(root + offset, 16)?))
        }
        let cache = self.attributes["event_cache"].clone();
        if cache[0] == root
            && cache[2] == json!(headers)
            && cache[1].as_array().is_some_and(|v| !v.is_empty())
        {
            let cached = cache[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().ok_or_else(|| "Invalid event cache owner".into()))
                .collect::<Result<Vec<_>>>()?;
            // Cached owners are hints, not a complete window inventory. A
            // closed/recycled popup must not permanently hide its successor
            // when the registry reuses its allocation and count.
            let mut active=false;
            let valid=cached.iter().all(|owner| (|| -> Result<bool> {
                self.typed(*owner,"CEventWindow")?;
                let container=self.object_pointer(owner+0x50,"CContainerWindow")?;
                let gui=container+48;
                if self.pointer(gui+104)? != root || self.pointer(gui+80)? != owner+56 {return Ok(false);}
                let shown=self.track(gui+117,1)?[0]&8 != 0;
                active |= shown && self.pointer(owner+0x68)? != 0;
                Ok(true)
            })().unwrap_or(false));
            if valid && active { return Ok(cached); }
        }
        let mut owners = BTreeSet::new();
        let mut seen = HashSet::new();
        let mut event_types = std::collections::HashMap::<u64, bool>::new();
        for (offset, dynamic) in [(registry[0], false), (registry[1], true)] {
            let raw = self.track(root + offset, 16)?;
            let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
            let capacity = i32::from_le_bytes(raw[8..12].try_into().unwrap());
            let count = i32::from_le_bytes(raw[12..].try_into().unwrap());
            if count < 0 || count > capacity || capacity > 500000 {
                return Err("Registro GUI fuori intervallo".into());
            }
            let extent = if dynamic { capacity } else { count } as usize;
            for start in (0..extent).step_by(8192) {
                let data = self
                    .d
                    .memory
                    .discovery_read(data + start as u64 * 8, (extent - start).min(8192) * 8)?;
                for row in data.as_chunks::<8>().0 {
                    let gui = u64::from_le_bytes(*row);
                    if gui == 0 || !seen.insert(gui) {
                        continue;
                    }
                    let candidate = (|| {
                        let bytes = self.d.memory.discovery_read(
                            gui.checked_add(80).ok_or("Invalid GUI discovery address")?,
                            8,
                        )?;
                        let parent = u64::from_le_bytes(
                            bytes.try_into().map_err(|_| "Partial GUI discovery")?,
                        );
                        if parent != 0 {
                            let bytes = self.d.memory.discovery_read(parent, 8)?;
                            let vt = u64::from_le_bytes(
                                bytes.try_into().map_err(|_| "Partial GUI discovery")?,
                            );
                            if self.d.memory.fixture_transport() {
                                if vt == self.d.base + 0x29a4878 {
                                    return Ok(Some(
                                        parent.checked_sub(56).ok_or("Invalid event owner")?,
                                    ));
                                }
                            } else {
                                let matches =
                                    if let Some(matches) = event_types.get(&vt) {
                                        *matches
                                    } else {
                                        let result = crate::gui_observer::Sample::new(
                                            self.d.memory,
                                            self.d.base,
                                            &self.attributes,
                                        )
                                        .rtti(vt)
                                        .is_ok_and(|(name, offset)| {
                                            name == ".?AVCEventWindow@@" && offset <= 512
                                        });
                                        if event_types.len() < 16384 {
                                            event_types.insert(vt, result);
                                        }
                                        result
                                    };
                                if matches {
                                    let (_, offset) = crate::gui_observer::Sample::new(
                                        self.d.memory, self.d.base, &self.attributes,
                                    ).rtti(vt)?;
                                    return Ok(Some(
                                        parent.checked_sub(offset as u64).ok_or("Invalid event owner")?,
                                    ));
                                }
                            }
                        }
                        Ok::<_, String>(None)
                    })();
                    if let Ok(Some(owner)) = candidate {
                        owners.insert(owner);
                    }
                }
            }
        }
        if owners.is_empty() {
            return Err("Finestra eventi non individuata; assenza non qualificata".into());
        }
        let owners = owners.into_iter().collect::<Vec<_>>();
        self.event_cache = Some(json!([root, owners, headers]));
        Ok(owners)
    }
    pub(super) fn events(&mut self) -> Result<Value> {
        let root = self.object_pointer(
            self.d.base
                + self.attributes["gui_layout"]["root_rva"]
                    .as_u64()
                    .unwrap_or(0x34531e0),
            "CGameGui",
        )?;
        let owners = self.discover_windows(root)?;
        let mut visible = vec![];
        for owner in owners {
            self.typed(owner, "CEventWindow")?;
            let container = self.object_pointer(owner + 0x50, "CContainerWindow")?;
            let gui = container + 48;
            if self.pointer(gui + 104)? != root {
                return Err("Finestra eventi di un altro contesto GUI".into());
            }
            if self.pointer(gui + 80)? != owner + 56 {
                return Err("Finestra eventi non associata al proprietario".into());
            }
            let shown = self.track(gui + 117, 1)?[0] & 8 != 0;
            let event = self.pointer(owner + 0x68)?;
            if !shown || event == 0 {
                continue;
            }
            self.typed(event, "CEvent")?;
            let id = self.d.string(event + 0x20)?;
            let definitions = self
                .array(event + 0x368, 256)?
                .into_iter()
                .collect::<HashSet<_>>();
            let grid = self.object_pointer(owner + 0x1280, "CStandardGridBox")?;
            let mut choices = BTreeSet::new();
            let mut indices = HashSet::new();
            for row in self.array(grid + 0x1a0, 256)? {
                self.typed(row, "CStandardGridBoxItem")?;
                let item = self.object_pointer(row + 16, "CContainerWindow")?;
                let buttons = self.array(item + 0x10d8, 64)?;
                if buttons.is_empty() {
                    return Err("Scelta evento priva di pulsante qualificato".into());
                }
                let mut options = HashSet::new();
                for button in buttons {
                    self.typed(button, "CButtonDrag")?;
                    let option = self.object_pointer(button + 8, "CEventOption")?;
                    if !definitions.contains(&option) {
                        return Err("Scelta estranea all’evento visualizzato".into());
                    }
                    options.insert(option);
                }
                if options.len() != 1 {
                    return Err("Riga evento associata a opzioni diverse".into());
                }
                let option = *options.iter().next().unwrap();
                let index = self.integer(option + 0x1f0, 0, 255)?;
                let key = self.d.string(option + 0x178)?;
                if !indices.insert(index) {
                    return Err("Indice scelta evento duplicato".into());
                }
                choices.insert(key);
            }
            visible.push(
                json!({"instance_id":format!("{owner:x}"),"event_id":id,"option_ids":choices}),
            );
        }
        Ok(
            json!({"status":"observed","visible":visible,"reason":"Finestra eventi marcata visibile dal motore; scelte dalla griglia corrente. Nessuna risposta scelta inferita."}),
        )
    }
}
