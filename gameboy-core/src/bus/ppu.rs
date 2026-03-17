use crate::Cycles;
use crate::util::{Address, BusComponent, MappedComponent, MemoryError, OffsetMemory};

pub struct Ppu {
    clock: u16,
    vram: OffsetMemory<0x8000, 0x2000>,
    pub voam: OffsetMemory<0xFE00, 0xA0>,
    framebuffer: [u8; Self::SCREEN_WIDTH * Self::SCREEN_HEIGHT],
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    pub ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    //
    window_line: u8,
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            clock: 0,
            vram: OffsetMemory::new("Video RAM"),
            voam: OffsetMemory::new("Video OAM"),
            framebuffer: [0x00; Self::SCREEN_WIDTH * Self::SCREEN_HEIGHT],
            lcdc: 0x91,
            stat: 0x85,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            window_line: 0,
        }
    }
}

impl Ppu {
    pub const SCREEN_WIDTH: usize = 160;
    pub const SCREEN_HEIGHT: usize = 144;

    pub const ADDRESS_LCDC: Address = Address(0xFF40);
    pub const ADDRESS_STAT: Address = Address(0xFF41); // lcd status memory location
    pub const ADDRESS_SCY: Address = Address(0xFF42);
    pub const ADDRESS_SCX: Address = Address(0xFF43);
    pub const ADDRESS_LY: Address = Address(0xFF44);
    pub const ADDRESS_LYC: Address = Address(0xFF45);
    pub const ADDRESS_BGP: Address = Address(0xFF47);
    pub const ADDRESS_OBJP1: Address = Address(0xFF48);
    pub const ADDRESS_OBJP2: Address = Address(0xFF49);
    pub const ADDRESS_WY: Address = Address(0xFF4A);
    pub const ADDRESS_WX: Address = Address(0xFF4B);

    pub const ADDRESS_VBK: Address = Address(0xFF4F); // CGB only, VRAM bank select

    // interrupt flag bits
    pub const INTERRUPT_VBLANK: u8 = 1 << 0;
    /**
     * Interrupt on mode 0, 1, 2, or LY=LYC match, depending on STAT settings
     * (Also called STAT)
     */
    pub const INTERRUPT_LCD: u8 = 1 << 1;

    pub const HBLANK: u8 = 0;
    pub const VBLANK: u8 = 1;
    pub const OAM: u8 = 2;
    pub const TRANSFER: u8 = 3;

    // stat bits
    pub const STAT_LY_COMPARE: u8 = 1 << 2;
    /**
     * Interrupt on switch to mode 0 (HBlank)
     */
    pub const STAT_MODE_0_SELECT: u8 = 1 << 3;
    /**
     * Interrupt on switch to mode 1 (VBlank)
     */
    pub const STAT_MODE_1_SELECT: u8 = 1 << 4;
    /**
     * Interrupt on switch to mode 2 (OAM)
     */
    pub const STAT_MODE_2_SELECT: u8 = 1 << 5;
    /**
     * Interrupt on LY=LYC match
     */
    pub const STAT_LYC_SELECT: u8 = 1 << 6;

    //
    pub const LCDC_BG: u8 = 1;
    pub const LCDC_OBJ: u8 = 1 << 1;
    pub const LCDC_OBJ_SIZE: u8 = 1 << 2;

    pub fn read_vram(&self, address: Address) -> Result<u8, MemoryError> {
        self.vram.read_mapped(address)
    }

    pub fn write_vram(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        self.vram.write_mapped(address, value)
    }

    pub fn cycle(&mut self, int: &mut u8, cycles: &Cycles) -> Result<bool, MemoryError> {
        let mut render = false;
        for _ in 0..cycles.m() {
            self.clock += 1;
            match self.stat & 0b11 {
                Self::HBLANK => {
                    if self.clock >= 204 {
                        self.clock = 0;
                        self.update_ly(int, self.ly().wrapping_add(1));
                        if self.ly() == Self::SCREEN_HEIGHT as u8 {
                            self.set_mode(int, Self::VBLANK)?;
                            render = true;
                        } else {
                            self.set_mode(int, Self::OAM)?;
                        }
                    }
                }
                Self::VBLANK => {
                    if self.clock >= 456 {
                        self.clock = 0;
                        self.update_ly(int, self.ly().wrapping_add(1));
                        if self.ly() >= Self::SCREEN_HEIGHT as u8 + 10 {
                            self.update_ly(int, 0);
                            self.set_mode(int, Self::OAM)?;
                        }
                    }
                }
                Self::OAM => {
                    if self.clock >= 80 {
                        self.clock = 0;
                        self.set_mode(int, Self::TRANSFER)?;
                    }
                }
                Self::TRANSFER => {
                    if self.clock >= 172 {
                        if self.ly() < Self::SCREEN_HEIGHT as u8 {
                            self.render_scanline()?;
                        }
                        self.clock = 0;
                        self.set_mode(int, Self::HBLANK)?;
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(render)
    }

    pub fn read_reg(&self, address: Address) -> Result<u8, MemoryError> {
        Ok(match address {
            Self::ADDRESS_LCDC => self.lcdc,
            Self::ADDRESS_STAT => self.stat,
            Self::ADDRESS_SCY => self.scy,
            Self::ADDRESS_SCX => self.scx,
            Self::ADDRESS_LY => self.ly(),
            Self::ADDRESS_LYC => self.lyc,
            Self::ADDRESS_BGP => self.bgp,
            Self::ADDRESS_OBJP1 => self.obp0,
            Self::ADDRESS_OBJP2 => self.obp1,
            Self::ADDRESS_WY => self.wy,
            Self::ADDRESS_WX => self.wx,
            _ => {
                return Err(MemoryError::read("PPU Register", address));
            }
        })
    }

    pub const fn ly(&self) -> u8 {
        self.ly
    }

    pub fn write_reg(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        match address {
            Self::ADDRESS_LCDC => self.lcdc = value,
            Self::ADDRESS_STAT => self.stat = (self.stat & 0x07) | (value & 0x78),
            Self::ADDRESS_SCY => self.scy = value,
            Self::ADDRESS_SCX => self.scx = value,
            Self::ADDRESS_LYC => self.lyc = value,
            Self::ADDRESS_BGP => self.bgp = value,
            Self::ADDRESS_OBJP1 => self.obp0 = value & 0xFC,
            Self::ADDRESS_OBJP2 => self.obp1 = value & 0xFC,
            Self::ADDRESS_WY => self.wy = value,
            Self::ADDRESS_WX => self.wx = value,
            _ => {
                return Err(MemoryError::write("PPU Register", address));
            }
        }
        Ok(())
    }

    fn update_ly(&mut self, int: &mut u8, value: u8) {
        self.ly = value;
        if self.ly() == self.lyc {
            self.stat |= Self::STAT_LY_COMPARE;
            if self.stat & Self::STAT_LYC_SELECT != 0 {
                *int |= Self::INTERRUPT_LCD;
            }
        } else {
            self.stat &= !Self::STAT_LY_COMPARE;
        }
    }

    fn set_mode(&mut self, int: &mut u8, mode: u8) -> Result<(), MemoryError> {
        self.stat &= 0xF8;
        self.stat |= mode;
        let mut interrupt = false;
        match mode {
            Self::HBLANK => {
                interrupt = self.stat & Self::STAT_MODE_0_SELECT != 0;
            }
            Self::VBLANK => {
                *int |= Self::INTERRUPT_VBLANK;
                interrupt = self.stat & Self::STAT_MODE_1_SELECT != 0;
            }
            Self::OAM => interrupt = self.stat & Self::STAT_MODE_2_SELECT != 0,
            _ => (),
        }
        if interrupt {
            *int |= Self::INTERRUPT_LCD;
        }
        Ok(())
    }

    fn render_scanline(&mut self) -> Result<(), MemoryError> {
        #[derive(Clone, Copy, Debug)]
        struct Sprite {
            x: i16,
            y: i16,
            tile: u8,
            attrs: u8,
            oam_index: usize,
        }

        let y = self.ly() as u16;
        let bg_enable = self.lcdc & 0x01 != 0;
        let sprite_enable = self.lcdc & 0x02 != 0;
        let sprite_size = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        let bg_map_base = if self.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };
        let tile_data_base = if self.lcdc & 0x10 != 0 {
            0x0000
        } else {
            0x1000
        };
        let window_enable = self.lcdc & 0x20 != 0;
        let window_map_base = if self.lcdc & 0x40 != 0 {
            0x1C00
        } else {
            0x1800
        };

        let mut sprites: [Sprite; 10] = [Sprite {
            x: 0,
            y: 0,
            tile: 0,
            attrs: 0,
            oam_index: 0,
        }; 10];
        let mut sprite_count = 0;
        if sprite_enable {
            for i in 0..40 {
                let base = i * 4;
                let sy = self.voam.read_offset(base)? as i16 - 16;
                let sx = self.voam.read_offset(base + 1)? as i16 - 8;
                let tile = self.voam.read_offset(base + 2)?;
                let attrs = self.voam.read_offset(base + 3)?;
                if y as i16 >= sy && (y as i16) < sy + sprite_size && sprite_count < 10 {
                    sprites[sprite_count] = Sprite {
                        x: sx,
                        y: sy,
                        tile,
                        attrs,
                        oam_index: i,
                    };
                    sprite_count += 1;
                }
            }
            sprites[..sprite_count].sort_by_key(|s| (s.x, s.oam_index as i16));
        }

        let mut window_drawn = false;
        for x in 0..Self::SCREEN_WIDTH as u16 {
            let mut bg_color_id = 0u8;
            if bg_enable {
                let mut use_window = false;
                let wx = self.wx.wrapping_sub(7);
                if window_enable && y >= self.wy as u16 && x >= wx as u16 && self.wx <= 166 {
                    use_window = true;
                    window_drawn = true;
                }

                let (map_base, pixel_x, pixel_y) = if use_window {
                    let wx = self.wx.wrapping_sub(7) as u16;
                    let px = x - wx;
                    let py = (self.window_line as u16).wrapping_sub(0);
                    (window_map_base, px, py)
                } else {
                    let px = x.wrapping_add(self.scx as u16);
                    let py = y.wrapping_add(self.scy as u16);
                    (bg_map_base, px, py)
                };

                let tile_x = (pixel_x / 8) & 31;
                let tile_y = (pixel_y / 8) & 31;
                let tile_index_addr = map_base + tile_y * 32 + tile_x;
                let tile_index = self.vram.read_offset(tile_index_addr as usize)?;
                let tile_addr = if self.lcdc & 0x10 != 0 {
                    tile_data_base + (tile_index as u16) * 16
                } else {
                    let signed = tile_index as i8 as i16;
                    (0x1000i16 + signed * 16) as u16
                };
                let line = pixel_y % 8;
                let bit = 7 - (pixel_x % 8);
                let lo = self.vram.read_offset((tile_addr + line * 2) as usize)?;
                let hi = self.vram.read_offset((tile_addr + line * 2 + 1) as usize)?;
                bg_color_id = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
            }

            let mut final_color = self.palette_color(self.bgp, bg_color_id);
            if sprite_enable && sprite_count > 0 {
                let mut sprite_pixel: Option<(u8, u8, bool)> = None;
                for sprite in sprites.iter().take(sprite_count) {
                    if x as i16 >= sprite.x && (x as i16) < sprite.x + 8 {
                        let mut sprite_y = y as i16 - sprite.y;
                        if sprite.attrs & 0x40 != 0 {
                            sprite_y = (sprite_size - 1) - sprite_y;
                        }
                        let mut sprite_x = x as i16 - sprite.x;
                        if sprite.attrs & 0x20 != 0 {
                            sprite_x = 7 - sprite_x;
                        }
                        let mut tile = sprite.tile;
                        if sprite_size == 16 {
                            tile &= 0xFE;
                            if sprite_y >= 8 {
                                tile = tile.wrapping_add(1);
                                sprite_y -= 8;
                            }
                        }
                        let tile_addr = (tile as u16) * 16;
                        let line = sprite_y as u16;
                        let bit = 7 - (sprite_x as u16);
                        let lo = self.vram.read_offset((tile_addr + line * 2) as usize)?;
                        let hi = self.vram.read_offset((tile_addr + line * 2 + 1) as usize)?;
                        let color_id = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
                        if color_id != 0 {
                            let palette = if sprite.attrs & 0x10 != 0 {
                                self.obp1
                            } else {
                                self.obp0
                            };
                            let priority = sprite.attrs & 0x80 != 0;
                            sprite_pixel = Some((color_id, palette, priority));
                            break;
                        }
                    }
                }
                if let Some((color_id, palette, priority)) = sprite_pixel {
                    if !priority || bg_color_id == 0 {
                        final_color = self.palette_color(palette, color_id);
                    }
                }
            }
            self.framebuffer[self.ly() as usize * Self::SCREEN_WIDTH + x as usize] = final_color;
        }

        if window_drawn {
            self.window_line = self.window_line.wrapping_add(1);
        }

        Ok(())
    }

    const fn palette_color(&self, palette: u8, color_id: u8) -> u8 {
        let shift = (color_id & 0x03) * 2;
        (palette >> shift) & 0x03
    }

    pub const fn clock(&self) -> u16 {
        self.clock
    }

    pub const fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub const fn lcdc(&self) -> u8 {
        self.lcdc
    }

    pub const fn stat(&self) -> u8 {
        self.stat & (1 >> 7)
    }
}
