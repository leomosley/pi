#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Self = Self(0, 0, 0);
    pub const WHITE: Self = Self(255, 255, 255);

    pub fn rgba(self) -> [u8; 4] {
        [self.0, self.1, self.2, 255]
    }

    pub fn indexed(index: u8) -> Self {
        const BASE: [Color; 16] = [
            Color(0, 0, 0),
            Color(205, 0, 0),
            Color(0, 205, 0),
            Color(205, 205, 0),
            Color(0, 0, 238),
            Color(205, 0, 205),
            Color(0, 205, 205),
            Color(229, 229, 229),
            Color(127, 127, 127),
            Color(255, 0, 0),
            Color(0, 255, 0),
            Color(255, 255, 0),
            Color(92, 92, 255),
            Color(255, 0, 255),
            Color(0, 255, 255),
            Color(255, 255, 255),
        ];
        match index {
            0..=15 => BASE[index as usize],
            16..=231 => {
                const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
                let cube = (index - 16) as usize;
                Self(LEVELS[cube / 36], LEVELS[(cube / 6) % 6], LEVELS[cube % 6])
            }
            _ => {
                let value = 8 + (index - 232) * 10;
                Self(value, value, value)
            }
        }
    }
}
