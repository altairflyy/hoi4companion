//! Save-stable campaign identity from the witnessed UUID and unique seed.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
impl<M: Memory> Reader<'_, M> {
    pub(super) fn campaign(&mut self) -> Result<Value> {
        let signature = self.attributes["campaign_signature"]
            .as_str()
            .unwrap_or("4c8d86280700004533c9ba803b0000488bcb");
        if !signature.len().is_multiple_of(2) || signature.len() > 131072 {
            return Err("Firma identità campagna non riconosciuta".into());
        }
        let expected = (0..signature.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&signature[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>>>()?;
        let rva = self.attributes["campaign_signature_rva"]
            .as_u64()
            .unwrap_or(0x1f2f2d);
        if self.d.memory.executable_bytes(rva, expected.len())? != expected {
            return Err("Firma identità campagna non riconosciuta".into());
        }
        let token = self.attributes["campaign_token_rva"]
            .as_u64()
            .unwrap_or(0x3423158);
        if u32::from_le_bytes(self.d.read(self.d.base + token, 4)?.try_into().unwrap()) != 0x3b80 {
            return Err("Token identità campagna non qualificato".into());
        }
        let seed = u32::from_le_bytes(self.d.read(self.world + 0x720, 4)?.try_into().unwrap());
        let id = self.d.string(self.world + 0x728)?.to_ascii_lowercase();
        if id.len() != 36
            || id.bytes().enumerate().any(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b != b'-'
                } else {
                    !b.is_ascii_hexdigit()
                }
            })
            || id.bytes().all(|b| b == b'0' || b == b'-')
        {
            return Err("UUID campagna non valido".into());
        }
        Ok(
            json!({"status":"observed","id":format!("hoi4:{id}:{seed:08x}"),"source":"native_game_unique_id_and_seed"}),
        )
    }
}
