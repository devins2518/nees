#[macro_use]
mod util;
mod apu;
mod cpu;
mod memory_map;
mod parse;
mod ppu;
mod win;

use memory_map as mmap;
use mmap::{CpuMemoryMap, PpuMemoryMap};

use pixel_renderer;
use pixel_renderer::PixelRenderer;

fn main() {
    let rom = std::fs::read("Super_Mario_Bros.nes").unwrap();
    assert!(parse::is_valid(&rom));
    logln!("{}", std::str::from_utf8(&rom[0..=3]).unwrap());
    logln!("is nes 2.0: {}", parse::is_nes_2_format(&rom));
    logln!("has trainer: {}", parse::has_trainer(&rom));
    logln!("mirroring type: {:?}", parse::get_mirroring_type(&rom));
    logln!("mapper number: {}", parse::get_mapper_num(&rom));
    logln!("prg rom size: {}KB", parse::get_prg_size(&rom) as u32 * 16);
    logln!("chr rom size: {}KB", parse::get_chr_size(&rom) as u32 * 8);
    logln!(
        "has battery-backed RAM: {}",
        parse::has_persistent_mem(&rom)
    );

    match (parse::get_mapper_num(&rom), parse::get_prg_size(&rom)) {
        (0, 1) => {
            // nrom-128 (maybe?)
        }
        (0, 2) => {
            // nrom-256
        }
        _ => (),
    }

    let nestest = false;
    if nestest {
        let mut win = win::XcbWindowWrapper::new("mynes", 1200, 600).unwrap();
        let renderer = PixelRenderer::new(&mut win.connection, win.win, 256, 240).unwrap();
        let ppu_memory = mmap::Nrom128PpuMemory::new();
        let ppu = ppu::Ppu::new(renderer, ppu_memory);
        let apu = apu::Apu {};
        let mut cpu = cpu::Cpu::new_nestest();
        let mut cpu_memory = mmap::Nrom128CpuMemory::new(ppu, apu);

        let prg_size = 0x4000 * (parse::get_prg_size(&rom) as usize);
        cpu_memory.load_prg_rom(&rom[0x10..=prg_size + 0xf]);

        loop {
            cpu.log_register_values();
            cpu.exec_instruction(&mut cpu_memory);
        }
    }
}
