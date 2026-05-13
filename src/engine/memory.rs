use anyhow::{bail, Result};

pub trait MemoryRead {
    fn read_u8(&self, address: usize) -> Result<u8>;
    fn read_u16(&self, address: usize) -> Result<u16>;
    fn read_i16(&self, address: usize) -> Result<i16>;
    fn read_u32(&self, address: usize) -> Result<u32>;
    fn read_i32(&self, address: usize) -> Result<i32>;
}

pub trait MemoryWrite {
    fn write_u32(&self, address: usize, value: u32) -> Result<()>;
}

pub fn checked_offset(base: usize, offset: isize) -> Result<usize> {
    if offset >= 0 {
        base.checked_add(offset as usize)
    } else {
        base.checked_sub(offset.unsigned_abs())
    }
    .ok_or_else(|| anyhow::anyhow!("address offset overflow: base={base:#x}, offset={offset:#x}"))
}

pub fn ensure_exact_read(expected: usize, actual: usize, address: usize) -> Result<()> {
    if expected == actual {
        return Ok(());
    }

    bail!("short memory read at {address:#x}: expected {expected} bytes, got {actual}")
}
