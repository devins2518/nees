mod nrom;

pub use nrom::Nrom128MemoryMap;

use super::{apu, ppu};

// trait to represent operations on the cpu and ppu memory maps/
// address spaces. allows implementing custom memory read/write
// behavior for the various 'mappers' used by nes games/cartridges
pub trait MemoryMap {
    fn read_cpu(&self, ptrs: &mut MemoryMapPtrs, addr: u16) -> u8;
    fn write_cpu(&mut self, ptrs: &mut MemoryMapPtrs, addr: u16, val: u8);
    fn read_ppu(&self, addr: u16) -> u8;
    fn write_ppu(&mut self, addr: u16, val: u8);
    // TODO: default methods for loading into rom/ram/etc.
}

// helper struct for passing other pointers to the memory map read/write functions
pub struct MemoryMapPtrs<'a, 'b> {
    pub ppu: &'a mut ppu::Ppu,
    pub apu: &'b mut apu::Apu,
}
