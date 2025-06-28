use metrohash::MetroHashSet;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator as _};

use crate::{board::Board, reverse_index::ReverseIndex, state::State};

mod bit_array;
mod board;
mod miniboard;
mod reverse_index;
mod state;

type N = typenum::U5;
const NUM_STEPS: usize = 16;
const BUDGET_FACTOR: usize = 2000;
const MAX_SOLUTIONS: usize = 1000000;
const MIN_SOLUTIONS: usize = 5;
const SEARCH_BREADTH: usize = 250;
const ADDITIONAL_STEPS: usize = 0;
const PRINT_SOLUTIONS: usize = 1;
const PARALLEL: bool = true;

fn solve_board(
    board: &Board,
    index: &ReverseIndex<N>,
    current_budget_factor: usize,
    desired_solutions: usize,
    results: &mut MetroHashSet<Board>,
) {
    let board_area = board.width() * board.height();
    let budget = board_area * current_budget_factor;

    let mut state = State::new(board, index);

    state.clear_borders(index);

    let mut budget_used = budget;
    let mut solutions_used = desired_solutions;

    state.solve(index, results, &mut budget_used, &mut solutions_used);
}

fn compute_previous(mut boards: Vec<Board>, index: &ReverseIndex<N>, steps: usize) -> Vec<Board> {
    for i in 0..steps {
        let mut current_budget_factor = BUDGET_FACTOR;
        let mut desired_solutions = (MAX_SOLUTIONS / boards.len()).max(MIN_SOLUTIONS);
        let mut attempts = 0;
        let results = loop {
            let results = if PARALLEL {
                boards
                    .par_iter()
                    .map(|board| {
                        let mut partial_results = MetroHashSet::default();
                        solve_board(
                            board,
                            index,
                            current_budget_factor,
                            desired_solutions,
                            &mut partial_results,
                        );
                        partial_results
                    })
                    .flatten()
                    .collect::<MetroHashSet<_>>()
            } else {
                let mut results = MetroHashSet::default();
                for board in &boards {
                    solve_board(
                        board,
                        index,
                        current_budget_factor,
                        desired_solutions,
                        &mut results,
                    );
                }
                results
            };
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
            for _ in 0..i + 1 + ADDITIONAL_STEPS {
                if j < PRINT_SOLUTIONS {
                    println!("{:?}", board);
                }
                board = board.simulate();
            }
            if j < PRINT_SOLUTIONS {
                println!("{:?}", board);
                println!("--------------------");
                println!();
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
