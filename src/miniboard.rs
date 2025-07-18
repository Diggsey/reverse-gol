use std::{
    fmt::Debug,
    hash::Hash,
    ops::{Mul, Sub},
};

use typenum::{Diff, Square, ToInt, U2};

use crate::bit_array::{BitArray, BitArraySize};

pub trait MiniboardSize:
    Sized
    + Mul<Output: BitArraySize>
    + ToInt<usize>
    + Copy
    + Clone
    + PartialEq
    + Eq
    + Hash
    + Default
    + Debug
    + Send
    + Sync
    + 'static
{
}
impl<
    N: Mul<Output: BitArraySize>
        + ToInt<usize>
        + Copy
        + Clone
        + PartialEq
        + Eq
        + Hash
        + Default
        + Debug
        + Send
        + Sync
        + 'static,
> MiniboardSize for N
{
}

pub trait MacroboardSize: MiniboardSize + Sub<U2, Output: MiniboardSize> {}
impl<N: MiniboardSize + Sub<U2, Output: MiniboardSize>> MacroboardSize for N {}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct B<N: MiniboardSize>(pub BitArray<Square<N>>);

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
