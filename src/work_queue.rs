use std::{
    collections::BinaryHeap,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use metrohash::MetroHashSet;

use crate::{
    BUDGET_FACTOR, board::Board, miniboard::MacroboardSize, reverse_index::ReverseIndex,
    state::State,
};

struct WorkItem<N: MacroboardSize> {
    state: State<N>,
    step: usize,
    priority: isize,
}

fn compute_priority(step: usize, live_count: usize, size: usize) -> isize {
    (step as isize + 10) * (step as isize + 10) * 10 - (live_count as isize) - (size as isize)
}

impl<N: MacroboardSize> WorkItem<N> {
    fn new(board: Board, index: &ReverseIndex<N>, step: usize) -> Self {
        Self {
            state: State::new(&board, index),
            step,
            priority: compute_priority(step, board.live_count(), board.size()),
        }
    }
    fn advance(&mut self, index: &ReverseIndex<N>, results: &mut MetroHashSet<Board>) {
        if self.state.advance(index, results, BUDGET_FACTOR) {
            self.priority -= 1;
        } else {
            self.priority -= 50;
        }
    }
}

impl<N: MacroboardSize> PartialOrd for WorkItem<N> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<N: MacroboardSize> Ord for WorkItem<N> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // We want to prioritize items with lower sunk cost
        self.priority.cmp(&other.priority)
    }
}

impl<N: MacroboardSize> Eq for WorkItem<N> {}
impl<N: MacroboardSize> PartialEq for WorkItem<N> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

struct WorkQueueInner<N: MacroboardSize> {
    heap: BinaryHeap<WorkItem<N>>,
    item_count: usize,
    terminated: bool,
}

struct WorkQueueState {
    seen_boards: MetroHashSet<(usize, Board)>,
    best_step: usize,
}

impl WorkQueueState {
    fn new() -> Self {
        Self {
            seen_boards: MetroHashSet::default(),
            best_step: 0,
        }
    }
    fn observe(&mut self, step: usize, board: Board) -> bool {
        if step > self.best_step {
            self.best_step = step;
            println!("{:?}", board);
            let mut new_board = board.simulate();
            for _ in 0..step {
                println!("{:?}", new_board);
                new_board = new_board.simulate();
            }
            println!("--------- {} ----------", step);
            println!();
        }
        self.seen_boards.insert((step, board))
    }
}

pub struct WorkQueue<N: MacroboardSize> {
    index: ReverseIndex<N>,
    queue: Mutex<WorkQueueInner<N>>,
    state: Mutex<WorkQueueState>,
    condvar: Condvar,
    terminate_condvar: Condvar,
    target_step: usize,
}

impl<N: MacroboardSize> WorkQueue<N> {
    fn take_item(&self) -> Option<WorkItem<N>> {
        let mut queue = self.queue.lock().unwrap();
        while queue.heap.is_empty() && queue.item_count > 0 && !queue.terminated {
            queue = self.condvar.wait(queue).unwrap();
        }
        if queue.item_count == 0 || queue.terminated {
            return None;
        }
        queue.heap.pop()
    }
    fn add_item(&self, item: WorkItem<N>) {
        let mut queue = self.queue.lock().unwrap();
        queue.heap.push(item);
        queue.item_count += 1;
        self.condvar.notify_one();
    }
    fn complete_item(&self) {
        let mut queue = self.queue.lock().unwrap();
        if queue.item_count > 0 {
            queue.item_count -= 1;
            if queue.item_count == 0 {
                queue.terminated = true;
                self.condvar.notify_all();
                self.terminate_condvar.notify_all();
            }
        }
    }
    fn terminate(&self) {
        let mut queue = self.queue.lock().unwrap();
        queue.terminated = true;
        self.condvar.notify_all();
        self.terminate_condvar.notify_all();
    }
    fn run(&self) {
        while let Some(mut item) = self.take_item() {
            let mut results = MetroHashSet::default();
            item.advance(&self.index, &mut results);
            if !results.is_empty() {
                let mut state = self.state.lock().unwrap();
                results.retain(|board| state.observe(item.step + 1, board.clone()));

                if item.step + 1 == self.target_step {
                    self.terminate();
                    return;
                }
            }

            for result in results {
                self.add_item(WorkItem::new(result.clone(), &self.index, item.step + 1));
            }

            if !item.state.is_done() {
                self.add_item(item);
            }
            self.complete_item();
        }
    }
    pub fn start(initial_boards: Vec<Board>, num_steps: usize, parallel: bool) -> Arc<Self> {
        let queue = Arc::new(WorkQueue::<N> {
            index: ReverseIndex::<N>::compute(),
            queue: Mutex::new(WorkQueueInner {
                heap: BinaryHeap::new(),
                item_count: 0,
                terminated: false,
            }),
            state: Mutex::new(WorkQueueState::new()),
            condvar: Condvar::new(),
            terminate_condvar: Condvar::new(),
            target_step: num_steps,
        });

        for board in initial_boards {
            queue.add_item(WorkItem::new(board, &queue.index, 0));
        }

        if parallel {
            for _ in 0..num_cpus::get() {
                let queue2 = queue.clone();
                thread::spawn(move || {
                    queue2.run();
                });
            }
        } else {
            let queue2 = queue.clone();
            thread::spawn(move || {
                queue2.run();
            });
        }
        queue
    }
    pub fn wait(&self) {
        let mut queue = self.queue.lock().unwrap();
        while queue.item_count > 0 && !queue.terminated {
            queue = self
                .terminate_condvar
                .wait_timeout(queue, Duration::from_secs(5))
                .unwrap()
                .0;
            {
                let state = self.state.lock().unwrap();
                let mut counts = vec![0; state.best_step + 1];
                for (step, _) in &state.seen_boards {
                    counts[*step] += 1;
                }
                println!("    {} active items... {:?}", queue.item_count, counts);
            }
        }
    }
}
