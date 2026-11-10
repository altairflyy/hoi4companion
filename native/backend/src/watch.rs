//! Bounded, page-local watch packets. Large gaps stay sparse; small gaps are
//! read but never compared.
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
    let mut groups = Vec::new();
    for parts in pages.values() {
        let mut group = Vec::new();
        let mut end = 0;
        for &(address, value) in parts {
            // Widely separated fields on the same page need not drag all
            // intervening bytes into every change probe (large empires).
            if !group.is_empty() && address > end + 128 {
                groups.push(std::mem::take(&mut group));
            }
            end = end.max(address + value.len() as u64);
            group.push((address, value));
        }
        if !group.is_empty() { groups.push(group); }
    }
    for parts in &groups {
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
        ranges: groups.len(),
        read_bytes,
    })
}
/// Verification keeps sparse fields sparse instead of reading page-sized gaps.
pub fn verification(witnesses: &Witnesses) -> Result<Packet> {
    let mut bytes = vec![];
    let mut read_bytes = 0;
    let mut ranges = 0;
    let mut start = 0;
    let mut end = 0;
    let mut parts: Vec<(u64, &[u8])> = vec![];
    let flush = |start: u64,
                 end: u64,
                 parts: &mut Vec<(u64, &[u8])>,
                 bytes: &mut Vec<u8>,
                 read_bytes: &mut usize,
                 ranges: &mut usize|
     -> Result<()> {
        if parts.is_empty() {
            return Ok(());
        }
        let size = (end - start) as usize;
        *read_bytes += size;
        if *read_bytes > 4 * 1024 * 1024 {
            return Err("Full sample exceeds verification read limit".into());
        }
        let offsets: Vec<_> = parts
            .iter()
            .map(|(address, value)| ((*address - start) as u32, *value))
            .collect();
        append(bytes, start, size, &offsets)?;
        *ranges += 1;
        parts.clear();
        Ok(())
    };
    for (&(address, size), value) in witnesses {
        if value.len() != size {
            return Err("Invalid native witness length".into());
        }
        if size == 0 {
            continue;
        }
        let next = address
            .checked_add(size as u64)
            .ok_or("Invalid witness address")?;
        if !parts.is_empty() && (address > end + 32 || next - start > 65536) {
            flush(
                start,
                end,
                &mut parts,
                &mut bytes,
                &mut read_bytes,
                &mut ranges,
            )?;
        }
        if parts.is_empty() {
            start = address;
            end = next
        } else {
            end = end.max(next)
        }
        parts.push((address, value));
    }
    flush(
        start,
        end,
        &mut parts,
        &mut bytes,
        &mut read_bytes,
        &mut ranges,
    )?;
    Ok(Packet {
        bytes,
        ranges,
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
        assert_eq!(compile(&ranges).unwrap().read_bytes, 1026);
        // Sparse optimization must not relax the real byte-read ceiling.
        let dense = (0..513).map(|i| ((0x40000000 + i * 4096, 4096), vec![1;4096])).collect();
        assert!(compile(&dense).is_err());
    }
    #[test]
    fn coalesced_verification_keeps_every_original_byte_witness() {
        let ranges = Witnesses::from([
            ((0x20000000, 4), b"abcd".to_vec()),
            ((0x20000010, 4), b"efgh".to_vec()),
        ]);
        let packet = verification(&ranges).unwrap();
        assert_eq!(packet.ranges, 1);
        assert_eq!(packet.read_bytes, 20);
        assert!(packet.bytes.windows(4).any(|b| b == b"abcd"));
        assert!(packet.bytes.windows(4).any(|b| b == b"efgh"));
    }
    #[test]
    fn full_verification_does_not_expand_scattered_fields_to_pages() {
        let mut fields = Witnesses::new();
        for i in 0..20000 {
            fields.insert((0x30000000 + i * 4096, 4), vec![1; 4]);
            fields.insert((0x30000000 + i * 4096 + 4000, 4), vec![2; 4]);
        }
        let probe = compile(&fields).unwrap();
        assert_eq!(probe.read_bytes, 160000);
        assert_eq!(probe.ranges, 40000);
        let packet = verification(&fields).unwrap();
        assert_eq!(packet.read_bytes, 160000);
        assert_eq!(packet.ranges, 40000);
    }
}
