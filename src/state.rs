use bitvec::vec::BitVec;
use metrohash::MetroHashSet;
use smallvec::{SmallVec, smallvec};
use typenum::{Diff, U2};

use crate::{
    board::Board,
    miniboard::{B, MacroboardSize, ReverseIndex},
};

#[derive(Debug)]
pub struct State<N: MacroboardSize> {
    board: Vec<CellState<N>>,
    stride: usize,
}

#[derive(Debug)]
struct CellState<N: MacroboardSize> {
    mini_b: B<Diff<N, U2>>,
    options: Option<SmallVec<[B<N>; 1]>>,
    priority: usize,
    weight: usize,
}

const INITIAL_WEIGHT: usize = 1000;
const WEIGHT_ADJUST: usize = 10;

impl<N: MacroboardSize> CellState<N> {
    fn options<'a, 'b: 'a>(&'a self, index: &'b ReverseIndex<N>) -> &'a [B<N>] {
        self.options.as_deref().unwrap_or(&index[self.mini_b])
    }
    pub fn recompute_priority(&mut self, index: &ReverseIndex<N>) {
        if self.priority != usize::MAX {
            self.priority = self.options(index).len() + self.weight;
        }
    }
}

impl<N: MacroboardSize> State<N> {
    fn iter_rows(&self) -> impl Iterator<Item = &[CellState<N>]> {
        self.board.chunks(self.stride)
    }
    fn generate_solution_row(
        &self,
        index: &ReverseIndex<N>,
        row: &[CellState<N>],
        y2: usize,
        output: &mut BitVec,
    ) {
        for (x, cell) in row.iter().enumerate() {
            let opts = cell.options(index);
            debug_assert_eq!(opts.len(), 1, "Expected exactly one option",);
            let opt = opts[0];
            if x == 0 {
                for x2 in 0..(N::INT - 1) {
                    output.push(opt.get(x2, y2));
                }
            }
            output.push(opt.get(N::INT - 1, y2));
        }
    }
    fn generate_solution(&self, index: &ReverseIndex<N>) -> Board {
        let mut solution = BitVec::new();
        for (y, row) in self.iter_rows().enumerate() {
            if y == 0 {
                for y2 in 0..(N::INT - 1) {
                    self.generate_solution_row(index, row, y2, &mut solution);
                }
            }
            self.generate_solution_row(index, row, N::INT - 1, &mut solution);
        }
        let mut result = Board::new(solution, self.stride + N::INT - 1);
        result.trim();
        result
    }
    pub fn new(board: &Board, index: &ReverseIndex<N>) -> Self {
        let mut new_board = Vec::new();
        for y in 0..board.height() + 3 - N::INT {
            for x in 0..board.width() + 3 - N::INT {
                let mut b2 = B::EMPTY;
                for dy in 0..(N::INT - 2) {
                    for dx in 0..(N::INT - 2) {
                        if board.get(x + dx, y + dy) {
                            b2.set(dx, dy, true);
                        }
                    }
                }
                new_board.push(CellState {
                    mini_b: b2,
                    options: None,
                    priority: index[b2].len() + INITIAL_WEIGHT,
                    weight: INITIAL_WEIGHT,
                });
            }
        }
        Self {
            board: new_board,
            stride: board.width() + 3 - N::INT,
        }
    }
    pub fn clear_borders(&mut self, index: &ReverseIndex<N>) {
        let w = self.stride;
        let h = self.board.len() / w;
        for y in 0..h {
            self.board[y * w].options = Some(
                self.board[y * w]
                    .options(index)
                    .iter()
                    .copied()
                    .filter(|b4| b4.shift_right(N::INT - 2).step() == B::EMPTY)
                    .collect(),
            );
            self.board[y * w].recompute_priority(index);
            self.board[y * w + w - 1].options = Some(
                self.board[y * w + w - 1]
                    .options(index)
                    .iter()
                    .copied()
                    .filter(|b4| b4.shift_left(N::INT - 2).step() == B::EMPTY)
                    .collect(),
            );
            self.board[y * w + w - 1].recompute_priority(index);
        }
        for x in 0..w {
            self.board[x].options = Some(
                self.board[x]
                    .options(index)
                    .iter()
                    .copied()
                    .filter(|b4| b4.shift_down(N::INT - 2).step() == B::EMPTY)
                    .collect(),
            );
            self.board[x].recompute_priority(index);
            self.board[(h - 1) * w + x].options = Some(
                self.board[(h - 1) * w + x]
                    .options(index)
                    .iter()
                    .copied()
                    .filter(|b4| b4.shift_up(N::INT - 2).step() == B::EMPTY)
                    .collect(),
            );
            self.board[(h - 1) * w + x].recompute_priority(index);
        }
    }
    pub fn solve(
        &mut self,
        index: &ReverseIndex<N>,
        result: &mut MetroHashSet<Board>,
        allowance: &mut usize,
        desired_solutions: &mut usize,
    ) -> bool {
        let w = self.stride;
        let h = self.board.len() / w;
        let (x, y, priority) = self
            .iter_rows()
            .enumerate()
            .flat_map(|(y, row)| {
                row.iter()
                    .enumerate()
                    .map(move |(x, cell)| (x, y, cell.priority))
            })
            .min_by_key(|&(_, _, priority)| priority)
            .unwrap();
        let idx = y * w + x;

        if priority == usize::MAX && *desired_solutions > 0 {
            *desired_solutions -= 1;
            result.insert(self.generate_solution(index));
            return true;
        } else if priority == 0 || *allowance == 0 || *desired_solutions == 0 {
            if priority == 0 {
                self.board[idx].weight = self.board[idx].weight.saturating_sub(WEIGHT_ADJUST);
                self.board[idx].recompute_priority(index);
            }
            // No solution possible
            return false;
        }
        *allowance -= 1;

        let mut success = false;
        self.board[idx].priority = usize::MAX;

        let num_opts = self.board[idx].options(index).len();
        for opt_index in 0..num_opts {
            let opt = self.board[idx].options(index)[opt_index];
            let original_options = self.board[idx].options.replace(smallvec![opt]);

            let mut saved_options = SmallVec::<[_; 4]>::new();
            let directions: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
            let mut conflicting = false;
            for (dx, dy) in directions {
                let nx = x.wrapping_add(dx as usize);
                let ny = y.wrapping_add(dy as usize);
                if nx < w && ny < h {
                    let current_opts = self.board[ny * w + nx].options(index);
                    let new_opts: SmallVec<_> = current_opts
                        .iter()
                        .copied()
                        .filter(|other| opt.is_compatible_with(*other, dx, dy))
                        .collect();
                    if new_opts.is_empty() {
                        conflicting = true;
                    }
                    saved_options.push(self.board[ny * w + nx].options.replace(new_opts));
                    self.board[ny * w + nx].recompute_priority(index);
                }
            }

            if !conflicting {
                success |= self.solve(index, result, allowance, desired_solutions);
            }

            saved_options.reverse();
            for (dx, dy) in directions {
                let nx = x.wrapping_add(dx as usize);
                let ny = y.wrapping_add(dy as usize);
                if nx < w && ny < h {
                    self.board[ny * w + nx].options = saved_options.pop().unwrap();
                    self.board[ny * w + nx].recompute_priority(index);
                }
            }

            self.board[idx].options = original_options;
        }
        self.board[idx].priority = priority;
        success
    }
}
