use std::mem;

use bitvec::vec::BitVec;
use metrohash::MetroHashSet;
use smallvec::SmallVec;

use crate::{
    board::Board,
    miniboard::{B, MacroboardSize},
    reverse_index::{Constraint, Direction, ReverseIndex, ReverseIndexKey},
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum InstructionPointer {
    #[default]
    Call,
    LoopStart,
    LoopMiddle,
    LoopEnd,
    Return,
}

#[derive(Default, Debug)]
struct StackFrame<N: MacroboardSize> {
    ip: InstructionPointer,
    idx: usize,
    priority: usize,
    opt_index: usize,
    saved_options: SmallVec<[ReverseIndexKey<N>; 4]>,
    original_options: ReverseIndexKey<N>,
}
#[derive(Debug)]
pub struct State<N: MacroboardSize> {
    board: Vec<CellState<N>>,
    stride: usize,
    stack: Vec<StackFrame<N>>,
    frame: StackFrame<N>,
}

#[derive(Debug)]
struct CellState<N: MacroboardSize> {
    key: ReverseIndexKey<N>,
    priority: usize,
    weight: usize,
}

const INITIAL_WEIGHT: usize = 1000;
const WEIGHT_ADJUST: usize = 10;

impl<N: MacroboardSize> CellState<N> {
    pub fn recompute_priority(&mut self, index: &ReverseIndex<N>) {
        if self.priority != usize::MAX {
            self.priority = self.key.options(index).len() + self.weight;
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
            let opts = cell.key.options(index);
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
                let mut miniboard = B::EMPTY;
                for dy in 0..(N::INT - 2) {
                    for dx in 0..(N::INT - 2) {
                        if board.get(x + dx, y + dy) {
                            miniboard.set(dx, dy, true);
                        }
                    }
                }
                let key = ReverseIndexKey::Unconstrained { miniboard };
                new_board.push(CellState {
                    priority: key.options(index).len() + INITIAL_WEIGHT,
                    key,
                    weight: INITIAL_WEIGHT,
                });
            }
        }
        let stride = board.width() + 3 - N::INT;
        let mut result = Self {
            board: new_board,
            stride,
            stack: Vec::with_capacity(stride * stride),
            frame: StackFrame::default(),
        };
        result.clear_borders(index);
        result
    }
    pub fn clear_borders(&mut self, index: &ReverseIndex<N>) {
        let w = self.stride;
        let h = self.board.len() / w;
        for y in 0..h {
            self.board[y * w].key = self.board[y * w].key.constrain(
                Constraint::Edge {
                    dir: Direction::Left,
                },
                index,
            );
            self.board[y * w].recompute_priority(index);
            self.board[y * w + w - 1].key = self.board[y * w + w - 1].key.constrain(
                Constraint::Edge {
                    dir: Direction::Right,
                },
                index,
            );
            self.board[y * w + w - 1].recompute_priority(index);
        }
        for x in 0..w {
            self.board[x].key = self.board[x]
                .key
                .constrain(Constraint::Edge { dir: Direction::Up }, index);
            self.board[x].recompute_priority(index);
            self.board[(h - 1) * w + x].key = self.board[(h - 1) * w + x].key.constrain(
                Constraint::Edge {
                    dir: Direction::Down,
                },
                index,
            );
            self.board[(h - 1) * w + x].recompute_priority(index);
        }
    }

    pub fn is_done(&self) -> bool {
        self.frame.ip == InstructionPointer::Return && self.stack.is_empty()
    }

    pub fn advance(
        &mut self,
        index: &ReverseIndex<N>,
        result: &mut MetroHashSet<Board>,
        steps: usize,
    ) -> bool {
        let w = self.stride;
        let h = self.board.len() / w;
        let mut success = false;

        for _ in 0..steps {
            match self.frame.ip {
                InstructionPointer::Call => {
                    (self.frame.idx, self.frame.priority) = self
                        .board
                        .iter()
                        .enumerate()
                        .map(move |(idx, cell)| (idx, cell.priority))
                        .min_by_key(|&(_, priority)| priority)
                        .unwrap();

                    if self.frame.priority == usize::MAX {
                        // Found solution
                        result.insert(self.generate_solution(index));
                        success = true;
                        self.frame.ip = InstructionPointer::Return;
                        continue;
                    } else if self.board[self.frame.idx].key.options(index).is_empty() {
                        self.board[self.frame.idx].weight = self.board[self.frame.idx]
                            .weight
                            .saturating_sub(WEIGHT_ADJUST);
                        self.board[self.frame.idx].recompute_priority(index);
                        // No solution possible
                        self.frame.ip = InstructionPointer::Return;
                        continue;
                    }

                    self.board[self.frame.idx].priority = usize::MAX;

                    self.frame.ip = InstructionPointer::LoopStart;
                }
                InstructionPointer::LoopStart => {
                    let opt = self.board[self.frame.idx].key.options(index)[self.frame.opt_index];
                    self.frame.original_options = mem::replace(
                        &mut self.board[self.frame.idx].key,
                        ReverseIndexKey::one(opt),
                    );

                    let mut conflicting = false;
                    for dir in Direction::ALL {
                        let nx = (self.frame.idx % w).wrapping_add(dir.dx() as usize);
                        let ny = (self.frame.idx / w).wrapping_add(dir.dy() as usize);
                        if nx < w && ny < h {
                            let new_opts = self.board[ny * w + nx]
                                .key
                                .constrain(Constraint::neighbor(opt, dir.rev()), index);
                            if new_opts.options(index).is_empty() {
                                conflicting = true;
                            }
                            let prev_opts =
                                mem::replace(&mut self.board[ny * w + nx].key, new_opts);
                            self.board[ny * w + nx].recompute_priority(index);
                            self.frame.saved_options.push(prev_opts);
                        }
                    }
                    self.frame.ip = InstructionPointer::LoopMiddle;
                    if !conflicting {
                        self.stack.push(mem::take(&mut self.frame));
                    }
                }
                InstructionPointer::LoopMiddle => {
                    self.frame.saved_options.reverse();
                    for dir in Direction::ALL {
                        let nx = (self.frame.idx % w).wrapping_add(dir.dx() as usize);
                        let ny = (self.frame.idx / w).wrapping_add(dir.dy() as usize);
                        if nx < w && ny < h {
                            self.board[ny * w + nx].key = self.frame.saved_options.pop().unwrap();
                            self.board[ny * w + nx].recompute_priority(index);
                        }
                    }

                    self.board[self.frame.idx].key = mem::take(&mut self.frame.original_options);

                    self.frame.opt_index += 1;
                    if self.frame.opt_index < self.board[self.frame.idx].key.options(index).len() {
                        self.frame.ip = InstructionPointer::LoopStart;
                    } else {
                        self.frame.ip = InstructionPointer::LoopEnd;
                    }
                }
                InstructionPointer::LoopEnd => {
                    self.board[self.frame.idx].priority = self.frame.priority;
                    self.frame.ip = InstructionPointer::Return;
                }
                InstructionPointer::Return => {
                    if let Some(f) = self.stack.pop() {
                        self.frame = f;
                    } else {
                        break;
                    }
                }
            }
        }

        success
    }
}
