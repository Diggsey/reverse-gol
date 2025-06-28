use std::mem;

use bitvec::vec::BitVec;
use metrohash::MetroHashSet;
use smallvec::SmallVec;

use crate::{
    board::Board,
    miniboard::{B, MacroboardSize},
    reverse_index::{Constraint, Direction, ReverseIndex, ReverseIndexKey},
};

#[derive(Debug)]
pub struct State<N: MacroboardSize> {
    board: Vec<CellState<N>>,
    stride: usize,
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
        Self {
            board: new_board,
            stride: board.width() + 3 - N::INT,
        }
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
    pub fn solve(
        &mut self,
        index: &ReverseIndex<N>,
        result: &mut MetroHashSet<Board>,
        allowance: &mut usize,
        desired_solutions: &mut usize,
    ) -> bool {
        #[derive(Default)]
        enum InstructionPointer {
            #[default]
            Call,
            LoopStart,
            LoopMiddle,
            LoopEnd,
            Return,
        }

        #[derive(Default)]
        struct StackFrame<N: MacroboardSize> {
            ip: InstructionPointer,
            idx: usize,
            priority: usize,
            opt_index: usize,
            saved_options: SmallVec<[ReverseIndexKey<N>; 4]>,
            original_options: ReverseIndexKey<N>,
        }

        let w = self.stride;
        let h = self.board.len() / w;
        let mut stack = Vec::<StackFrame<N>>::new();
        let mut frame = StackFrame::<N>::default();
        let mut success = false;

        loop {
            match frame.ip {
                InstructionPointer::Call => {
                    (frame.idx, frame.priority) = self
                        .board
                        .iter()
                        .enumerate()
                        .map(move |(idx, cell)| (idx, cell.priority))
                        .min_by_key(|&(_, priority)| priority)
                        .unwrap();

                    if frame.priority == usize::MAX && *desired_solutions > 0 {
                        // Found solution
                        *desired_solutions -= 1;
                        result.insert(self.generate_solution(index));
                        success = true;
                        frame.ip = InstructionPointer::Return;
                        continue;
                    } else if self.board[frame.idx].key.options(index).is_empty()
                        || *allowance == 0
                        || *desired_solutions == 0
                    {
                        if frame.priority == 0 {
                            self.board[frame.idx].weight =
                                self.board[frame.idx].weight.saturating_sub(WEIGHT_ADJUST);
                            self.board[frame.idx].recompute_priority(index);
                        }
                        // No solution possible
                        frame.ip = InstructionPointer::Return;
                        continue;
                    }
                    *allowance -= 1;

                    self.board[frame.idx].priority = usize::MAX;

                    frame.ip = InstructionPointer::LoopStart;
                }
                InstructionPointer::LoopStart => {
                    let opt = self.board[frame.idx].key.options(index)[frame.opt_index];
                    frame.original_options =
                        mem::replace(&mut self.board[frame.idx].key, ReverseIndexKey::one(opt));

                    let mut conflicting = false;
                    for dir in Direction::ALL {
                        let nx = (frame.idx % w).wrapping_add(dir.dx() as usize);
                        let ny = (frame.idx / w).wrapping_add(dir.dy() as usize);
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
                            frame.saved_options.push(prev_opts);
                        }
                    }
                    frame.ip = InstructionPointer::LoopMiddle;
                    if !conflicting {
                        stack.push(mem::take(&mut frame));
                    }
                }
                InstructionPointer::LoopMiddle => {
                    frame.saved_options.reverse();
                    for dir in Direction::ALL {
                        let nx = (frame.idx % w).wrapping_add(dir.dx() as usize);
                        let ny = (frame.idx / w).wrapping_add(dir.dy() as usize);
                        if nx < w && ny < h {
                            self.board[ny * w + nx].key = frame.saved_options.pop().unwrap();
                            self.board[ny * w + nx].recompute_priority(index);
                        }
                    }

                    self.board[frame.idx].key = mem::take(&mut frame.original_options);

                    frame.opt_index += 1;
                    if frame.opt_index < self.board[frame.idx].key.options(index).len() {
                        frame.ip = InstructionPointer::LoopStart;
                    } else {
                        frame.ip = InstructionPointer::LoopEnd;
                    }
                }
                InstructionPointer::LoopEnd => {
                    self.board[frame.idx].priority = frame.priority;
                    frame.ip = InstructionPointer::Return;
                }
                InstructionPointer::Return => {
                    if let Some(f) = stack.pop() {
                        frame = f;
                    } else {
                        break;
                    }
                }
            }
        }

        success
    }
}
