use crate::{board::Board, work_queue::WorkQueue};

mod bit_array;
mod board;
mod miniboard;
mod reverse_index;
mod state;
mod work_queue;

type N = typenum::U5;
const NUM_STEPS: usize = 16;
const BUDGET_FACTOR: usize = 100000;
const PARALLEL: bool = true;

fn main() {
    let boards = Board::load("input.txt").expect("Failed to load board");

    let queue = WorkQueue::<N>::start(boards, NUM_STEPS, PARALLEL);
    queue.wait();
}
