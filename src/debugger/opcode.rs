use std::collections::HashMap;

use gameboy_core::bus::Bus;
use gameboy_core::util::{Address, Width};
use gameboy_core::cpu::Opcode;

macro_rules! op {
    ($n:expr) => {
        OpcodeDescriptor {
            length: 1,
            name: $n.to_string(),
            args: String::new(),
        }
    };
    ($n:expr, $l:expr) => {
        OpcodeDescriptor {
            length: $l,
            name: $n.to_string(),
            args: String::new(),
        }
    };
    ($n:expr, $l:expr, $a:expr) => {
        OpcodeDescriptor {
            length: $l,
            name: $n.to_string(),
            args: $a.to_string(),
        }
    };
}

#[derive(Debug)]
pub struct OpcodeDescriptor {
    pub length: Width,
    pub name: String,
    pub args: String,
}

impl OpcodeDescriptor {
    pub fn format(&self, memory: &Bus, mut address: Address) -> String {
        let mut s = String::with_capacity(self.name.len() + 1 + self.args.len());
        s.push_str(&self.name);
        s.push('\t');
        s.push_str(&self.args);
        let mut index = 0;
        address.0 += self.length as u16;
        while let Some(offset) = s.get(index..).map(|s| s.find('%')).flatten() {
            index += offset;
            match s.get(index + 1..index + 2) {
                Some(char) => {
                    let value = match char {
                        "I" => {
                            address.0 -= 1;
                            memory.read(address).map(|val| (val as i8).to_string()).ok()
                        },
                        "S" => {
                            address.0 -= 1;
                            memory.read(address).map(|val| format!("{val:02X}")).ok()
                        },
                        "D" => {
                            address.0 -= 2;
                            memory.read_word(address).map(|val| format!("{val:04X}")).ok()
                        }
                        _ => None,
                    };
                    s.replace_range(index..index + 2, value.as_deref().unwrap_or("??"));
                }
                None => {
                    s.replace_range(index..index + 1, "??");
                }
            }
            index += 2;
        }
        s
    }
}

pub fn generate_table() -> HashMap<Opcode, OpcodeDescriptor> {
    let mut table = HashMap::<u8, OpcodeDescriptor>::default();

    table.insert(0x00, op!("nop"));
    table.insert(0x10, op!("stop"));
    table.insert(0x20, op!("jr", 2, "nz, %I"));
    table.insert(0x30, op!("jr", 2, "nc, %I"));

    table.insert(0x01, op!("ld", 3, "bc, %D"));
    table.insert(0x11, op!("ld", 3, "de, %D"));
    table.insert(0x21, op!("ld", 3, "hl, %D"));
    table.insert(0x31, op!("ld", 3, "sp, %D"));

    table.insert(0x02, op!("ld", 1, "(bc), a"));
    table.insert(0x12, op!("ld", 1, "(de), a"));
    table.insert(0x22, op!("ld", 1, "(hl), a"));
    table.insert(0x32, op!("ld", 1, "(sp), a"));

    table.insert(0x03, op!("inc", 1, "bc"));
    table.insert(0x13, op!("inc", 1, "de"));
    table.insert(0x23, op!("inc", 1, "hl"));
    table.insert(0x33, op!("inc", 1, "sp"));

    table.insert(0x04, op!("inc", 1, "b"));
    table.insert(0x14, op!("inc", 1, "d"));
    table.insert(0x24, op!("inc", 1, "h"));
    table.insert(0x34, op!("inc", 1, "(hl)"));

    table.insert(0x05, op!("dec", 1, "b"));
    table.insert(0x15, op!("dec", 1, "d"));
    table.insert(0x25, op!("dec", 1, "h"));
    table.insert(0x35, op!("dec", 1, "(hl)"));

    table.insert(0x06, op!("ld", 2, "b, %S"));
    table.insert(0x16, op!("ld", 2, "d, %S"));
    table.insert(0x26, op!("ld", 2, "h, %S"));
    table.insert(0x36, op!("ld", 2, "(hl), %S"));

    table.insert(0x07, op!("rlca", 1));
    table.insert(0x17, op!("rla", 1));
    table.insert(0x27, op!("daa", 1));
    table.insert(0x37, op!("scf", 1));

    table.insert(0x08, op!("ld", 3, "%D, (sp)"));
    table.insert(0x18, op!("jr", 2, "%I"));
    table.insert(0x28, op!("jr z", 2, "%I"));
    table.insert(0x38, op!("jr c", 2, "%I"));

    table.insert(0x09, op!("add", 1, "hl, bc"));
    table.insert(0x19, op!("add", 1, "hl, de"));
    table.insert(0x29, op!("add", 1, "hl, hl"));
    table.insert(0x39, op!("add", 1, "hl, sp"));

    table.insert(0x0A, op!("ld", 1, "a, (bc)"));
    table.insert(0x1A, op!("ld", 1, "a, (de)"));
    table.insert(0x2A, op!("ld", 1, "a, (hl+)"));
    table.insert(0x3A, op!("ld", 1, "a, (hl-)"));

    table.insert(0x0B, op!("dec", 1, "bc"));
    table.insert(0x1B, op!("dec", 1, "de"));
    table.insert(0x2B, op!("dec", 1, "hl"));
    table.insert(0x3B, op!("dec", 1, "sp"));

    table.insert(0x0C, op!("inc", 1, "c"));
    table.insert(0x1C, op!("inc", 1, "e"));
    table.insert(0x2C, op!("inc", 1, "l"));
    table.insert(0x3C, op!("inc", 1, "a"));

    table.insert(0x0D, op!("dec", 1, "c"));
    table.insert(0x1D, op!("dec", 1, "e"));
    table.insert(0x2D, op!("dec", 1, "l"));
    table.insert(0x3D, op!("dec", 1, "a"));

    table.insert(0x0E, op!("ld", 2, "c, %S"));
    table.insert(0x1E, op!("ld", 2, "e, %S"));
    table.insert(0x2E, op!("ld", 2, "l, %S"));
    table.insert(0x3E, op!("ld", 2, "a, %S"));

    table.insert(0x0F, op!("rrca", 1));
    table.insert(0x1F, op!("rra", 1));
    table.insert(0x2F, op!("cpl", 1));
    table.insert(0x3F, op!("ccf", 1));

    const LD_REGS: [&'static str; 8] = ["b", "c", "d", "e", "h", "l", "(hl)", "a"];

    for (x, reg_x) in LD_REGS.into_iter().enumerate() {
        for (y, reg_y) in LD_REGS.into_iter().enumerate() {
            if x == 6 && y == 6 {
                table.insert(0x76, op!("halt", 1));
            } else {
                table.insert(
                    (0x40 + x as u8) + (y as u8 * 8),
                    op!("ld", 1, format!("{}, {}", reg_y, reg_x)),
                );
            }
        }
    }

    for (y, op) in ["add", "adc", "sub", "sbc", "and", "xor", "or", "cp"]
        .iter()
        .enumerate()
    {
        for (x, reg) in ["b", "c", "d", "e", "h", "l", "(hl)", "a"]
            .iter()
            .enumerate()
        {
            table.insert(
                (0x80 + x as u8) + (y as u8 * 8),
                op!(op, 1, format!("a, {reg}")),
            );
        }
    }

    
    table.insert(0xC0, op!("ret nz", 1));
    table.insert(0xD0, op!("ret nc", 1));
    table.insert(0xE0, op!("ld", 2, "(FF00 + %S), a"));
    table.insert(0xF0, op!("ld", 2, "a, (FF00 + %S)"));
    
    table.insert(0xC1, op!("pop", 1, "bc"));
    table.insert(0xD1, op!("pop", 1, "de"));
    table.insert(0xE1, op!("pop", 1, "hl"));
    table.insert(0xF1, op!("pop", 1, "af"));

    table.insert(0xC2, op!("jp nz", 3, "%D"));
    table.insert(0xD2, op!("jp nc", 3, "%D"));
    table.insert(0xE2, op!("ld", 2, "(FF00 + C), a"));
    table.insert(0xF2, op!("ld", 2, "a, (FF00 + C)"));

    table.insert(0xC3, op!("jp", 3, "%D"));
    table.insert(0xF3, op!("di", 1));
    
    table.insert(0xC4, op!("call nz", 3, "%D"));
    table.insert(0xD4, op!("call nc", 3, "%D"));

    table.insert(0xC5, op!("push", 1, "bc"));
    table.insert(0xD5, op!("push", 1, "de"));
    table.insert(0xE5, op!("push", 1, "hl"));
    table.insert(0xF5, op!("push", 1, "af"));

    table.insert(0xC6, op!("add", 2, "a, %S"));
    table.insert(0xD6, op!("sub", 2, "a, %S"));
    table.insert(0xE6, op!("and", 2, "a, %S"));
    table.insert(0xF6, op!("or", 2, "a, %S"));
    
    table.insert(0xC7, op!("rst", 1, "0x00"));
    table.insert(0xD7, op!("rst", 1, "0x10"));
    table.insert(0xE7, op!("rst", 1, "0x20"));
    table.insert(0xF7, op!("rst", 1, "0x30"));

    table.insert(0xC8, op!("ret z", 1));
    table.insert(0xD8, op!("ret c", 1));
    table.insert(0xE8, op!("add", 2, "sp, %I"));
    table.insert(0xF8, op!("ld", 2, "hl, sp + %I"));
    
    table.insert(0xC9, op!("ret", 1));
    table.insert(0xD9, op!("reti", 1));
    table.insert(0xE9, op!("jp", 1, "hl"));
    table.insert(0xF9, op!("ld", 1, "sp, hl"));
    
    table.insert(0xCA, op!("jp z", 3, "%D"));
    table.insert(0xDA, op!("jp c", 3, "%D"));
    table.insert(0xEA, op!("ld", 3, "(%D), a"));
    table.insert(0xFA, op!("ld", 3, "a, (%D)"));
    
    table.insert(0xCB, op!("CB", 2, "%S"));
    table.insert(0xFB, op!("ei", 1));
    
    table.insert(0xCC, op!("call z", 3, "%D"));
    table.insert(0xDC, op!("call c", 3, "%D"));
    
    table.insert(0xCD, op!("call", 3, "%D"));

    table.insert(0xCE, op!("adc", 2, "a, %S"));
    table.insert(0xDE, op!("sbc", 2, "a, %S"));
    table.insert(0xEE, op!("xor", 2, "a, %S"));
    table.insert(0xFE, op!("cp", 2, "a, %S"));
        
    table.insert(0xCF, op!("rst", 1, "0x08"));
    table.insert(0xDF, op!("rst", 1, "0x18"));
    table.insert(0xEF, op!("rst", 1, "0x28"));
    table.insert(0xFF, op!("rst", 1, "0x38"));


    table.into_iter().map(|(k, v)| (Opcode(k as u8), v)).collect()
}
