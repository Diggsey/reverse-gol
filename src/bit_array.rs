use std::{
    fmt::{Debug, Display},
    hash::Hash,
    ops::{
        BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, ShlAssign, Shr,
        ShrAssign,
    },
};

use num::{
    NumCast, PrimInt, ToPrimitive as _, Unsigned,
    traits::{ConstOne, ConstZero},
};
use typenum::ToInt;

macro_rules! impl_bit_array_sizes {
    ($($($n:ident),*: $t:ty,)*) => {
        $($(impl BitArraySize for typenum::$n {
            type T = $t;
            const MASK: Self::T = Self::T::MAX >> (Self::T::BITS - <typenum::$n as ToInt<u32>>::INT);
        })*)*
    };
}

impl_bit_array_sizes! {
    U1,U2,U3,U4,U5,U6,U7,U8: u8,
    U9,U10,U11,U12,U13,U14,U15,U16: u16,
    U17,U18,U19,U20,U21,U22,U23,U24,U25,U26,U27,U28,U29,U30,U31,U32: u32,
    U33,U34,U35,U36,U37,U38,U39,U40,U41,U42,U43,U44,U45,U46,U47,U48,U49,U50,U51,U52,U53,U54,U55,U56,U57,U58,U59,U60,U61,U62,U63,U64: u64,
}

pub trait BitArraySize:
    ToInt<usize> + typenum::Unsigned + Copy + Clone + Debug + PartialEq + Eq + Hash
{
    type T: PrimInt + Unsigned + Hash + ConstZero + ConstOne + Display;
    const MASK: Self::T;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitArray<N: BitArraySize>(pub N::T);

impl<N: BitArraySize> BitArray<N> {
    pub const ZERO: Self = Self(N::T::ZERO);
    pub const ONE: Self = Self(N::T::ONE);
    pub const MAX: Self = Self(N::MASK);

    #[inline]
    pub fn get(self, index: usize) -> bool {
        debug_assert!(index < N::INT, "Index out of bounds: {}", index);
        (self & (Self::ONE >> index)) != Self::ZERO
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: bool) {
        debug_assert!(index < N::INT, "Index out of bounds: {}", index);
        if value {
            *self |= Self::ONE >> index;
        } else {
            *self &= !(Self::ONE >> index);
        }
    }

    #[inline(always)]
    pub fn count_ones(self) -> u32 {
        self.0.count_ones()
    }

    pub fn to_u64(self) -> u64 {
        self.0.to_u64().expect("BitArray size exceeds u64 range")
    }

    pub fn from_u64(value: u64) -> Self {
        let value = <N::T as NumCast>::from(value).expect("Invalid BitArray size");
        debug_assert!(
            value <= N::MASK,
            "Value exceeds BitArray size: {} > {}",
            value,
            N::MASK
        );
        Self(value & N::MASK)
    }
}

impl<N: BitArraySize> BitAnd for BitArray<N> {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl<N: BitArraySize> BitAndAssign for BitArray<N> {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl<N: BitArraySize> BitOr for BitArray<N> {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl<N: BitArraySize> BitOrAssign for BitArray<N> {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl<N: BitArraySize> BitXor for BitArray<N> {
    type Output = Self;

    #[inline(always)]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl<N: BitArraySize> BitXorAssign for BitArray<N> {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl<N: BitArraySize> Not for BitArray<N> {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self::Output {
        Self(!self.0 & N::MASK)
    }
}

impl<N: BitArraySize> Shl<usize> for BitArray<N> {
    type Output = Self;

    #[inline(always)]
    fn shl(self, rhs: usize) -> Self::Output {
        // BitArray imagines bits from lowest to highest
        #[allow(clippy::suspicious_arithmetic_impl)]
        Self(self.0 >> rhs)
    }
}

impl<N: BitArraySize> ShlAssign<usize> for BitArray<N> {
    #[inline(always)]
    fn shl_assign(&mut self, rhs: usize) {
        *self = *self << rhs;
    }
}

impl<N: BitArraySize> Shr<usize> for BitArray<N> {
    type Output = Self;

    #[inline(always)]
    fn shr(self, rhs: usize) -> Self::Output {
        // BitArray imagines bits from lowest to highest
        #[allow(clippy::suspicious_arithmetic_impl)]
        Self((self.0 << rhs) & N::MASK)
    }
}

impl<N: BitArraySize> ShrAssign<usize> for BitArray<N> {
    #[inline(always)]
    fn shr_assign(&mut self, rhs: usize) {
        *self = *self >> rhs;
    }
}

impl<N: BitArraySize> Debug for BitArray<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for i in 0..N::INT {
            if self.get(i) {
                write!(f, "1")?;
            } else {
                write!(f, "0")?;
            }
        }
        write!(f, "]")
    }
}
