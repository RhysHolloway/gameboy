use crate::Cycles;
use crate::util::{Address, MemoryError};

#[derive(Clone, Debug)]
pub struct SerialState {
    sb: u8,
    sc: u8,
    active: bool,
    cycles: u16,
    pub output: Vec<u8>,
}

impl Default for SerialState {
    fn default() -> Self {
        Self {
            sb: 0x00,
            sc: 0x7E,
            active: false,
            cycles: 0,
            output: Vec::new(),
        }
    }
}

impl SerialState {

    pub const LOCATION: &'static str = "Serial IO";
    pub const INTERRUPT_BIT: u8 = 0x08;

    pub fn read(&self, address: Address) -> Result<u8, MemoryError> {
        match address.0 {
            0xFF01 => Ok(self.sb),
            0xFF02 => Ok(self.sc | 0x7E),
            _ => Err(MemoryError::read(Self::LOCATION, address)),
        }
    }

    pub fn write(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        match address.0 {
            0xFF01 => self.sb = value,
            0xFF02 => {
                self.sc = value & 0x83;
                if self.sc & 0x81 == 0x81 {
                    self.active = true;
                    self.cycles = 0;
                }
            }
            _ => return Err(MemoryError::write(Self::LOCATION, address)),
        }
        Ok(())
    }

    pub fn cycle(&mut self, int: &mut u8, cycles: &Cycles) {
        if self.active {
            self.cycles = self.cycles.saturating_add(cycles.t() as u16);
            if self.cycles >= 4096 {
                self.active = false;
                self.sc &= 0x7F;
                self.output.push(self.sb);
                *int |= Self::INTERRUPT_BIT;
            }
        }
    }
}