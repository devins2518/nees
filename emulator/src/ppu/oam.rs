use super::super::memory_map::PpuMemoryMap;

#[derive(Default)]
pub struct Oam {
    pub primary: PrimaryOam,
    secondary: SecondaryOam,
    // contains data for the 8 sprites to be drawn on the current or
    // next scanline (is filled with sprite data for the next scanline
    // on cycles 257-320)
    current_sprites_data: [SpriteRenderData; 8],
    // during sprite evaluation (dots 65-256 of each visible scanline),
    // this is used as the index of the current sprite to be evaluated
    // in primary oam. during sprite fetching (dots 257-320 of each
    // visible scanline) it is used as an index into secondary oam to
    // get the current sprite to fetch data for
    pub current_sprite: u8,
    // number of sprites found on the next scanline
    pub sprites_found: u8,
}

#[derive(Copy, Clone, Default)]
#[repr(C)]
pub struct OamEntry {
    pub y: u8,
    pub tile_index: u8,
    pub attributes: u8,
    pub x: u8,
}

pub struct PrimaryOam {
    pub entries: [OamEntry; 64],
}

pub struct SecondaryOam {
    pub entries: [OamEntry; 8],
}

// struct used internally in 'Oam' to store sprite data between scanlines
#[derive(Copy, Clone, Default)]
struct SpriteRenderData {
    x: u8,
    tile_bitplane_lo: u8,
    tile_bitplane_hi: u8,
    // bit 2 of 'attributes' indicates end of array, bit 3 indicates sprite zero
    attributes: u8,
}

// convenience info struct returned by 'Oam::get_sprite_at_dot_info()'.
// isn't stored persistently anywhere
pub struct SpriteInfo {
    pub color_index: u8,
    pub palette_index: u8,
    pub is_in_front: bool,
    pub is_sprite_zero: bool,
}

// convenience methods for the 'SecondaryOam' and 'PrimaryOam' structs
macro_rules! oam_impl {
    ($oam:ty, $n_entries:literal) => {
        impl $oam {
            pub fn as_bytes<'a>(&'a self) -> &'a [u8; $n_entries * 4] {
                unsafe { std::mem::transmute(self) }
            }

            pub fn as_bytes_mut<'a>(&'a mut self) -> &'a mut [u8; $n_entries * 4] {
                unsafe { std::mem::transmute(self) }
            }

            pub fn get_byte(&self, index: u8) -> u8 {
                unsafe { *self.as_bytes().get_unchecked(index as usize) }
            }

            pub fn set_byte(&mut self, index: u8, val: u8) {
                unsafe { *self.as_bytes_mut().get_unchecked_mut(index as usize) = val };
            }

            #[inline]
            pub unsafe fn get_sprite_unchecked(&mut self, index: u8) -> OamEntry {
                *(self.as_bytes_mut().get_unchecked_mut(index as usize) as *mut _ as *mut _)
            }
        }

        impl Default for $oam {
            fn default() -> Self {
                Self {
                    entries: [OamEntry::default(); $n_entries],
                }
            }
        }
    };
}

oam_impl!(PrimaryOam, 64);
oam_impl!(SecondaryOam, 8);

impl Oam {
    pub fn eval_next_scanline_sprite(&mut self, current_scanline: i16, current_scanline_dot: u16) {
        assert!(matches!(current_scanline, -1..=239));
        assert!(matches!(current_scanline_dot, 65..=256));

        if self.sprites_found < 8 {
            let mut sprite = unsafe { self.primary.get_sprite_unchecked(self.current_sprite) };

            // NOTE: 'sprite.y' is 1 less than the screen y coordinate
            if ((current_scanline) as u16).wrapping_sub(sprite.y as u16) < 8 {
                // clear bits 2-4 of attribute byte
                sprite.attributes &= 0b11100011;

                if self.current_sprite == 0 {
                    // set bit 3 of 'attributes' to indicate sprite zero
                    sprite.attributes |= 0b1000
                }

                // copy sprite into secondary oam
                self.secondary.entries[self.sprites_found as usize] = sprite;
                self.sprites_found += 1;
            }
        } else {
            // TODO: sprite overflow stuff (if this is reached, sprites_found == 8)
        }

        // increment 'current_sprite'
        self.current_sprite = self.current_sprite.wrapping_add(1 << 2);
        if self.current_sprite == 0 {
            // FIXME: fail the first y-copy?
        }
    }

    pub fn fetch_next_scanline_sprite_data(
        &mut self,
        current_scanline: i16,
        current_scanline_dot: u16,
        pattern_table_addr: u16,
        memory: &mut dyn PpuMemoryMap,
    ) {
        assert!(matches!(current_scanline, -1..=239));
        assert!(matches!(current_scanline_dot, 257..=320));
        assert!(self.sprites_found <= 8);

        if (self.current_sprite >> 2) < self.sprites_found {
            // fill a slot in 'current_sprites_data' with data for the current sprite

            let sprite = unsafe { self.secondary.get_sprite_unchecked(self.current_sprite) };
            let tile_index = sprite.tile_index;
            let tile = {
                let sprite_table_ptr = memory.get_pattern_tables();
                unsafe {
                    *((sprite_table_ptr
                        .get_unchecked_mut(pattern_table_addr as usize + tile_index as usize * 16))
                        as *mut _ as *mut [u8; 16])
                }
            };

            let x = sprite.x;
            let attributes = sprite.attributes;
            let y = sprite.y;
            // NOTE: 'sprite.y' is 1 less than the screen y coordinate
            let y_offset = current_scanline - y as i16;

            assert!(y_offset >= 0);
            assert!(y_offset < 8);

            let (tile_bitplane_lo, tile_bitplane_hi) = if attributes & 0b10000000 != 0 {
                // use flipped tile bitplanes if sprite is vertically flipped
                (
                    unsafe { *tile.get_unchecked(7 - y_offset as usize) },
                    unsafe { *tile.get_unchecked(15 - y_offset as usize) },
                )
            } else {
                (
                    unsafe { *tile.get_unchecked(0 + y_offset as usize) },
                    unsafe { *tile.get_unchecked(8 + y_offset as usize) },
                )
            };

            // FIXME: indexing
            self.current_sprites_data[(self.current_sprite >> 2) as usize] = SpriteRenderData {
                tile_bitplane_lo,
                tile_bitplane_hi,
                attributes,
                x,
            };

            self.current_sprite = self.current_sprite.wrapping_add(1 << 2);
        } else {
            // fill a slot in 'current_sprites_data' with sentinel value
            // (bit 2 of 'attributes' being set indicates end of array)
            self.current_sprites_data[(self.current_sprite >> 2) as usize] = SpriteRenderData {
                tile_bitplane_lo: 0,
                tile_bitplane_hi: 0,
                attributes: 0b100,
                x: 0,
            };
        }
    }

    pub fn get_sprite_at_dot_info(&self, current_scanline_dot: u16) -> Option<SpriteInfo> {
        self.current_sprites_data
            .iter()
            // bit 2 of 'attributes' being set indicates end of array
            .take_while(|data| data.attributes & 0b100 == 0)
            .find_map(|data| {
                // ignore sprites that are partially outside of the screen
                if data.x >= 0xf9 {
                    return None;
                }

                // get distance between current dot and sprite's leftmost x coordinate
                let tile_offset = current_scanline_dot.wrapping_sub(data.x as u16);

                // if current dot is within x-coords of sprite
                if tile_offset < 8 {
                    // calculate amount to shift tile bitplanes by
                    // to get the current pixel (depends on whether
                    // the sprite is flipped horizontally or not)
                    let shift_amt = if data.attributes & 0b01000000 != 0 {
                        tile_offset
                    } else {
                        7 - tile_offset
                    };

                    let color_index = {
                        let lo = (data.tile_bitplane_lo >> shift_amt) & 1;
                        let hi = ((data.tile_bitplane_hi >> shift_amt) << 1) & 2;
                        lo | hi
                    };

                    let palette_index = (data.attributes & 0b11) | 4;
                    let is_in_front = (data.attributes & 0b100000) != 1;
                    // bit 3 of 'attributes' being set means data belongs to sprite zero
                    let is_sprite_zero = (data.attributes & 0b1000) == 0b1000;

                    return Some(SpriteInfo {
                        palette_index,
                        color_index,
                        is_in_front,
                        is_sprite_zero,
                    });
                }

                None
            })
    }
}
