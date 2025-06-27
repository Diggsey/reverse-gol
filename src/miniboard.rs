use std::{fmt::Debug, ops::Index};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct B2(u8);

impl Debug for B2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "B2({:02b})", self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct B4(u16);

impl Debug for B4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "B4({:016b})", self.0)
    }
}

macro_rules! impl_b {
    ($name:ident, $n:literal) => {
        #[allow(unused)]
        impl $name {
            pub fn empty() -> Self {
                $name(0)
            }
            pub fn get(&self, x: usize, y: usize) -> bool {
                debug_assert!(x < $n && y < $n);
                (self.0 & (1 << (y * $n + x))) != 0
            }
            pub fn set(&mut self, x: usize, y: usize, value: bool) {
                debug_assert!(x < $n && y < $n);
                if value {
                    self.0 |= 1 << (y * $n + x);
                } else {
                    self.0 &= !(1 << (y * $n + x));
                }
            }
        }
    };
}

impl_b!(B2, 2);
impl_b!(B4, 4);

impl B4 {
    pub fn step(self) -> B2 {
        let mut result = B2::empty();
        for y in 0..2 {
            for x in 0..2 {
                let neighbor_count = self.get(x, y) as usize
                    + self.get(x, y + 1) as usize
                    + self.get(x, y + 2) as usize
                    + self.get(x + 1, y) as usize
                    + self.get(x + 1, y + 2) as usize
                    + self.get(x + 2, y) as usize
                    + self.get(x + 2, y + 1) as usize
                    + self.get(x + 2, y + 2) as usize;
                result.set(
                    x,
                    y,
                    neighbor_count == 3 || (self.get(x + 1, y + 1) && neighbor_count == 2),
                );
            }
        }
        result
    }
    pub fn shift_left(self) -> B4 {
        const MASK: u16 = 0b0111_0111_0111_0111;
        B4((self.0 >> 1) & MASK)
    }
    pub fn shift_right(self) -> B4 {
        const MASK: u16 = 0b1110_1110_1110_1110;
        B4((self.0 << 1) & MASK)
    }
    pub fn shift_up(self) -> B4 {
        const MASK: u16 = 0b0000_1111_1111_1111;
        B4((self.0 >> 4) & MASK)
    }
    pub fn shift_down(self) -> B4 {
        const MASK: u16 = 0b1111_1111_1111_0000;
        B4((self.0 << 4) & MASK)
    }
    pub fn can_be_left_of(self, other: B4) -> bool {
        const MASK: u16 = 0b1110_1110_1110_1110;
        self.0 & MASK == (other.0 << 1) & MASK
    }
    pub fn can_be_above(self, other: B4) -> bool {
        const MASK: u16 = 0b1111_1111_1111_0000;
        self.0 & MASK == (other.0 << 4) & MASK
    }
    pub fn is_compatible_with(self, other: B4, dx: i32, dy: i32) -> bool {
        match (dx, dy) {
            (1, 0) => self.can_be_left_of(other),
            (0, 1) => self.can_be_above(other),
            (-1, 0) => other.can_be_left_of(self),
            (0, -1) => other.can_be_above(self),
            _ => false,
        }
    }
    pub fn live_count(self) -> u32 {
        self.0.count_ones()
    }
}

#[derive(Debug)]
pub struct ReverseIndex(Vec<Vec<B4>>);

impl ReverseIndex {
    pub fn compute() -> Self {
        let mut index = Vec::new();
        index.resize(1 << 16, Vec::new());
        for b4 in (0..=0b1111_1111_1111_1111).map(B4) {
            let b2 = b4.step();

            index[b2.0 as usize].push(b4);
        }
        for item in &mut index {
            item.sort_by_key(|b4| b4.live_count());
        }
        ReverseIndex(index)
    }
}

impl Index<B2> for ReverseIndex {
    type Output = Vec<B4>;

    fn index(&self, b2: B2) -> &Self::Output {
        &self.0[b2.0 as usize]
    }
}
