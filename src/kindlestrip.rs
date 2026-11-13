//! Removes the SRCS sections kindlegen appends to a MOBI file: a zipped copy
//! of the book's sources that the Kindle never reads and that can make up
//! most of the file.
//!
//! Port of kindlestrip.py 1.35 by Paul Durrant and Kevin Hendricks (public
//! domain), by way of the C# port in ComicToMobi 1.x.

const MALFORMED: &str = "malformed MOBI file";

// PalmDB file header.
const PALMDB_TYPE: usize = 0x3C;
const UNIQUE_ID_SEED: usize = 68;
const SECTION_COUNT: usize = 76;
/// One entry per section: 4-byte file offset, 4-byte flags and unique id.
const SECTION_TABLE: usize = 78;
const SECTION_ENTRY: usize = 8;

// MOBI header, relative to the start of section 0.
const MOBI_LENGTH: usize = 0x14;
const EXTH_FLAGS: usize = 0x80;
const HAS_EXTH: u32 = 0x40;
const SRCS_INDEX: usize = 0xE0;
const SRCS_COUNT: usize = 0xE4;
const NO_SECTION: u32 = u32::MAX;

/// The MOBI header proper starts after the 16-byte PalmDOC header.
const PALMDOC_LENGTH: usize = 16;
const EXTH_RECORDS: usize = 12;
/// EXTH record holding the section number of the KF8 boundary.
const EXTH_KF8_BOUNDARY: u32 = 121;

/// A stripped file, as the parts to write out in order. Only the small front
/// part is rebuilt; the section data stays borrowed from the input.
pub struct Stripped<'a> {
    /// PalmDB header, section table and the patched section 0.
    pub head: Vec<u8>,
    /// The sections before and after the removed ones.
    pub body: [&'a [u8]; 2],
}

/// Returns the file without its SRCS sections, or `None` if it has none.
pub fn strip(data: &[u8]) -> Result<Option<Stripped<'_>>, &'static str> {
    if data.get(PALMDB_TYPE..PALMDB_TYPE + 8) != Some(b"BOOKMOBI") {
        return Err("not a MOBI file");
    }
    let section_count = usize::from(be16(data, SECTION_COUNT)?);
    let section_offset = |index: usize| {
        be32(data, SECTION_TABLE + index * SECTION_ENTRY).map(|offset| offset as usize)
    };
    let header = section_offset(0)?;
    let srcs_index = be32(data, header + SRCS_INDEX)?;
    let srcs_count = be32(data, header + SRCS_COUNT)?;
    if srcs_index == NO_SECTION || srcs_count == 0 {
        return Ok(None);
    }

    let (first, removed) = (srcs_index as usize, srcs_count as usize);
    let after = first
        .checked_add(removed)
        .filter(|&after| first > 0 && after <= section_count)
        .ok_or(MALFORMED)?;
    let srcs_start = section_offset(first)?;
    let srcs_end = if after == section_count {
        data.len()
    } else {
        section_offset(after)?
    };
    let srcs = data.get(srcs_start..srcs_end).ok_or(MALFORMED)?;
    if !srcs.starts_with(b"SRCS") {
        return Err(MALFORMED);
    }
    let header_end = section_offset(1)?;
    let section0 = data.get(header..header_end).ok_or(MALFORMED)?;
    let before = data.get(header_end..srcs_start).ok_or(MALFORMED)?;
    let kept = section_count - removed;

    // The table loses one entry per removed section, so section 0 moves up.
    let table_shrink = removed * SECTION_ENTRY;
    let new_header = header.checked_sub(table_shrink).ok_or(MALFORMED)?;
    let mut head = Vec::with_capacity(new_header + section0.len());
    head.extend_from_slice(&data[..UNIQUE_ID_SEED]);
    // Undocumented, but always 2 * section count + 1: the next free unique id.
    head.extend_from_slice(&(kept as u32 * 2 + 1).to_be_bytes());
    head.extend_from_slice(&data[UNIQUE_ID_SEED + 4..SECTION_COUNT]);
    head.extend_from_slice(&(kept as u16).to_be_bytes());

    for index in 0..first {
        let entry = SECTION_TABLE + index * SECTION_ENTRY;
        let offset = section_offset(index)?
            .checked_sub(table_shrink)
            .ok_or(MALFORMED)?;
        head.extend_from_slice(&(offset as u32).to_be_bytes());
        head.extend_from_slice(
            data.get(entry + 4..entry + SECTION_ENTRY)
                .ok_or(MALFORMED)?,
        );
    }
    // Sections after SRCS also move up by its data, and get new unique ids.
    let shift = table_shrink + srcs.len();
    for index in after..section_count {
        let offset = section_offset(index)?.checked_sub(shift).ok_or(MALFORMED)?;
        head.extend_from_slice(&(offset as u32).to_be_bytes());
        head.extend_from_slice(&((index - removed) as u32 * 2).to_be_bytes());
    }

    if new_header < head.len() {
        return Err(MALFORMED);
    }
    head.resize(new_header, 0);
    head.extend_from_slice(section0);
    put_be32(&mut head, new_header + SRCS_INDEX, NO_SECTION)?;
    put_be32(&mut head, new_header + SRCS_COUNT, 0)?;
    // A missing or damaged EXTH block just means there is no boundary to move.
    let _ = shift_kf8_boundary(&mut head, new_header, srcs_index, srcs_count);

    Ok(Some(Stripped {
        head,
        body: [before, &data[srcs_end..]],
    }))
}

/// In a combined MOBI7 + KF8 file, EXTH record 121 points at the KF8 boundary
/// section, which has moved if it came after the removed SRCS sections.
fn shift_kf8_boundary(
    head: &mut [u8],
    header: usize,
    srcs_index: u32,
    srcs_count: u32,
) -> Result<(), &'static str> {
    let exth = header + PALMDOC_LENGTH + be32(head, header + MOBI_LENGTH)? as usize;
    if be32(head, header + EXTH_FLAGS)? & HAS_EXTH == 0 || head.get(exth..exth + 4) != Some(b"EXTH")
    {
        return Ok(());
    }
    let record_count = be32(head, exth + 8)?;
    let mut record = exth + EXTH_RECORDS;
    for _ in 0..record_count {
        let kind = be32(head, record)?;
        let size = be32(head, record + 4)? as usize;
        if kind == EXTH_KF8_BOUNDARY {
            let boundary = be32(head, record + 8)?;
            if srcs_index <= boundary {
                put_be32(
                    head,
                    record + 8,
                    boundary.checked_sub(srcs_count).ok_or(MALFORMED)?,
                )?;
            }
        }
        record += size;
    }
    Ok(())
}

fn be16(data: &[u8], at: usize) -> Result<u16, &'static str> {
    let bytes = data.get(at..at + 2).ok_or(MALFORMED)?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn be32(data: &[u8], at: usize) -> Result<u32, &'static str> {
    let bytes = data.get(at..at + 4).ok_or(MALFORMED)?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn put_be32(data: &mut [u8], at: usize, value: u32) -> Result<(), &'static str> {
    data.get_mut(at..at + 4)
        .ok_or(MALFORMED)?
        .copy_from_slice(&value.to_be_bytes());
    Ok(())
}
