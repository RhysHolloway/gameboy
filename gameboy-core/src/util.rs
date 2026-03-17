use std::ops::{Add, Sub};

pub type Width = u16;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address(pub Width);

impl Address {
    pub const fn add(self, offset: usize) -> usize {
        self.0 as usize + offset
    }

    pub const fn sub(self, offset: usize) -> usize {
        self.0 as usize - offset
    }
}

impl Into<usize> for Address {
    fn into(self) -> usize {
        self.0 as usize
    }
}

impl Add<Width> for Address {
    type Output = Address;

    fn add(self, rhs: Width) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Sub for Address {
    type Output = Address;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Sub<Width> for Address {
    type Output = Address;

    fn sub(self, rhs: Width) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl std::fmt::UpperHex for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.0, f)
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#04X}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Controls {
    A = 0, B = 1, Start = 2, Select = 3, Up = 4, Down = 5, Left = 6, Right = 7
}

pub trait BusComponent {
    fn read_offset(&self, address: impl Into<usize> + Copy) -> Result<u8, MemoryError>;
    fn write_offset(&mut self, address: impl Into<usize> + Copy, value: u8) -> Result<(), MemoryError>;
}

#[derive(Clone)]
pub struct Memory<const SIZE: usize> {
    location: &'static str,
    data: Vec<u8>,
}

impl<const SIZE: usize> Memory<SIZE> {
    pub const SIZE: usize = SIZE;

    pub fn new(location: &'static str) -> Self {
        Self {
            location,
            data: vec![0; Self::SIZE],
        }
    }

}

impl<const SIZE: usize> BusComponent for Memory<SIZE> {

    fn read_offset(&self, address: impl Into<usize> + Copy) -> Result<u8, MemoryError> {
        self.data
            .get(address.into())
            .copied()
            .ok_or(MemoryError::read(self.location, address))
    }

    fn write_offset(
        &mut self,
        address: impl Into<usize> + Copy,
        value: u8,
    ) -> Result<(), MemoryError> {
        if let Some(byte) = self.data.get_mut(address.into()) {
            *byte = value;
            Ok(())
        } else {
            Err(MemoryError::write(self.location, address))
        }
    }
}

pub trait MappedComponent: BusComponent {

    fn map(&self, address: Address) -> usize {
        address.into()
    }

    fn read_mapped(&self, address: Address) -> Result<u8, MemoryError> {
        self.read_offset(self.map(address))
    }
    fn write_mapped(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        self.write_offset(self.map(address), value)
    }
}

#[derive(Clone)]
pub struct OffsetMemory<const START: usize, const SIZE: usize>(Memory<SIZE>);

impl<const START: usize, const SIZE: usize> BusComponent for OffsetMemory<START, SIZE> {
    fn read_offset(&self, address: impl Into<usize> + Copy) -> Result<u8, MemoryError> {
        self.0.read_offset(address)
    }

    fn write_offset(
        &mut self,
        address: impl Into<usize> + Copy,
        value: u8,
    ) -> Result<(), MemoryError> {
        self.0.write_offset(address, value)
    }
}

impl<const START: usize, const SIZE: usize> MappedComponent for OffsetMemory<START, SIZE> {
    fn map(&self, address: Address) -> usize {
        address.sub(START)
    }
}

impl<const START: usize, const SIZE: usize> OffsetMemory<START, SIZE> {
    pub const START: usize = START;
    pub const END: usize = START + (SIZE) - 1;

    pub fn new(location: &'static str) -> Self {
        Self(Memory::new(location))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryError {
    location: &'static str,
    address: usize,
    write: bool,
}

impl MemoryError {

    pub fn read(location: &'static str, address: impl Into<usize>) -> Self {
        Self {
            location: location,
            address: address.into(),
            write: false,
        }
    }

    pub fn write(location: &'static str, address: impl Into<usize>) -> Self {
        Self {
            location: location,
            address: address.into(),
            write: true,
        }
    }
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = if self.write { "write to" } else { "read from" };
        let args = format_args!("Could not {} address {:#04X}", mode, self.address);
        write!(f, "{} in {}", args, self.location)
    }
}
