use crate::util::Controls;

#[derive(Default)]
enum Select {
    #[default]
    None,
    DPad,
    Buttons,
}


#[derive(Default)]
pub struct Joypad {
    select: Select,
    state: u8,
}

impl Joypad {

    pub const INTERRUPT_BIT: u8 = 0x10;

    pub const fn read(&self) -> u8 {
        match self.select {
            Select::None => 0xF,
            Select::DPad => self.state >> 4,
            Select::Buttons => self.state & 0xF,
        }
    }

    pub const fn write(&mut self, value: u8) {
        self.select = match (value & 0x30) >> 4 {
            1 => Select::DPad,
            2 => Select::Buttons, 
            _ => Select::None,
        };
    }

    pub const fn update(&mut self, int: &mut u8, (control, down): (Controls, bool)) {
        let bit = 1u8 << control as u8;
        let up = !down;
        self.state = (self.state & !(bit)) | ((up as u8) << control as u8);
        if up {
            *int |= Self::INTERRUPT_BIT;
        }
    }

}