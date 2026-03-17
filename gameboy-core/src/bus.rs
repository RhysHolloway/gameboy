mod cartridge;
mod wram;
mod joypad;
mod ppu;
mod timer;
mod interrupts;
mod serial;
mod dma;
mod cgb;

pub use interrupts::*;
use crate::util::{Address, BusComponent, Controls, MappedComponent, MemoryError, OffsetMemory};

use self::{cartridge::Cartridge};

pub struct Bus {
    // bios: Bios,
    pub cartridge: Cartridge,
    wram: wram::Wram,
    pub ppu: ppu::Ppu,
    pub timer: timer::Timer,
    hram: OffsetMemory<0xFF80, 0x7F>,
    joypad: joypad::Joypad,
    pub dma: dma::Dma,
    pub serial: serial::SerialState,
    pub interrupts: interrupts::Interrupts,
    pub cgb: cgb::Cgb,
}

pub type Bank = u8;

impl Bus {

    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            cartridge: Cartridge::new(rom),
            hram: OffsetMemory::new("High RAM"),
            wram: wram::Wram::default(),
            interrupts: Default::default(),
            ppu: ppu::Ppu::default(),
            timer: timer::Timer::default(),
            serial: serial::SerialState::default(),
            joypad: joypad::Joypad::default(),
            dma: dma::Dma::default(),
            cgb: cgb::Cgb::default(),
        }
    }

    pub fn read(&self, address: Address) -> Result<u8, BusError> {
        Ok(match address.0 {
            0x0000 ..= 0x7FFF => self.cartridge.read_rom(address)?,
            0x8000..=0x9FFF => self.ppu.read_vram(address)?, // video ram
            0xA000..=0xBFFF => self.cartridge.read_ram(address)?,
            0xC000..=0xDFFF => self.wram.read_mapped(address)?, // wram
            // 0xFF40..=0xFF4B => self.ppu.read_reg(address),
            0xFF00 => self.joypad.read(),
            0xFF01 => self.serial.read(address)?,
            0xFF04..=0xFF07 => self.timer.read(address),
            0xFF0F => self.interrupts.i,
            0xFF10..=0xFF26 => 0xFF, // self.audio.read(address)?,
            0xFF38..=0xFF3F => 0xFF, // self.audio.read(address)?,
            0xFF40..0xFF46 | 0xFF47..=0xFF4B => self.ppu.read_reg(address)?,
            0xFF46 => self.dma.read(),
            0xFF4D => self.cgb.read_mapped(address)?,
            0xFF80..=0xFFFE => self.hram.read_mapped(address)?,
            0xFFFF => self.interrupts.ie,
            _ => return Err(BusError::Memory(MemoryError::read("Inaccessible", address))),
        })
    }
        
    pub fn read_dma(&self, address: Address) -> Option<u8> {
        match address.0 {
            0xE000..=0xFDFF => self.wram.read_offset(address - 0xE000).ok(),
            0xFE00..=0xFE9F => self.ppu.voam.read_mapped(address).ok(),
            _ => self.read(address).ok(),
        }
    }

    pub fn write(&mut self, address: Address, value: u8) -> Result<(), BusError> {
        if self.dma.is_active() && address.0 < 0xFF80 {
            return Ok(()); // During DMA, only allow writes to HRAM
        }
        match address.0 {
            0x0000 ..=0x7FFF => self.cartridge.write_rom(address, value)?,
            0x8000..=0x9FFF => self.ppu.write_vram(address, value)?,
            0xA000..=0xBFFF => self.cartridge.write_ram(address, value)?,
            0xC000..=0xDFFF => self.wram.write_mapped(address, value)?,
            0xFE00..=0xFE9F => self.ppu.voam.write_mapped(address, value)?,
            0xFF00 => self.joypad.write(value),
            0xFF01 | 0xFF02 => self.serial.write(address, value)?,
            0xFF04..=0xFF07 => self.timer.write(address, value),
            0xFF10..=0xFF26 => (), // self.audio.write(address, value)?,
            0xFF30..=0xFF3F => (), // self.audio.write(address, value)?,
            0xFF0F => self.interrupts.i = value & 0x1F,
            0xFF40..0xFF46 | 0xFF47..=0xFF4B => self.ppu.write_reg(address, value)?,
            0xFF46 => self.dma.write(value), // OAM DMA
            0xFF4D => self.cgb.write_mapped(address, value)?,
            // 0xFF00..=0xFF7F => self.io.write(address - 0xFF00, value).map_err(From::from),
            0xFF80..=0xFFFE => self.hram.write_mapped(address, value)?,
            0xFFFF => self.interrupts.ie = value,
            _ => return Err(BusError::Memory(MemoryError::write("Inaccessible", address))),
        }
        Ok(())
    }

    pub fn read_word(&self, address: Address) -> Result<u16, BusError> {
        Ok(u16::from_le_bytes([self.read(address)?, self.read(address + 1)?]))
    }

    pub fn write_word(&mut self, address: Address, value: u16) -> Result<(), BusError> {
        let bytes = u16::to_le_bytes(value);
        self.write(address, bytes[0])?;
        self.write(address + 1, bytes[1])?;
        Ok(())
    }

    pub(super) fn cycle(&mut self, cycles: &super::Cycles) -> Result<bool, BusError> {
        let slow_cycles = cycles / self.cgb.speed(); 
        self.timer.cycle(&mut self.interrupts.i, cycles);
        let render = self.ppu.cycle(&mut self.interrupts.i, &slow_cycles)?;
        dma::Dma::cycle(&slow_cycles, self)?;
        self.serial.cycle(&mut self.interrupts.i, cycles);
        Ok(render)
    }

    pub fn reset(&mut self) {
        let rom: Vec<u8> = self.cartridge.rom().clone();
        *self = Self::new(rom);
    }

    pub fn update_input(&mut self, input: (Controls, bool)) {
        self.joypad.update(&mut self.interrupts.i, input);
    }

}

#[derive(Debug, Clone, Copy)]
pub enum BusError {
    Memory(MemoryError),
    Cartridge(cartridge::CartridgeError),
    Overflow,
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BusError::Memory(e) => std::fmt::Display::fmt(e, f),
            BusError::Cartridge(e) => std::fmt::Display::fmt(e, f),
            BusError::Overflow => write!(f, "Numerical overflow"),
        }
    }
}

impl From<cartridge::CartridgeError> for BusError {
    fn from(value: cartridge::CartridgeError) -> Self {
        Self::Cartridge(value)
    }
}

impl From<MemoryError> for BusError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}