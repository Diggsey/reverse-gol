use std::ops::Index;

use metrohash::MetroHashMap;
use smallvec::{SmallVec, smallvec};
use typenum::{Diff, Square, ToInt, U2};

use crate::{
    bit_array::BitArray,
    miniboard::{B, MacroboardSize, MiniboardSize},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ];
    pub fn shift<N: MiniboardSize>(&self, b: B<N>, n: usize) -> B<N> {
        match self {
            Direction::Up => b.shift_up(n),
            Direction::Down => b.shift_down(n),
            Direction::Left => b.shift_left(n),
            Direction::Right => b.shift_right(n),
        }
    }
    #[inline(always)]
    pub fn rev(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
    #[inline(always)]
    pub fn dx(self) -> i32 {
        match self {
            Direction::Left => -1,
            Direction::Right => 1,
            _ => 0,
        }
    }
    #[inline(always)]
    pub fn dy(self) -> i32 {
        match self {
            Direction::Up => -1,
            Direction::Down => 1,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Constraint<N: MacroboardSize> {
    Neighbor { macroboard: B<N>, dir: Direction },
    Edge { dir: Direction },
}

impl<N: MacroboardSize> Constraint<N> {
    pub fn neighbor(macroboard: B<N>, dir: Direction) -> Self {
        Constraint::Neighbor {
            macroboard: dir.shift(macroboard, 1),
            dir,
        }
    }
    pub fn compute(macroboard: B<N>) -> SmallVec<[Constraint<N>; 8]> {
        let mut result = SmallVec::new();
        result.extend(
            Direction::ALL
                .into_iter()
                .filter(|dir| dir.rev().shift(macroboard, N::INT - 2).step() == B::EMPTY)
                .map(|dir| Constraint::Edge { dir }),
        );
        result.extend(Direction::ALL.into_iter().map(|dir| Constraint::Neighbor {
            macroboard: dir.shift(dir.rev().shift(macroboard, 1), 1),
            dir,
        }));
        result
    }
    pub fn matches(self, b: B<N>) -> bool {
        match self {
            Constraint::Neighbor { macroboard, dir } => {
                macroboard == dir.shift(dir.rev().shift(b, 1), 1)
            }
            Constraint::Edge { dir } => dir.rev().shift(b, N::INT - 2).step() == B::EMPTY,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ReverseIndexSegment<N: MacroboardSize> {
    map: MetroHashMap<Constraint<N>, Vec<B<N>>>,
    all: Vec<B<N>>,
}

impl<N: MacroboardSize> ReverseIndexSegment<N> {
    pub fn push(&mut self, b: B<N>) {
        self.all.push(b);
        for k in Constraint::compute(b) {
            self.map.entry(k).or_default().push(b);
        }
    }
    pub fn sort(&mut self) {
        self.all.sort_by_key(|b| b.live_count());
        for v in self.map.values_mut() {
            v.sort_by_key(|b| b.live_count());
        }
    }
}

impl<N: MacroboardSize> Index<Constraint<N>> for ReverseIndexSegment<N> {
    type Output = [B<N>];

    fn index(&self, constraint: Constraint<N>) -> &Self::Output {
        self.map
            .get(&constraint)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

#[derive(Debug)]
pub struct ReverseIndex<N: MacroboardSize>(Vec<ReverseIndexSegment<N>>);

impl<N: MacroboardSize> ReverseIndex<N> {
    pub fn compute() -> Self {
        let mut index = Vec::new();
        let size = 1 << <Square<N>>::INT;
        index.resize(size as usize, ReverseIndexSegment::default());
        for i in 0..size {
            let b: B<N> = B(BitArray::from_u64(i));
            let b_small = b.step();

            index[b_small.0.to_u64() as usize].push(b);
        }
        for item in &mut index {
            item.sort();
        }
        ReverseIndex(index)
    }
}

impl<N: MacroboardSize> Index<B<Diff<N, U2>>> for ReverseIndex<N> {
    type Output = ReverseIndexSegment<N>;

    fn index(&self, miniboard: B<Diff<N, U2>>) -> &Self::Output {
        &self.0[miniboard.0.to_u64() as usize]
    }
}

#[derive(Debug)]
pub enum ReverseIndexKey<N: MacroboardSize> {
    Unconstrained {
        miniboard: B<Diff<N, U2>>,
    },
    Constrained {
        miniboard: B<Diff<N, U2>>,
        constraint: Constraint<N>,
    },
    List {
        options: SmallVec<[B<N>; 1]>,
    },
}

impl<N: MacroboardSize> Default for ReverseIndexKey<N> {
    fn default() -> Self {
        ReverseIndexKey::List {
            options: smallvec![],
        }
    }
}

impl<N: MacroboardSize> ReverseIndexKey<N> {
    pub fn constrain(&self, constraint: Constraint<N>, index: &ReverseIndex<N>) -> Self {
        let existing_options = match self {
            ReverseIndexKey::Unconstrained { miniboard } => {
                return ReverseIndexKey::Constrained {
                    miniboard: *miniboard,
                    constraint,
                };
            }
            ReverseIndexKey::Constrained {
                miniboard,
                constraint: existing,
            } => &index[*miniboard][*existing],
            ReverseIndexKey::List { options } => options,
        };
        ReverseIndexKey::List {
            options: existing_options
                .iter()
                .copied()
                .filter(|b| constraint.matches(*b))
                .collect(),
        }
    }
    pub fn options<'a, 'b: 'a>(&'a self, index: &'b ReverseIndex<N>) -> &'a [B<N>] {
        match self {
            ReverseIndexKey::Unconstrained { miniboard } => &index[*miniboard].all,
            ReverseIndexKey::Constrained {
                miniboard,
                constraint,
            } => &index[*miniboard][*constraint],
            ReverseIndexKey::List { options } => options,
        }
    }
    pub fn one(b: B<N>) -> Self {
        ReverseIndexKey::List {
            options: smallvec![b],
        }
    }
}
