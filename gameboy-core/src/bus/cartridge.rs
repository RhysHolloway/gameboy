use std::borrow::Cow;

use crate::util::{Address, MemoryError};

use super::Bank;

pub struct Cartridge {
    rom: Vec<u8>,
    pub rom_bank: Bank,
    ram: Vec<u8>,
    ram_bank: Bank,
    ram_enabled: bool,
    select: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum CartridgeError {
    Rom(MemoryError, Bank),
    Ram(MemoryError, Bank, bool),
}

impl core::fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CartridgeError::Rom(error, bank) => write!(f, "{error} in cartridge ROM bank {bank:02X}"),
            CartridgeError::Ram(error, bank, enabled) => {
                match enabled {
                    true => write!(f, "{error} in cartridge RAM bank {bank:02X}"),
                    false => write!(f, "{error} in cartridge RAM as is not enabled"),
                }
            }
        }
    }
}

impl Cartridge {

    pub const ROM_BANK_MIN: Address = Address(0x4000);

    pub fn new(rom: Vec<u8>) -> Self {
        // TODO: read 
        Self { rom, rom_bank: 0, ram: vec![0; 0x2000 * 64], ram_bank: 0, ram_enabled: false, select: false }
    }

    pub(super) fn read_rom(&self, address: Address) -> Result<u8, CartridgeError> {
        match address.0 {
            0x0000..=0x3FFF => self.rom.get(address.0 as usize),
            0x4000..=0x7FFF => self.rom.get(address.add(self.rom_bank.saturating_sub(1) as usize * 0x4000)),
            0x8000.. => unreachable!(),
        }.copied().ok_or(CartridgeError::Rom(MemoryError::read("Cartridge ROM", address), self.rom_bank))
    }

    pub(super) fn write_rom(&mut self, address: Address, value: u8) -> Result<(), CartridgeError> {
        match address.0 {
            0x0000..=0x1FFF => {
                self.ram_enabled = value == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank = (value & 0x7F).max(1);
            }
            0x4000..=0x5FFF => {
                if self.select {
                    self.ram_bank = value;
                    // printf("SET RAM BANK TO 0x%02X\n", cart->ram_bank);
                } else {
                    self.rom_bank = (self.rom_bank & 31) | (value << 5);
                    // printf("SET ROM BANK (UPPER) TO 0x%02X\n", cart->rom_bank);
                }
            }
            0x6000..=0x7FFF => {
                if value <= 1 {
                    self.select = value == 1;
                }
            }
            0x8000.. => return Err(CartridgeError::Rom(MemoryError::write("Cartridge ROM", address), self.rom_bank)),
        }
        Ok(())
    }

    fn ram_address(&self, address: Address) -> usize {
        (address - Address(0xA000)).add(self.ram_bank as usize * 0x2000)
    }

    pub(super) fn read_ram(&self, address: Address) -> Result<u8, CartridgeError> {
        self.ram.get(self.ram_address(address)).copied().ok_or(CartridgeError::Ram(MemoryError::read("Cartridge RAM", address), self.ram_bank, self.ram_enabled))
    }

    pub(super) fn write_ram(&mut self, address: Address, value: u8) -> Result<(), CartridgeError> {
        let offset = self.ram_address(address);
        match self.ram.get_mut(offset) {
            Some(ptr) => Ok(*ptr = value),
            None => Err(CartridgeError::Ram(MemoryError::write("Cartridge RAM", address), self.ram_bank, self.ram_enabled))
        }
    }

    pub const fn rom(&self) -> &Vec<u8> {
        &self.rom
    }

    pub const fn rom_bank(&self) -> u8 {
        self.rom_bank
    }

    pub fn title(&self) -> Cow<'_, str> {
        self.rom.get(0x134..0x144).map(String::from_utf8_lossy).unwrap_or(Cow::Borrowed("UNKNOWN"))
    }
}
