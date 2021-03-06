use crate::{apu, bus, controller as ctrl, cpu, parse, ppu, serialize};
use bus::{CpuAddressBus, CpuAddressBusBase, Mmc3CpuAddressBus, NromCpuAddressBus, PpuAddressBus};

use std::cell::Cell;

// wrapper struct around 'CpuAddressBus' implementations that stores writes
// in the 0x6000 area (blargg's tests output a result string to these addresses)
pub struct TestCpuAddressBus<A> {
    bus: A,
    // the output string (we assume a max size of 256 chars)
    test_output: [u8; 0x100],
    // the test status written to 0x6000
    test_status: Option<u8>,
}

impl<'a> TestCpuAddressBus<NromCpuAddressBus<'a>> {
    pub fn new(
        prg_rom: &[u8],
        chr_ram: &[u8],
        mirroring: parse::MirroringType,
        ppu: ppu::Ppu,
        apu: apu::Apu,
        controller: ctrl::Controller,
        framebuffer: &'a [Cell<u32>; 256 * 240],
    ) -> Self {
        Self {
            bus: NromCpuAddressBus::new(
                prg_rom,
                chr_ram,
                mirroring,
                ppu,
                apu,
                controller,
                framebuffer,
            ),
            test_output: [0; 0x100],
            test_status: None,
        }
    }
}

// // TODO: make the 'new()' method on 'NromAddressBus' and 'Mmc3CpuAddressBus'
// // a part of the 'CpuAddressBus' trait to remove this code duplication
impl<'a> TestCpuAddressBus<Mmc3CpuAddressBus<'a>> {
    pub fn new(
        prg_rom: &[u8],
        chr_ram: &[u8],
        mirroring: parse::MirroringType,
        ppu: ppu::Ppu,
        apu: apu::Apu,
        controller: ctrl::Controller,
        framebuffer: &'a [Cell<u32>; 256 * 240],
    ) -> Self {
        Self {
            bus: Mmc3CpuAddressBus::new(
                prg_rom,
                chr_ram,
                mirroring,
                ppu,
                apu,
                controller,
                framebuffer,
            ),
            test_output: [0; 0x100],
            test_status: None,
        }
    }
}

// 'Serialize' is required by the 'CpuAddressBus' trait
impl<'a, A> serialize::Serialize for TestCpuAddressBus<A>
where
    A: CpuAddressBus<'a>,
{
    fn serialize(&self, file: &mut std::io::BufWriter<std::fs::File>) -> Result<(), String> {
        Ok(())
    }

    fn deserialize(&mut self, file: &mut std::io::BufReader<std::fs::File>) -> Result<(), String> {
        Ok(())
    }
}

impl<'a, A> CpuAddressBus<'a> for TestCpuAddressBus<A>
where
    A: CpuAddressBus<'a>,
{
    fn read(&mut self, addr: u16, cpu: &mut cpu::Cpu) -> u8 {
        if addr >= 0x6004 && addr <= 0x6004 + self.test_output.len() as u16 {
            self.test_output[addr as usize - 0x6004]
        } else if addr == 0x6000 {
            self.test_status.unwrap_or(0xff)
        } else {
            self.bus.read(addr, cpu)
        }
    }

    fn write(&mut self, addr: u16, val: u8, cpu: &mut cpu::Cpu) {
        if addr >= 0x6004 && addr <= 0x6004 + self.test_output.len() as u16 {
            self.test_output[addr as usize - 0x6004] = val;
        } else if addr == 0x6000 {
            self.test_status = Some(val);
        }

        self.bus.write(addr, val, cpu);
    }

    fn base(&mut self) -> (&mut CpuAddressBusBase<'a>, &mut dyn PpuAddressBus) {
        self.bus.base()
    }
}

#[cfg(test)]
fn run_test(rom_path: &str, expected_test_output: &str) {
    let mut rom_file = std::fs::File::open(rom_path).unwrap();
    let framebuffer = Cell::new([0u32; 256 * 240]);
    let mut nes = crate::Nes::new(
        unsafe { &*(&framebuffer as *const _ as *const _) },
        &mut rom_file,
    );

    nes.cpu.pc = u16::from_le_bytes([
        nes.bus.read(0xfffc, &mut nes.cpu),
        nes.bus.read(0xfffd, &mut nes.cpu),
    ]);

    // run nes until test status at 0x6000 indicates that
    // the test results have been written to 0x6004-
    while nes.bus.read(0x6000, &mut nes.cpu) >= 0x80 {
        while !nes.bus.base().0.ppu.is_frame_done() {
            nes.cpu.exec_instruction(nes.bus);
            let (base, ppu_bus) = nes.bus.base();
            let framebuffer = base.framebuffer;
            base.ppu.catch_up(&mut nes.cpu, ppu_bus, framebuffer);
        }

        // reset counters
        let base = nes.bus.base().0;
        base.ppu.sub_cycle_count(nes.cpu.cycle_count as i32 * 3);
        base.ppu.set_frame_done(false);
        nes.cpu.cycle_count = 0;
    }

    let mut test_output = String::new();
    let mut addr = 0x6004;
    loop {
        let character = nes.bus.read(addr, &mut nes.cpu);
        if character == 0 {
            break;
        } else {
            test_output.push(character as char);
            addr += 1;
        }
    }

    assert_eq!(test_output, expected_test_output);
}

#[test]
fn instr_test_v5() {
    run_test(
        "src/test/instr_test-v5/rom_singles/01-basics.nes",
        "\n01-basics\n\nPassed\n",
    );

    run_test(
        "src/test/instr_test-v5/rom_singles/02-implied.nes",
        "\n02-implied\n\nPassed\n",
    );

    // NOTE: tests 03 through 09 fail due to invalid opcodes

    run_test(
        "src/test/instr_test-v5/rom_singles/10-branches.nes",
        "\n10-branches\n\nPassed\n",
    );

    run_test(
        "src/test/instr_test-v5/rom_singles/11-stack.nes",
        "\n11-stack\n\nPassed\n",
    );

    run_test(
        "src/test/instr_test-v5/rom_singles/12-jmp_jsr.nes",
        "\n12-jmp_jsr\n\nPassed\n",
    );

    run_test(
        "src/test/instr_test-v5/rom_singles/13-rts.nes",
        "\n13-rts\n\nPassed\n",
    );

    run_test(
        "src/test/instr_test-v5/rom_singles/14-rti.nes",
        "\n14-rti\n\nPassed\n",
    );

    run_test(
        "src/test/instr_test-v5/rom_singles/15-brk.nes",
        "\n15-brk\n\nPassed\n",
    );

    run_test(
        "src/test/instr_test-v5/rom_singles/16-special.nes",
        "\n16-special\n\nPassed\n",
    );
}

#[test]
fn instr_misc() {
    run_test(
        "src/test/instr_misc/rom_singles/01-abs_x_wrap.nes",
        "\n01-abs_x_wrap\n\nPassed\n",
    );

    run_test(
        "src/test/instr_misc/rom_singles/02-branch_wrap.nes",
        "\n02-branch_wrap\n\nPassed\n",
    );

    run_test(
        "src/test/instr_misc/rom_singles/03-dummy_reads.nes",
        "\n03-dummy_reads\n\nPassed\n",
    );

    // NOTE: '04-dummy_reads_apu' fails since apu emulation isn't implemented yet
}

#[test]
fn instr_timing() {
    // NOTE: both '1-instr_timing' and '2-branch_timing' fail due to lack of apu support
}

#[test]
fn cpu_timing_test6() {
    // NOTE: this test passes, but i've yet to implement a way of verifiying it in code
    // TODO: allow testing roms that don't output results to 0x6000
}

#[test]
fn branch_timing_tests() {
    // NOTE: all of these tests pass, but haven't been automated for the same reason as above
}

#[test]
fn ppu_vbl_nmi() {
    run_test(
        "src/test/ppu_vbl_nmi/rom_singles/01-vbl_basics.nes",
        "\n01-vbl_basics\n\nPassed\n",
    );

    run_test(
        "src/test/ppu_vbl_nmi/rom_singles/02-vbl_set_time.nes",
        "T+ 1 2\n00 - V\n01 - V\n02 - V\n03 - V\n04 - -\n05 V \
         -\n06 V -\n07 V -\n08 V -\n\n02-vbl_set_time\n\nPassed\n",
    );

    run_test(
        "src/test/ppu_vbl_nmi/rom_singles/03-vbl_clear_time.nes",
        "00 V\n01 V\n02 V\n03 V\n04 V\n05 V\n06 -\n07 -\n08 -\n\
         \n03-vbl_clear_time\n\nPassed\n",
    );

    run_test(
        "src/test/ppu_vbl_nmi/rom_singles/04-nmi_control.nes",
        "\n04-nmi_control\n\nPassed\n",
    );

    // NOTE: '05-nmi-timing' fails due to lack of cycle accurate nmi polling

    run_test(
        "src/test/ppu_vbl_nmi/rom_singles/06-suppression.nes",
        "00 - N\n01 - N\n02 - N\n03 - N\n04 - -\n05 V -\n06 V \
         -\n07 V N\n08 V N\n09 V N\n\n06-suppression\n\nPassed\n",
    );

    // NOTE: '07-nmi_on_timing' fails for unknown reasons (many
    // other emulators seem to struggle with this one)

    run_test(
        "src/test/ppu_vbl_nmi/rom_singles/08-nmi_off_timing.nes",
        "03 -\n04 -\n05 -\n06 -\n07 N\n08 N\n09 N\n0A N\n0B N\n0C \
         N\n\n08-nmi_off_timing\n\nPassed\n",
    );

    run_test(
        "src/test/ppu_vbl_nmi/rom_singles/09-even_odd_frames.nes",
        "00 01 01 02 \n09-even_odd_frames\n\nPassed\n",
    );

    // NOTE: '10-even_odd_timing' fails due to cycle inaccuracies (the last
    // 8 cycles of the pre-render line all have to be run in one step)
}

#[test]
fn ppu_sprite_overflow() {
    run_test(
        "src/test/ppu_sprite_overflow/rom_singles/01-basics.nes",
        "\n01-basics\n\nPassed\n",
    );

    run_test(
        "src/test/ppu_sprite_overflow/rom_singles/02-details.nes",
        "\n02-details\n\nPassed\n",
    );

    // NOTE: test 03 fails (for reasons i've yet to investigate)

    run_test(
        "src/test/ppu_sprite_overflow/rom_singles/04-obscure.nes",
        "\n04-obscure\n\nPassed\n",
    );

    // NOTE: test 05 also fails
}

#[test]
fn mmc3_test_2() {
    run_test(
        "src/test/mmc3_test_2/rom_singles/1-clocking.nes",
        "\n1-clocking\n\nPassed\n",
    );

    run_test(
        "src/test/mmc3_test_2/rom_singles/2-details.nes",
        "\n2-details\n\nPassed\n",
    );

    run_test(
        "src/test/mmc3_test_2/rom_singles/3-A12_clocking.nes",
        "\n3-A12_clocking\n\nPassed\n",
    );

    // NOTE: '4-scanline_timing' fails due to cycle inaccuracies (most likely caused by
    // the fact that the 8 cycles at dot 257-265 all have to be executed in one step)

    run_test(
        "src/test/mmc3_test_2/rom_singles/5-MMC3.nes",
        "\n5-MMC3\n\nPassed\n",
    );

    // NOTE: '6-MMC3_alt' fails since only newer MMC3 revisions are supported
    // (both A and B can't be supported at once)
}
