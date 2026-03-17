use crate::bus::Bus;
use crate::Cycles;
use crate::util::{Address, BusComponent, MemoryError};

#[derive(Clone, Copy, Debug, Default)]
pub struct Dma {
    active: bool,
    value: u8,
    index: u16,
    cycle_counter: u8,
}

impl Dma {

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub(super) fn read(&self) -> u8 {
        self.value
    }

    pub(super) fn write(&mut self, value: u8) {
        self.value = value;
        self.index = 0;
        self.cycle_counter = 0;
        self.active = true;
    }

    pub(super) fn cycle(cycles: &Cycles, bus: &mut Bus) -> Result<(), MemoryError> {
        let mut this = std::mem::take(&mut bus.dma);
        if !this.active {
            return Ok(());
        }
        for _ in 0..cycles.t() {
            this.cycle_counter += 1;
            if this.cycle_counter >= 4 {
                this.cycle_counter = 0;
                let address = Address(((this.value as u16) << 8) | this.index);
                let value = bus.read_dma(address).unwrap_or(0xFF);
                bus.ppu.voam.write_offset(this.index as usize, value)?;
                this.index += 1;
                if this.index >= 160 {
                    this.active = false;
                    break;
                }
            }
        }
        bus.dma = this;
        Ok(())
    }
}