use std::{
    fmt::Debug,
    hash::Hash,
    ops::{Index, Mul, Sub},
};

use typenum::{Diff, Square, ToInt, U2};

use crate::bit_array::{BitArray, BitArraySize};

pub trait MiniboardSize:
    Sized + Mul<Output: BitArraySize> + ToInt<usize> + Copy + Clone + PartialEq + Eq + Hash
{
}
impl<N: Mul<Output: BitArraySize> + ToInt<usize> + Copy + Clone + PartialEq + Eq + Hash>
    MiniboardSize for N
{
}

pub trait MacroboardSize: MiniboardSize + Sub<U2, Output: MiniboardSize> {}
impl<N: MiniboardSize + Sub<U2, Output: MiniboardSize>> MacroboardSize for N {}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct B<N: MiniboardSize>(BitArray<Square<N>>);

impl<N: MiniboardSize> B<N> {
    pub const EMPTY: Self = B(BitArray::ZERO);

    fn h_mask() -> BitArray<Square<N>> {
        !BitArray(
            <BitArray<Square<N>>>::MAX.0
                / (<BitArray<Square<N>>>::MAX.0 >> (N::INT * N::INT - N::INT)),
        )
    }

    fn v_mask() -> BitArray<Square<N>> {
        BitArray((<BitArray<Square<N>>>::MAX.0 << N::INT) & <BitArray<Square<N>>>::MAX.0)
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        debug_assert!(x < N::INT && y < N::INT);
        self.0.get(y * N::INT + x)
    }

    pub fn set(&mut self, x: usize, y: usize, value: bool) {
        debug_assert!(x < N::INT && y < N::INT);
        self.0.set(y * N::INT + x, value);
    }

    pub fn shift_left(mut self, n: usize) -> Self {
        for _ in 0..n {
            self.0 &= Self::h_mask();
            self.0 <<= 1;
        }
        self
    }
    pub fn shift_right(mut self, n: usize) -> Self {
        for _ in 0..n {
            self.0 >>= 1;
            self.0 &= Self::h_mask();
        }
        self
    }
    pub fn shift_up(mut self, n: usize) -> Self {
        self.0 <<= N::INT * n;
        self
    }
    pub fn shift_down(mut self, n: usize) -> Self {
        self.0 >>= N::INT * n;
        self.0 &= Self::v_mask();
        self
    }

    pub fn can_be_left_of(self, other: Self) -> bool {
        self.0 & Self::h_mask() == other.shift_right(1).0
    }
    pub fn can_be_above(self, other: Self) -> bool {
        self.0 & Self::v_mask() == other.shift_down(1).0
    }
    pub fn is_compatible_with(self, other: Self, dx: i32, dy: i32) -> bool {
        match (dx, dy) {
            (1, 0) => self.can_be_left_of(other),
            (0, 1) => self.can_be_above(other),
            (-1, 0) => other.can_be_left_of(self),
            (0, -1) => other.can_be_above(self),
            _ => false,
        }
    }
    #[inline(always)]
    pub fn live_count(self) -> u32 {
        self.0.count_ones()
    }
}

impl<N: MiniboardSize> Debug for B<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "B{}{:?}", N::INT, self.0)
    }
}

impl<N: MacroboardSize> B<N> {
    pub fn step(self) -> B<Diff<N, U2>> {
        let mut result = B::<Diff<N, U2>>::EMPTY;
        for y in 0..(N::INT - 2) {
            for x in 0..(N::INT - 2) {
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
}

#[derive(Debug)]
pub struct ReverseIndex<N: MacroboardSize>(Vec<Vec<B<N>>>);

impl<N: MacroboardSize> ReverseIndex<N> {
    pub fn compute() -> Self {
        let mut index = Vec::new();
        let size = 1 << <Square<N>>::INT;
        index.resize(size as usize, Vec::new());
        for i in 0..size {
            let b: B<N> = B(BitArray::from_u64(i));
            let b_small = b.step();

            index[b_small.0.to_u64() as usize].push(b);
        }
        for item in &mut index {
            item.sort_by_key(|b| b.live_count());
        }
        ReverseIndex(index)
    }
}

impl<N: MacroboardSize> Index<B<Diff<N, U2>>> for ReverseIndex<N> {
    type Output = Vec<B<N>>;

    fn index(&self, b_small: B<Diff<N, U2>>) -> &Self::Output {
        &self.0[b_small.0.to_u64() as usize]
    }
}
