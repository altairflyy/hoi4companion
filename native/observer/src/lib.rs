//! Bounded revalidation of an already qualified, borrowed read-only handle.
//! No offsets, process discovery, write APIs, game calls, or persistent cache.
use std::ffi::c_void;

const MAX_PACKET: usize = 8 * 1024 * 1024;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_RANGES: usize = 131_072;
const LIMIT: u64 = 0x7fff_ffff_ffff;
const INVALID: i32 = -1;
const UNREADABLE: i32 = -2;

struct Part<'a> {
    offset: usize,
    expected: &'a [u8],
    mask: u8,
}
struct Range<'a> {
    address: u64,
    size: usize,
    parts: Vec<Part<'a>>,
}

fn number(data: &[u8], cursor: &mut usize, size: usize) -> Result<u64, i32> {
    let end = cursor.checked_add(size).ok_or(INVALID)?;
    let bytes = data.get(*cursor..end).ok_or(INVALID)?;
    *cursor = end;
    let mut value = [0u8; 8];
    value[..size].copy_from_slice(bytes);
    Ok(u64::from_le_bytes(value))
}

fn decode(data: &[u8]) -> Result<Vec<Range<'_>>, i32> {
    if data.len() > MAX_PACKET {
        return Err(INVALID);
    }
    let (mut cursor, mut total, mut total_parts) = (0, 0usize, 0usize);
    let mut ranges = Vec::new();
    while cursor < data.len() {
        let address = number(data, &mut cursor, 8)?;
        let size = number(data, &mut cursor, 4)? as usize;
        let count = number(data, &mut cursor, 4)? as usize;
        total = total.checked_add(size).ok_or(INVALID)?;
        total_parts = total_parts.checked_add(count).ok_or(INVALID)?;
        if !(0x10000..LIMIT).contains(&address)
            || address.checked_add(size as u64).is_none_or(|v| v > LIMIT)
            || size == 0
            || size > 65536
            || count == 0
            || count > MAX_RANGES
            || total > MAX_BYTES
            || total_parts > MAX_RANGES
            || ranges.len() >= MAX_RANGES
        {
            return Err(INVALID);
        }
        let mut parts = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            let offset = number(data, &mut cursor, 4)? as usize;
            let length = number(data, &mut cursor, 4)? as usize;
            let mask = number(data, &mut cursor, 4)?;
            if length == 0
                || offset.checked_add(length).is_none_or(|v| v > size)
                || mask == 0
                || mask > 255
                || (mask != 255 && length != 1)
            {
                return Err(INVALID);
            }
            let end = cursor.checked_add(length).ok_or(INVALID)?;
            let expected = data.get(cursor..end).ok_or(INVALID)?;
            cursor = end;
            parts.push(Part {
                offset,
                expected,
                mask: mask as u8,
            });
        }
        ranges.push(Range {
            address,
            size,
            parts,
        });
    }
    Ok(ranges)
}

fn compare(
    data: &[u8],
    passes: u32,
    mut read: impl FnMut(u64, &mut [u8]) -> bool,
) -> Result<bool, i32> {
    if !(1..=2).contains(&passes) {
        return Err(INVALID);
    }
    // Validate the COMPLETE packet before touching memory, even if an early
    // range would differ. Only selected bytes (not watch-range gaps) count.
    let ranges = decode(data)?;
    let mut buffer = vec![0; ranges.iter().map(|r| r.size).max().unwrap_or(0)];
    for _ in 0..passes {
        for range in &ranges {
            let bytes = &mut buffer[..range.size];
            if !read(range.address, bytes) {
                return Err(UNREADABLE);
            }
            for part in &range.parts {
                let actual = &bytes[part.offset..part.offset + part.expected.len()];
                let equal = if part.mask == 255 {
                    actual == part.expected
                } else {
                    actual[0] & part.mask == part.expected[0] & part.mask
                };
                if !equal {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn ReadProcessMemory(
        handle: *mut c_void,
        address: *const c_void,
        buffer: *mut c_void,
        size: usize,
        count: *mut usize,
    ) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn hoi4_observer_abi_version() -> u32 {
    1
}

/// Returns 0 unchanged, 1 changed, -1 invalid, -2 unreadable, -3 panic.
/// # Safety
/// `data` addresses `length` readable bytes for this call. Handle is borrowed;
/// its identity/rights/lifetime remain the caller's responsibility. Never freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_observer_compare(
    handle: *mut c_void,
    data: *const u8,
    length: usize,
    passes: u32,
) -> i32 {
    if handle.is_null() || length > MAX_PACKET || (data.is_null() && length != 0) {
        return INVALID;
    }
    std::panic::catch_unwind(|| {
        let packet = if length == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(data, length) }
        };
        let result = compare(packet, passes, |address, output| {
            #[cfg(windows)]
            {
                let mut count = 0;
                unsafe {
                    ReadProcessMemory(
                        handle,
                        address as *const c_void,
                        output.as_mut_ptr().cast(),
                        output.len(),
                        &mut count,
                    ) != 0
                        && count == output.len()
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (address, output);
                false
            }
        });
        match result {
            Ok(changed) => i32::from(changed),
            Err(error) => error,
        }
    })
    .unwrap_or(-3)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(mask: u32) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend(0x10000u64.to_le_bytes());
        for value in [8u32, 1, 2, 1, mask] {
            p.extend(value.to_le_bytes());
        }
        p.push(8);
        p
    }
    #[test]
    fn selected_bytes_and_mask() {
        let p = packet(8);
        assert_eq!(
            compare(&p, 2, |_, b| {
                b.fill(255);
                b[2] = 24;
                true
            }),
            Ok(false)
        );
        assert_eq!(
            compare(&p, 1, |_, b| {
                b.fill(0);
                true
            }),
            Ok(true)
        );
    }
    #[test]
    fn validates_all_ranges_before_reads() {
        let mut p = packet(255);
        p.push(0);
        assert_eq!(compare(&p, 1, |_, _| panic!("must not read")), Err(INVALID));
    }
    #[test]
    fn short_read_is_not_a_change() {
        assert_eq!(compare(&packet(255), 1, |_, _| false), Err(UNREADABLE));
    }
    #[test]
    fn second_pass_can_detect_change() {
        let mut calls = 0;
        assert_eq!(
            compare(&packet(255), 2, |_, b| {
                calls += 1;
                b.fill(0);
                b[2] = if calls == 1 { 8 } else { 9 };
                true
            }),
            Ok(true)
        );
        assert_eq!(calls, 2);
    }
    #[test]
    fn malformed_limits_and_masks() {
        assert!(decode(&packet(0)).is_err());
        assert!(decode(&packet(256)).is_err());
        let mut p = packet(8);
        p[8..12].copy_from_slice(&65537u32.to_le_bytes());
        assert!(decode(&p).is_err());
        assert_eq!(compare(&[], 3, |_, _| true), Err(INVALID));
        assert_eq!(compare(&[], 1, |_, _| panic!("empty")), Ok(false));
    }
}
