use crate::util::Address;

pub struct Interrupts {
    pub i: u8,
    pub ie: u8,
    ime: bool,
    halt: bool,
}

impl Default for Interrupts {
    fn default() -> Self {
        Self {
            i: 0,
            ie: 0,
            ime: Default::default(),
            halt: false,
        }
    }
}

// struct ImeState {
//     current: bool,
//     next: (u8, bool),
// }

// impl Default for ImeState {
//     fn default() -> Self {
//         Self {
//             current: true,
//             next: Default::default(),
//         }
//     }
// }

impl Interrupts {

    pub fn interrupt(&mut self) -> InterruptState {
        if self.halt {
            if self.i & self.ie != 0 {
                self.halt = false;
            }
            return InterruptState::Halt;
        } else if self.ime() {
            let bits = self.ie & self.i;
            for bit in 0..5 {
                if bits & (1 << bit) != 0 {
                    self.halt = false;
                    self.i &= !(1 << bit);
                    return InterruptState::Interrupt(Address(0x40 + 8 * bit));
                }
            }
        }
        InterruptState::None
    }
    
    pub const fn halt(&mut self) {
        self.halt = true;
    }
    
    pub(crate) const fn set_ime(&mut self, value: bool) {
        self.ime = value;
    }

    pub const fn is_halting(&self) -> bool {
        self.halt
    }

    pub const fn ie(&self) -> u8 {
        self.ie
    }

    pub const fn ime(&self) -> bool {
        self.ime
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InterruptState {
    Interrupt(Address),
    Halt,
    None,
}
