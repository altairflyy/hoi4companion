//! Bounded, page-local watch packets. Gaps are read but never compared.
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;
pub type Witnesses = BTreeMap<(u64, usize), Vec<u8>>;
pub struct Packet {
    pub bytes: Vec<u8>,
    pub ranges: usize,
    pub read_bytes: usize,
}
fn append(output: &mut Vec<u8>, address: u64, size: usize, parts: &[(u32, &[u8])]) -> Result<()> {
    if !(0x10000..0x7fff_ffff_ffff).contains(&address)
        || size == 0
        || size > 65536
        || address
            .checked_add(size as u64)
            .is_none_or(|v| v > 0x7fff_ffff_ffff)
        || parts.is_empty()
    {
        return Err("Invalid native observation range".into());
    }
    output.extend(address.to_le_bytes());
    output.extend((size as u32).to_le_bytes());
    output.extend((parts.len() as u32).to_le_bytes());
    for (offset, expected) in parts {
        if expected.is_empty() || *offset as usize + expected.len() > size {
            return Err("Invalid native observation part".into());
        }
        output.extend(offset.to_le_bytes());
        output.extend((expected.len() as u32).to_le_bytes());
        output.extend(255u32.to_le_bytes());
        output.extend(*expected);
        if output.len() > 8 * 1024 * 1024 {
            return Err("Native observation exceeds limit".into());
        }
    }
    Ok(())
}
pub fn sample(witnesses: &Witnesses) -> Result<Packet> {
    let mut bytes = vec![];
    let mut read_bytes = 0;
    for ((address, size), value) in witnesses {
        if value.len() != *size {
            return Err("Invalid native witness length".into());
        }
        append(&mut bytes, *address, *size, &[(0, value)])?;
        read_bytes += size;
    }
    Ok(Packet {
        bytes,
        ranges: witnesses.len(),
        read_bytes,
    })
}
pub fn compile(witnesses: &Witnesses) -> Result<Packet> {
    let mut pages = BTreeMap::<u64, Vec<(u64, &[u8])>>::new();
    for ((address, size), value) in witnesses {
        if value.len() != *size {
            return Err("Invalid native witness length".into());
        }
        let mut offset = 0;
        while offset < *size {
            let current = address
                .checked_add(offset as u64)
                .ok_or("Invalid watch address")?;
            let page = current & !4095;
            let n = (size - offset).min(4096 - (current - page) as usize);
            pages
                .entry(page)
                .or_default()
                .push((current, &value[offset..offset + n]));
            offset += n;
        }
    }
    let mut bytes = vec![];
    let mut read_bytes = 0;
    for parts in pages.values() {
        let start = parts.iter().map(|(address, _)| *address).min().unwrap();
        let end = parts
            .iter()
            .map(|(address, b)| *address + b.len() as u64)
            .max()
            .unwrap();
        let size = (end - start) as usize;
        read_bytes += size;
        if read_bytes * 2 > 4 * 1024 * 1024 {
            return Err("Limite del controllo leggero raggiunto".into());
        }
        append(
            &mut bytes,
            start,
            size,
            &parts
                .iter()
                .map(|(address, b)| ((*address - start) as u32, *b))
                .collect::<Vec<_>>(),
        )?;
    }
    Ok(Packet {
        bytes,
        ranges: pages.len(),
        read_bytes,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_edges_gaps_and_expansion_are_bounded() {
        let ranges = Witnesses::from([((0x20000ffe, 4), b"abcd".to_vec())]);
        let packet = compile(&ranges).unwrap();
        assert_eq!(packet.ranges, 2);
        assert_eq!(packet.read_bytes, 4);
        let mut ranges = Witnesses::new();
        for i in 0..513 {
            let address = 0x30000000 + i * 4096;
            ranges.insert((address, 1), vec![1]);
            ranges.insert((address + 4095, 1), vec![2]);
        }
        assert!(compile(&ranges).is_err());
    }
}
