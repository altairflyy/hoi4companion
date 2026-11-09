//! Immutable, bounded PE64/RTTI access shared by profile and live readers.
type Result<T> = std::result::Result<T, String>;
pub(crate) struct Pe<'a> {
    data: &'a [u8],
    pub(crate) base: u64,
    pub(crate) sections: Vec<(u64, u64, usize)>,
}
fn raw(data: &[u8], offset: usize, n: usize) -> Result<&[u8]> {
    data.get(offset..offset.checked_add(n).ok_or("Riferimento PE non mappato")?)
        .ok_or_else(|| "Riferimento PE non mappato".into())
}
pub(crate) fn number(data: &[u8], offset: usize, n: usize) -> Result<u64> {
    let mut bytes = [0; 8];
    bytes[..n].copy_from_slice(raw(data, offset, n)?);
    Ok(u64::from_le_bytes(bytes))
}
impl<'a> Pe<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Result<Self> {
        let pe = number(data, 0x3c, 4)? as usize;
        if raw(data, pe, 4)? != b"PE\0\0"
            || number(data, pe + 4, 2)? != 0x8664
            || number(data, pe + 24, 2)? != 0x20b
        {
            return Err("Eseguibile PE64 non valido".into());
        }
        let count = number(data, pe + 6, 2)? as usize;
        let opt = number(data, pe + 20, 2)? as usize;
        if count == 0 || count > 96 {
            return Err("Sezioni PE non valide".into());
        }
        let base = number(data, pe + 48, 8)?;
        let mut sections = vec![];
        for i in 0..count {
            let offset = pe + 24 + opt + 40 * i;
            sections.push((
                number(data, offset + 12, 4)?,
                number(data, offset + 16, 4)?,
                number(data, offset + 20, 4)? as usize,
            ));
        }
        Ok(Self {
            data,
            base,
            sections,
        })
    }
    pub(crate) fn bytes(&self, rva: u64, size: usize) -> Result<&[u8]> {
        for &(start, length, offset) in &self.sections {
            if start <= rva
                && rva
                    .checked_add(size as u64)
                    .is_some_and(|end| end <= start + length)
            {
                return raw(self.data, offset + (rva - start) as usize, size);
            }
        }
        Err("Riferimento PE non mappato".into())
    }
    pub(crate) fn class_name(&self, rva: u64) -> Result<String> {
        let location = number(
            self.bytes(rva.checked_sub(8).ok_or("RTTI non riconosciuto")?, 8)?,
            0,
            8,
        )?
        .checked_sub(self.base)
        .ok_or("RTTI non riconosciuto")?;
        let header = self.bytes(location, 24)?;
        if number(header, 0, 4)? != 1
            || number(header, 4, 4)? != 0
            || number(header, 20, 4)? != location
        {
            return Err("RTTI non riconosciuto".into());
        }
        let descriptor = number(header, 12, 4)?;
        let bytes = self.bytes(descriptor + 16, 128)?;
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        if !bytes[..end].is_ascii() {
            return Err("RTTI non ASCII".into());
        }
        String::from_utf8(bytes[..end].to_vec()).map_err(|e| e.to_string())
    }
}
