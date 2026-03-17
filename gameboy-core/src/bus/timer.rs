use crate::Cycles;
use crate::util::Address;

#[derive(Default)]
pub struct Timer {
    counter: u8,
    div: u8,
    tima: u8,
    tma: u8,
    tac: u8,
    tima_cooldown: u8,
}

impl Timer {

    pub const DIV: u16      = 0xFF04;
    pub const TIMA: u16     = 0xFF05;
    pub const TMA: u16      = 0xFF06;
    pub const TAC: u16      = 0xFF07;

    const INTERRUPT_BIT: u8 = 0b100;

    const TAC_ENABLE_BIT: u8 = 2;

    const TIMA_COOLDOWN_OVERFLOW: u8 = 4;

    pub(super) const fn write(&mut self, address: Address, value: u8) {
        match address.0 {
            Self::DIV => self.div = 0,
            Self::TIMA => self.tima = value,
            Self::TMA => {
                self.tma = value;
                self.tima_cooldown = 0;
            }
            Self::TAC => self.tac = value & 0b111,
            _ => unreachable!(),
        }
    }

    pub(super) const fn read(&self, address: Address) -> u8 {
        match address.0 {
            Self::DIV => self.div,
            Self::TIMA => self.tima,
            Self::TMA => self.tma,
            Self::TAC => self.tac(),
            _ => unreachable!()
        }
    }

    pub fn cycle(&mut self, int: &mut u8, cycles: &Cycles) {
        let t_cycles = cycles.t();

        for _ in 0..t_cycles {
            let (counter, overflow) = self.counter.overflowing_add(1);
            self.counter = counter;
            if !overflow {
                continue;
            }

            let old_bit = self.tima_status();
            self.div = self.div.wrapping_add(1);
            let new_bit = self.tima_status();
            let enabled = (self.tac & Self::TAC_ENABLE_BIT) != 0;

            if self.tima_cooldown != 0 {
                self.tima_cooldown -= 1;
                if self.tima_cooldown == 0 {
                    self.tima = self.tma;
                    *int |= Self::INTERRUPT_BIT;
                }
            } else if enabled & old_bit & !new_bit {
                let (new_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = new_tima;
                if overflow {
                    self.tima_cooldown = Self::TIMA_COOLDOWN_OVERFLOW;
                }
            }
        }
    }

    const fn get_tima_period(&self) -> u16 {
        match self.tac & 0b11 {
            0b00 => 1 << 9,
            0b01 => 1 << 3,
            0b10 => 1 << 5,
            0b11 => 1 << 7,
            _ => unreachable!()
        }
    }

    const fn tima_status(&self) -> bool {
        (self.div as u16 & self.get_tima_period()) != 0
    }

    pub const fn div(&self) -> u8 {
        self.div
    }

    pub const fn tima(&self) -> u8 {
        self.tima
    }

    pub const fn tma(&self) -> u8 {
        self.tma
    }

    pub const fn tac(&self) -> u8 {
        self.tac | 0b11111000
    }


}