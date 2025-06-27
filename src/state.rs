use bitvec::vec::BitVec;
use metrohash::MetroHashSet;
use smallvec::{SmallVec, smallvec};

use crate::{
    board::Board,
    miniboard::{B2, B4, ReverseIndex},
};

#[derive(Debug)]
pub struct State {
    board: Vec<CellState>,
    stride: usize,
}

#[derive(Debug)]
struct CellState {
    b2: B2,
    options: Option<SmallVec<[B4; 1]>>,
    priority: usize,
    weight: usize,
}

const INITIAL_WEIGHT: usize = 1000;
const WEIGHT_ADJUST: usize = 10;

impl CellState {
    fn options<'a, 'b: 'a>(&'a self, index: &'b ReverseIndex) -> &'a [B4] {
        self.options.as_deref().unwrap_or(&index[self.b2])
    }
    pub fn recompute_priority(&mut self, index: &ReverseIndex) {
        if self.priority != usize::MAX {
            self.priority = self.options(index).len() + self.weight;
        }
    }
}

impl State {
    fn iter_rows(&self) -> impl Iterator<Item = &[CellState]> {
        self.board.chunks(self.stride)
    }
    fn generate_solution_row(
        &self,
        index: &ReverseIndex,
        row: &[CellState],
        y2: usize,
        output: &mut BitVec,
    ) {
        for (x, cell) in row.iter().enumerate() {
            let opts = cell.options(index);
            debug_assert_eq!(opts.len(), 1, "Expected exactly one option",);
            let opt = opts[0];
            if x == 0 {
                for x2 in 0..3 {
                    output.push(opt.get(x2, y2));
                }
            }
            output.push(opt.get(3, y2));
        }
    }
    fn generate_solution(&self, index: &ReverseIndex) -> Board {
        let mut solution = BitVec::new();
        for (y, row) in self.iter_rows().enumerate() {
            if y == 0 {
                for y2 in 0..3 {
                    self.generate_solution_row(index, row, y2, &mut solution);
                }
            }
            self.generate_solution_row(index, row, 3, &mut solution);
        }
        let mut result = Board::new(solution, self.stride + 3);
        result.trim();
        result
    }
    pub fn new(board: &Board, index: &ReverseIndex) -> Self {
        let mut new_board = Vec::new();
        for y in 0..board.height() - 1 {
            for x in 0..board.width() - 1 {
                let mut b2 = B2::empty();
                for dy in 0..2 {
                    for dx in 0..2 {
                        if board.get(x + dx, y + dy) {
                            b2.set(dx, dy, true);
                        }
                    }
                }
                new_board.push(CellState {
                    b2,
                    options: None,
                    priority: index[b2].len() + INITIAL_WEIGHT,
                    weight: INITIAL_WEIGHT,
                });
            }
        }
        Self {
            board: new_board,
            stride: board.width() - 1,
        }
    }
    pub fn clear_borders(&mut self, index: &ReverseIndex) {
        let w = self.stride;
        let h = self.board.len() / w;
        for y in 0..h {
            self.board[y * w].options = Some(
                self.board[y * w]
                    .options(index)
                    .iter()
                    .copied()
                    .filter(|b4| b4.shift_right().shift_right().step() == B2::empty())
                    .collect(),
            );
            self.board[y * w].recompute_priority(index);
            self.board[y * w + w - 1].options = Some(
                self.board[y * w + w - 1]
                    .options(index)
                    .iter()
                    .copied()
                    .filter(|b4| b4.shift_left().shift_left().step() == B2::empty())
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
                    .filter(|b4| b4.shift_down().shift_down().step() == B2::empty())
                    .collect(),
            );
            self.board[x].recompute_priority(index);
            self.board[(h - 1) * w + x].options = Some(
                self.board[(h - 1) * w + x]
                    .options(index)
                    .iter()
                    .copied()
                    .filter(|b4| b4.shift_up().shift_up().step() == B2::empty())
                    .collect(),
            );
            self.board[(h - 1) * w + x].recompute_priority(index);
        }
    }
    pub fn solve(
        &mut self,
        index: &ReverseIndex,
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
