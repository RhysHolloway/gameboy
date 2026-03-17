use crate::util::MemoryError;

#[derive(Default)]
pub struct Cgb {
    key1: u8,
}

impl Cgb {

    pub fn read_mapped(&self, address: impl Into<usize> + Copy) -> Result<u8, MemoryError> {
        match address.into() {
            0xFF4D => Ok(self.key1),
            _ => Err(MemoryError::read("CGB", address)),
        }
    }

    pub fn write_mapped(&mut self, address: impl Into<usize> + Copy, value: u8) -> Result<(), MemoryError> {
        match address.into() {
            0xFF4D => self.key1 = value & 1,
            _ => return Err(MemoryError::write("CGB", address)),
        }
        Ok(())
    }

    pub fn speed(&self) -> u8 {
        (self.key1 & 1) + 1
    }

    pub fn disarm(&mut self) -> bool {
        if self.key1 & 1 == 1 {
            self.key1 &= !1;
            true
        } else {
            false
        }

    }

}