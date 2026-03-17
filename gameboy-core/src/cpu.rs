use std::fmt::Display;

use crate::bus::{Bus, BusError, InterruptState};
use crate::util::{Address, Width};

use super::Cycles;

mod registers;
pub use self::registers::*;

pub struct CPU {
    pub registers: Registers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Opcode(pub u8);

impl core::fmt::Display for Opcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:02X}", self.0)
    }
}

#[derive(Debug)]
pub enum CycleError {
    Bus(Address, BusError),
    Opcode(Address, Opcode, OpcodeError),
}

impl core::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CycleError::Bus(address, error) => write!(
                f,
                "Could not get interrupt/opcode at memory address {address} (): {error}"
            ),
            CycleError::Opcode(address, opcode, error) => write!(
                f,
                "Could not execute opcode {opcode} at address {address}, {error}"
            ),
        }
    }
}

impl std::error::Error for CycleError {}

#[derive(Debug)]
pub enum OpcodeError {
    Bus(BusError),
    Stop,
}

impl Display for OpcodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpcodeError::Bus(err) => write!(f, "Error while executing opcode: {err}"),
            OpcodeError::Stop => write!(f, "Ran into stop instruction!"),
        }
    }
}

impl From<BusError> for OpcodeError {
    fn from(err: BusError) -> Self {
        Self::Bus(err)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExecutionType {
    Interrupt(Address),
    Halt,
    Opcode(Address),
}

#[derive(Debug, Clone, Copy)]
pub struct CycleExecution {
    pub execution: ExecutionType,
    pub cycles: Cycles,
}

impl CPU {
    pub fn new() -> Self {
        Self {
            registers: Registers::default(),
        }
    }

    pub fn cycle(&mut self, bus: &mut Bus) -> Result<CycleExecution, CycleError> {
        match bus.interrupts.interrupt() {
            InterruptState::Interrupt(address) => match self.op_call(bus, address) {
                Ok(()) => Ok(CycleExecution {
                    execution: ExecutionType::Interrupt(address),
                    cycles: Cycles(20),
                }),
                Err(err) => Err(CycleError::Bus(address, err)),
            },
            InterruptState::Halt => {
                // Handle halt interrupt
                Ok(CycleExecution {
                    execution: ExecutionType::Halt,
                    cycles: Cycles(4),
                })
            }
            InterruptState::None => {
                let address = Address(self.registers[DReg::PC]);
                let opcode = Opcode(
                    self.read_next(bus)
                        .map_err(|e| CycleError::Bus(address, e))?,
                );
                self.execute(bus, opcode)
                    .map(|cycles| CycleExecution {
                        execution: ExecutionType::Opcode(address),
                        cycles,
                    })
                    .map_err(|e| CycleError::Opcode(address, opcode, e))
            }
        }
    }

    fn execute(&mut self, bus: &mut Bus, Opcode(opcode): Opcode) -> Result<Cycles, OpcodeError> {
        let x = opcode >> 6;
        let y = (opcode >> 3) & 7;
        let z = opcode & 7;
        let p = (y >> 1) & 3;
        let q = (y & 1) == 1;
        Ok(match x {
            0 => match z {
                0 => match y {
                    0 => Cycles(4),
                    1 => {
                        let address = Address(self.read_next_u16(bus)?);
                        bus.write_word(address, self.registers[DReg::SP])?;
                        Cycles(20)
                    }
                    2 => {
                        if !bus.cgb.disarm() {
                            return Err(OpcodeError::Stop);
                        }
                        Cycles(4)
                    }
                    3..=7 => Cycles(8 + 4 * self.op_jr_cc_d(y, bus)? as usize),
                    8.. => unreachable!(),
                },
                1 => match q {
                    false => {
                        let value = self.read_next_u16(bus)?;
                        self.registers[DReg::pair1(p)] = value;
                        Cycles(12)
                    }
                    true => self.op_add_hl(self.registers[DReg::pair1(p)]),
                },
                2 => {
                    let address = match p {
                        0 => Address(self.registers[DReg::BC]),
                        1 => Address(self.registers[DReg::DE]),
                        2 => {
                            let reg = &mut self.registers[DReg::HL];
                            let addr = Address(*reg);
                            *reg += 1;
                            addr
                        }
                        3 => {
                            let reg = &mut self.registers[DReg::HL];
                            let addr = Address(*reg);
                            *reg -= 1;
                            addr
                        }
                        4.. => unreachable!(),
                    };
                    let a = &mut self.registers[Reg::A];

                    match q {
                        false => bus.write(address, *a)?,
                        true => *a = bus.read(address)?,
                    };
                    Cycles(8)
                }
                3 => {
                    let r = DReg::pair1(p);
                    self.registers[r] = match q {
                        false => self.registers[r].wrapping_add(1),
                        true => self.registers[r].wrapping_sub(1),
                    };
                    Cycles(8)
                }
                4 => self.op_inc(bus, y)?,
                5 => self.op_dec(bus, y)?,
                6 => {
                    let value = self.read_next(bus)?;
                    self.registers.write_index(bus, y, value)?;
                    Cycles(8 + 4 * (y == 6) as usize)
                }
                7 => {
                    match y {
                        0 => self.registers[Reg::A] = self.op_rlc::<false>(self.registers[Reg::A]),
                        1 => self.registers[Reg::A] = self.op_rrc::<false>(self.registers[Reg::A]),
                        2 => self.registers[Reg::A] = self.op_rl::<false>(self.registers[Reg::A]),
                        3 => self.registers[Reg::A] = self.op_rr::<false>(self.registers[Reg::A]),
                        4 => self.op_daa(),
                        5 => {
                            // CPL
                            self.registers[Reg::A] = !self.registers[Reg::A];
                            self.registers
                                .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, true);
                        }
                        6 => {
                            // SCF
                            self.registers.set_flag(Reg::FLAG_CARRY, true);
                            self.registers
                                .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
                        }
                        7 => {
                            // CCF
                            self.registers
                                .set_flag(Reg::FLAG_CARRY, !self.registers.flag(Reg::FLAG_CARRY));
                            self.registers
                                .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
                        }
                        8.. => unreachable!(),
                    }
                    Cycles(4)
                }
                8.. => unreachable!(),
            },
            1 => match z == 6 && y == 6 {
                true => {
                    bus.interrupts.halt();
                    Cycles(4)
                }
                false => {
                    self.registers
                        .write_index(bus, y, self.registers.read_index(bus, z)?)?;
                    Cycles(4 << (y == 6 || z == 6) as usize)
                }
            },
            2 => {
                let value = self.registers.read_index(bus, z)?;
                self.ops_a_math(y, value);
                Cycles(4 >> (z == 6) as usize)
            }
            3 => match z {
                0 => match y {
                    0..=3 => {
                        let cond = self.subop_cc_flag(y);
                        if cond {
                            self.op_ret(bus)?;
                        }
                        Cycles(8 + 12 * cond as usize)
                    }
                    4 | 6 => {
                        let address = Address(self.read_next(bus)? as Width | 0xFF00);
                        if y == 6 {
                            self.registers[Reg::A] = bus.read(address)?;
                        } else {
                            bus.write(address, self.registers[Reg::A])?;
                        }
                        Cycles(12)
                    }
                    5 => {
                        self.registers[DReg::SP] =
                            self.subop_add_next_signed::<true>(bus, self.registers[DReg::SP])?;
                        Cycles(16)
                    }
                    7 => {
                        self.registers[DReg::HL] =
                            self.subop_add_next_signed::<true>(bus, self.registers[DReg::SP])?;
                        Cycles(12)
                    }
                    8.. => unreachable!(),
                },
                1 => match q {
                    false => {
                        let dreg = DReg::pair2(p);
                        let mut value = self.op_pop(bus)?;
                        if matches!(dreg, DReg::AF) {
                            // ignore four flag bits of AF
                            value = (value & 0xFFF0) | (self.registers[DReg::AF] & 0xF);
                        }
                        self.registers[dreg] = value;
                        Cycles(12)
                    }
                    true => match p {
                        0 => {
                            self.op_ret(bus)?;
                            Cycles(16)
                        }
                        1 => {
                            self.op_ret(bus)?;
                            self.op_ei(bus);
                            Cycles(16)
                        }
                        2 => {
                            self.op_jump(Address(self.registers[DReg::HL]));
                            Cycles(4)
                        }
                        3 => {
                            self.registers[DReg::SP] = self.registers[DReg::HL];
                            Cycles(8)
                        }
                        4.. => unreachable!(),
                    },
                },
                2 => match y {
                    0..=3 => {
                        let cond = self.subop_cc_flag(y);
                        let address = Address(self.read_next_u16(bus)?);
                        if cond {
                            self.op_jump(address);
                        }
                        Cycles(12 + 4 * cond as usize)
                    }
                    4 | 6 => {
                        let address = Address(self.registers[Reg::C] as Width) + 0xFF00;
                        let a = &mut self.registers[Reg::A];
                        if y == 6 {
                            *a = bus.read(address)?;
                        } else {
                            bus.write(address, *a)?;
                        }
                        Cycles(8)
                    }
                    5 | 7 => {
                        let address = Address(self.read_next_u16(bus)?);
                        let a = &mut self.registers[Reg::A];
                        if y == 7 {
                            *a = bus.read(address)?;
                        } else {
                            bus.write(address, *a)?;
                        };
                        Cycles(16)
                    }
                    8.. => unreachable!(),
                },
                3 => match y {
                    0 => {
                        let address = Address(self.read_next_u16(bus)?);
                        self.op_jump(address);
                        Cycles(16)
                    }
                    1 => self.op_cb(bus)?,
                    6 => {
                        self.op_di(bus);
                        Cycles(4)
                    }
                    7 => {
                        self.op_ei(bus);
                        Cycles(4)
                    }
                    2..=5 => unimplemented!(),
                    8.. => unreachable!(),
                },
                4 => match y < 4 {
                    true => {
                        let address = Address(self.read_next_u16(bus)?);
                        let cond = self.subop_cc_flag(y);
                        if cond {
                            self.op_call(bus, address)?;
                        }
                        Cycles(12 + 12 * cond as usize)
                    }
                    false => unimplemented!(),
                },
                5 => match q {
                    true => {
                        assert!(p == 0);
                        let address = Address(self.read_next_u16(bus)?);
                        self.op_call(bus, address)?;
                        Cycles(24)
                    }
                    false => {
                        self.op_push(bus, self.registers[DReg::pair2(p)])?;
                        Cycles(16)
                    }
                },
                6 => {
                    let value = self.read_next(bus)?;
                    self.ops_a_math(y, value);
                    Cycles(8)
                }
                7 => {
                    self.op_call(bus, Address(y as Width * 8))?;
                    Cycles(16)
                }
                8.. => unreachable!(),
            },
            4.. => unreachable!(),
        })
    }

    fn op_jump(&mut self, address: Address) {
        self.registers[DReg::PC] = address.0;
    }

    fn op_push(&mut self, bus: &mut Bus, value: u16) -> Result<(), BusError> {
        self.registers[DReg::SP] = self.registers[DReg::SP]
            .checked_sub(2)
            .ok_or(BusError::Overflow)?;
        bus.write_word(Address(self.registers[DReg::SP]), value)
    }

    fn op_pop(&mut self, bus: &mut Bus) -> Result<u16, BusError> {
        let value = bus.read_word(Address(self.registers[DReg::SP]))?;
        self.registers[DReg::SP] = self.registers[DReg::SP]
            .checked_add(2)
            .ok_or(BusError::Overflow)?;
        Ok(value)
    }

    fn op_call(&mut self, bus: &mut Bus, address: Address) -> Result<(), BusError> {
        self.op_push(bus, self.registers[DReg::PC])?;
        self.op_jump(address);
        Ok(())
    }

    fn op_ret(&mut self, bus: &mut Bus) -> Result<(), BusError> {
        let address = Address(self.op_pop(bus)?);
        self.op_jump(address);
        Ok(())
    }

    fn op_di(&mut self, bus: &mut Bus) {
        bus.interrupts.set_ime(false);
    }

    fn op_ei(&mut self, bus: &mut Bus) {
        bus.interrupts.set_ime(true);
    }

    fn subop_add_next_signed<const FLAGS: bool>(
        &mut self,
        bus: &mut Bus,
        value: u16,
    ) -> Result<u16, BusError> {
        let diff = self.read_next(bus)? as i8 as i16 as u16;
        let result = value.wrapping_add(diff);
        if FLAGS {
            self.registers
                .set_flag(Reg::FLAG_ZERO | Reg::FLAG_NEGATIVE, false);
            self.registers
                .set_flag(Reg::FLAG_CARRY, (value as u8).overflowing_add(diff as u8).1);
            self.registers.set_flag(
                Reg::FLAG_HALF_CARRY,
                Self::half_carry_add_u8(value as u8, diff as u8),
            );
        }
        Ok(result)
        // diff.is_positive()
    }

    #[inline]
    fn subop_cc_flag(&self, y: u8) -> bool {
        match y {
            0 => !self.registers.flag(Reg::FLAG_ZERO),
            1 => self.registers.flag(Reg::FLAG_ZERO),
            2 => !self.registers.flag(Reg::FLAG_CARRY),
            3 => self.registers.flag(Reg::FLAG_CARRY),
            4.. => unreachable!(),
        }
    }

    fn op_jr_cc_d(&mut self, y: u8, bus: &mut Bus) -> Result<bool, BusError> {
        let jump = self.subop_add_next_signed::<false>(bus, self.registers[DReg::PC] + 1)?;
        let cond = y == 3 || self.subop_cc_flag(y - 4);
        if cond {
            self.registers[DReg::PC] = jump;
        }
        Ok(cond)
    }

    fn op_add_hl(&mut self, value: u16) -> Cycles {
        let hl = self.registers[DReg::HL];

        let (result, carry) = hl.overflowing_add(value);

        self.registers.set_flag(Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_CARRY, carry);
        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY, Self::half_carry_add_u16(hl, value));

        self.registers[DReg::HL] = result;
        Cycles(8)
    }

    fn op_inc(&mut self, bus: &mut Bus, index: u8) -> Result<Cycles, BusError> {
        let original = self.registers.read_index(bus, index)?;

        let (value, carry) = original.overflowing_add(1);

        self.registers.set_flag(Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_ZERO, carry);
        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY, Self::half_carry_add_u8(original, 1));

        self.registers.write_index(bus, index, value)?;

        Ok(Cycles(4 + 8 * (index == 6) as usize))
    }

    fn op_dec(&mut self, bus: &mut Bus, index: u8) -> Result<Cycles, BusError> {
        let original = self.registers.read_index(bus, index)?;

        self.registers.set_flag(Reg::FLAG_NEGATIVE, true);
        self.registers.set_flag(Reg::FLAG_ZERO, original == 1);
        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY, Self::half_carry_sub_u8(original, 1));

        self.registers
            .write_index(bus, index, original.wrapping_sub(1))?;

        Ok(Cycles(4 + 8 * (index == 6) as usize))
    }

    fn ops_a_math(&mut self, op: u8, value: u8) {
        match op {
            0 => self.op_add(value),
            1 => self.op_adc(value),
            2 => self.op_sub(value),
            3 => self.op_sbc(value),
            4 => self.op_and(value),
            5 => self.op_xor(value),
            6 => self.op_or(value),
            7 => {
                self.op_cp(self.registers[Reg::A], value);
            }
            8.. => unreachable!(),
        }
    }

    fn op_add(&mut self, value: u8) {
        self.subop_add_flag(self.registers[Reg::A], value);
        let a = &mut self.registers[Reg::A];
        *a = a.wrapping_add(value);
    }

    fn subop_add_flag(&mut self, a: u8, b: u8) -> u8 {
        let (result, overflow) = a.overflowing_add(b);
        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);
        self.registers.set_flag(Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_CARRY, overflow);
        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY, Self::half_carry_add_u8(a, b));
        result
    }

    fn op_adc(&mut self, b: u8) {
        let c = self.registers.flag(Reg::FLAG_CARRY) as u8;
        let a = self.registers[Reg::A];
        let result = a.wrapping_add(b).wrapping_add(c);

        let half_carry = (a & 0x0F) + (b & 0x0F) + c > 0x0F;
        let carry = (a as u16 + b as u16 + c as u16) > 0xFF;

        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);
        self.registers.set_flag(Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_CARRY, carry);
        self.registers.set_flag(Reg::FLAG_HALF_CARRY, half_carry);

        self.registers[Reg::A] = result;
    }

    fn op_sub(&mut self, value: u8) {
        self.op_cp(self.registers[Reg::A], value);
        let a = &mut self.registers[Reg::A];
        *a = a.wrapping_sub(value);
    }

    fn op_sbc(&mut self, b: u8) {
        let c = self.registers.flag(Reg::FLAG_CARRY) as u8;
        let a = self.registers[Reg::A];
        let result = a.wrapping_sub(b).wrapping_sub(c);

        let half_carry = (a & 0x0F) < (b & 0x0F) + c;
        let carry = (a as u16) < b as u16 + c as u16;

        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);
        self.registers.set_flag(Reg::FLAG_NEGATIVE, true);
        self.registers.set_flag(Reg::FLAG_CARRY, carry);
        self.registers.set_flag(Reg::FLAG_HALF_CARRY, half_carry);

        self.registers[Reg::A] = result;
    }

    fn op_and(&mut self, value: u8) {
        self.registers[Reg::A] &= value;
        self.registers
            .set_flag(Reg::FLAG_ZERO, self.registers[Reg::A] == 0);
        self.registers.set_flag(Reg::FLAG_HALF_CARRY, true);
        self.registers
            .set_flag(Reg::FLAG_NEGATIVE | Reg::FLAG_CARRY, false);
    }

    fn op_xor(&mut self, value: u8) {
        self.registers[Reg::A] ^= value;
        self.registers
            .set_flag(Reg::FLAG_ZERO, self.registers[Reg::A] == 0);
        self.registers.set_flag(
            Reg::FLAG_NEGATIVE | Reg::FLAG_CARRY | Reg::FLAG_HALF_CARRY,
            false,
        );
    }

    fn op_or(&mut self, value: u8) {
        self.registers[Reg::A] |= value;
        self.registers
            .set_flag(Reg::FLAG_ZERO, self.registers[Reg::A] == 0);
        self.registers.set_flag(
            Reg::FLAG_NEGATIVE | Reg::FLAG_CARRY | Reg::FLAG_HALF_CARRY,
            false,
        );
    }

    fn op_cp(&mut self, a: u8, b: u8) -> u8 {
        let (result, overflow) = a.overflowing_sub(b);
        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);
        self.registers.set_flag(Reg::FLAG_NEGATIVE, true);
        self.registers.set_flag(Reg::FLAG_CARRY, overflow);
        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY, Self::half_carry_sub_u8(a, b));
        result
    }

    fn op_rl<const ZERO: bool>(&mut self, value: u8) -> u8 {
        let old_flag = self.registers.flag(Reg::FLAG_CARRY) as u8;

        let result = ((value << 1) & !1) | old_flag;

        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
        self.registers
            .set_flag(Reg::FLAG_CARRY, ((value >> 7) & 0x1) == 1);
        self.registers.set_flag(Reg::FLAG_ZERO, ZERO && result == 0);

        return result;
    }

    fn op_rr<const ZERO: bool>(&mut self, value: u8) -> u8 {
        let old_flag = (self.registers.flag(Reg::FLAG_CARRY) as u8) << 7;

        let result = (value >> 1) | old_flag;

        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_CARRY, (value & 0x1) == 1);
        self.registers.set_flag(Reg::FLAG_ZERO, ZERO && result == 0);

        return result;
    }

    fn op_rlc<const ZERO: bool>(&mut self, value: u8) -> u8 {
        let result = ((value << 1) & !1) | ((value >> 7) & 1);

        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
        self.registers
            .set_flag(Reg::FLAG_CARRY, ((value >> 7) & 0x1) == 1);
        self.registers.set_flag(Reg::FLAG_ZERO, ZERO && result == 0);

        result
    }

    fn op_rrc<const ZERO: bool>(&mut self, value: u8) -> u8 {
        let result = ((value >> 1) & !(1 << 7)) | ((value & 1) << 7);

        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_CARRY, (value & 0x1) == 1);
        self.registers.set_flag(Reg::FLAG_ZERO, ZERO && result == 0);

        result
    }

    fn op_sla(&mut self, value: u8) -> u8 {
        let result = (value << 1) & !1; // & (0b01111111);

        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
        self.registers
            .set_flag(Reg::FLAG_CARRY, ((value >> 7) & 0x1) == 1);
        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);

        result
    }

    fn op_sra(&mut self, value: u8) -> u8 {
        let result = (value >> 1) | (value & 0b10000000);

        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_CARRY, (value & 0x1) == 1);
        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);

        result
    }

    fn op_swap(&mut self, value: u8) -> u8 {
        let result = ((value & 0xF0) >> 4) | ((value & 0x0F) << 4);

        self.registers.set_flag(
            Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE | Reg::FLAG_CARRY,
            false,
        );
        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);
        result
    }

    fn op_srl(&mut self, value: u8) -> u8 {
        let result = value >> 1;

        self.registers
            .set_flag(Reg::FLAG_HALF_CARRY | Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_CARRY, (value & 0x1) == 1);
        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);

        result
    }

    fn op_bit(&mut self, value: u8, by: u8) {
        let result = value & (1 << by);

        self.registers.set_flag(Reg::FLAG_NEGATIVE, false);
        self.registers.set_flag(Reg::FLAG_HALF_CARRY, true);
        self.registers.set_flag(Reg::FLAG_ZERO, result == 0);
    }

    pub fn op_cb(&mut self, bus: &mut Bus) -> Result<Cycles, BusError> {
        let cb = self.read_next(bus)?;
        let reg = cb & 7;
        let bits = (cb >> 3) & 7;
        let mut value = self.registers.read_index(bus, reg)?;
        match cb >> 6 {
            0 => match bits {
                0 => value = self.op_rlc::<true>(value),
                1 => value = self.op_rrc::<true>(value),
                2 => value = self.op_rl::<true>(value),
                3 => value = self.op_rr::<true>(value),
                4 => value = self.op_sla(value),
                5 => value = self.op_sra(value),
                6 => value = self.op_swap(value),
                7 => value = self.op_srl(value),
                8.. => unreachable!(),
            },
            1 => self.op_bit(value, bits), // check bit
            2 => value &= !(1 << bits), // reset bit
            3 => value |= 1 << bits,    // set bit
            4.. => unreachable!(),
        }
        self.registers.write_index(bus, reg, value)?;
        Ok(Cycles(8 << (reg == 6) as usize))
    }

    fn read_next(&mut self, bus: &mut Bus) -> Result<u8, BusError> {
        let value = bus.read(Address(self.registers[DReg::PC]))?;
        self.registers[DReg::PC] += 1;
        Ok(value)
    }

    fn read_next_u16(&mut self, bus: &mut Bus) -> Result<u16, BusError> {
        let value = bus.read_word(Address(self.registers[DReg::PC]));
        self.registers[DReg::PC] += 2;
        value
    }

    fn op_daa(&mut self) {
        let mut a = self.registers[Reg::A];
        let mut adjust = if self.registers.flag(Reg::FLAG_CARRY) {
            0x60
        } else {
            0x00
        };
        if self.registers.flag(Reg::FLAG_HALF_CARRY) {
            adjust |= 0x06;
        };
        if !self.registers.flag(Reg::FLAG_NEGATIVE) {
            if a & 0x0F > 0x09 {
                adjust |= 0x06
            };
            if a > 0x99 {
                adjust |= 0x60
            };
            a = a.wrapping_add(adjust);
        } else {
            a = a.wrapping_sub(adjust);
        }

        self.registers.set_flag(Reg::FLAG_CARRY, adjust >= 0x60);
        self.registers.set_flag(Reg::FLAG_HALF_CARRY, false);
        self.registers.set_flag(Reg::FLAG_ZERO, a == 0);
        self.registers[Reg::A] = a;
    }

    pub fn reset(&mut self) {
        *self = CPU::new();
    }

    pub fn pc(&self) -> Address {
        Address(self.registers[DReg::PC])
    }

    const fn half_carry_add_u16(a: u16, b: u16) -> bool {
        ((a & 0xFFF).wrapping_add(b & 0xFFF) & 0x1000) != 0
    }

    const fn half_carry_add_u8(a: u8, b: u8) -> bool {
        ((a & 0xF).wrapping_add(b & 0xF) & 0x10) != 0
    }

    const fn half_carry_sub_u8(a: u8, b: u8) -> bool {
        (a & 0xF).wrapping_sub(b & 0xF) > 0xF
    }

    // const fn half_carry_sub_u16(a: u16, b: u16) -> bool {
    //     (a & 0xFFF).wrapping_sub(b & 0xFFF) > 0xFFF
    // }
}
