use crate::util::{Address, BusComponent, MappedComponent, Memory, MemoryError};

pub struct Wram {
    data: Memory<{Self::SIZE}>,
    bank: u8,
}

impl Default for Wram {
    fn default() -> Self {
        Self { data: Memory::new("Work RAM"), bank: 1 }
    }
}

impl BusComponent for Wram {
    fn read_offset(&self, address: impl Into<usize> + Copy) -> Result<u8, MemoryError> {
        self.data.read_offset(address)
    }

    fn write_offset(
        &mut self,
        address: impl Into<usize> + Copy,
        value: u8,
    ) -> Result<(), MemoryError> {
        self.data.write_offset(address, value)
    }
}

impl MappedComponent for Wram {
    fn map(&self, address: Address) -> usize {
        let offset = address.sub(Self::WRAM_START);
        match offset {
            0x0000..=0x0FFF => return offset,
            0x1000..=0x1FFF => return offset + (self.bank - 1) as usize * 0x4000,
            _ => return usize::MAX,
        }
    }
}

impl Wram {

    pub const WRAM_START: usize = 0xC000;
    pub const WRAM_END: usize = 0xDFFF;

    pub const SIZE: usize = Self::WRAM_END - Self::WRAM_START + 1;

}