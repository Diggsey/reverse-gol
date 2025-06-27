use metrohash::MetroHashSet;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator as _};

use crate::{board::Board, miniboard::ReverseIndex, state::State};

mod board;
mod miniboard;
mod state;

const NUM_STEPS: usize = 8;
const BUDGET_FACTOR: usize = 2000;
const MAX_SOLUTIONS: usize = 1000000;
const MIN_SOLUTIONS: usize = 5;
const SEARCH_BREADTH: usize = 250;
const ADDITIONAL_STEPS: usize = 0;

fn compute_previous(mut boards: Vec<Board>, index: &ReverseIndex, steps: usize) -> Vec<Board> {
    for i in 0..steps {
        let mut current_budget_factor = BUDGET_FACTOR;
        let mut desired_solutions = (MAX_SOLUTIONS / boards.len()).max(MIN_SOLUTIONS);
        let mut attempts = 0;
        let results = loop {
            let results = boards
                .par_iter()
                .map(|board| {
                    let board_area = board.width() * board.height();
                    let budget = board_area * current_budget_factor;

                    let mut state = State::new(board, index);

                    state.clear_borders(index);
                    let mut partial_results = MetroHashSet::default();
                    let mut budget_used = budget;
                    let mut solutions_used = desired_solutions;

                    state.solve(
                        index,
                        &mut partial_results,
                        &mut budget_used,
                        &mut solutions_used,
                    );
                    partial_results
                })
                .flatten()
                .collect::<MetroHashSet<_>>();
            if results.len() >= SEARCH_BREADTH || (attempts > 0 && !results.is_empty()) {
                break results;
            }
            current_budget_factor *= 2;
            desired_solutions *= 2;
            attempts += 1;
            println!("    Increasing budget...");
        };

        if results.is_empty() {
            return Vec::new();
        }

        let initial_count = results.len();

        let mut results = results.into_iter().collect::<Vec<_>>();
        results.sort_by_cached_key(|r| r.size() + r.live_count());
        results.truncate(SEARCH_BREADTH);

        for (j, mut board) in results.iter().cloned().enumerate() {
            let initial_board = board.clone();
            for _ in 0..i + 1 + ADDITIONAL_STEPS {
                if j < 10 {
                    println!("{:?}", board);
                }
                board = board.simulate();
            }
            if j < 10 {
                println!("{:?}", board);
                println!("--------------------");
                println!();
            }
            if board.live_count() != 30 {
                dbg!(&boards);
                dbg!(&initial_board);
                dbg!(&board);
                panic!(
                    "Something went wrong, expected 30 live cells, got {}",
                    board.live_count()
                );
            }
        }

        println!("Step -{}: Found {} candidates", i + 1, initial_count);

        boards = results;
    }
    boards
}

fn main() {
    let index = ReverseIndex::compute();
    let boards = Board::load("input.txt").expect("Failed to load board");

    let solutions = compute_previous(boards, &index, NUM_STEPS);
    if !solutions.is_empty() {
        println!("Found {} solutions.", solutions.len());
    } else {
        println!("No solution found.");
    }
}
