use std::{
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

fn compute_priority(step: usize, live_count: usize, _size: usize, _score: usize) -> isize {
    (step as isize + 10) * 20 - (live_count as isize)
}

impl<N: MacroboardSize> WorkItem<N> {
    fn new(board: Board, index: &ReverseIndex<N>, step: usize) -> Self {
        let state = State::new(&board, index);
        Self {
            priority: compute_priority(step, board.live_count(), board.size(), state.score(index)),
            state,
            step,
        }
    }
    fn advance(&mut self, index: &ReverseIndex<N>, results: &mut MetroHashSet<Board>) {
        if self.state.advance(
            index,
            results,
            BUDGET_FACTOR * (self.step + 1) * (self.step + 1),
        ) {
            self.priority += 1;
        } else {
            self.priority -= 15;
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

const MAX_LIST_LEN: usize = 1000;

#[derive(Default)]
struct PriorityQueue<N: MacroboardSize> {
    items: Vec<Vec<WorkItem<N>>>,
}

impl<N: MacroboardSize> PriorityQueue<N> {
    fn push(&mut self, item: WorkItem<N>) -> bool {
        while self.items.len() <= item.step {
            self.items.push(Vec::new());
        }
        let list = &mut self.items[item.step];
        list.push(item);
        list.sort();
        if list.len() > MAX_LIST_LEN {
            list.remove(0);
            false
        } else {
            true
        }
    }
    fn pop(&mut self) -> Option<WorkItem<N>> {
        if let Some((_, idx)) = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                item.last().map(|x| {
                    (
                        x.priority - self.items.get(i + 1).map(|v| v.len()).unwrap_or(0) as isize,
                        i,
                    )
                })
            })
            .max_by_key(|x| x.0)
        {
            self.items[idx].pop()
        } else {
            None
        }
    }
    fn is_empty(&self) -> bool {
        self.items.iter().all(|list| list.is_empty())
    }
}

struct WorkQueueInner<N: MacroboardSize> {
    heap: PriorityQueue<N>,
    item_count: usize,
    processed_count: usize,
    terminated: bool,
}

struct WorkQueueState {
    seen_boards: MetroHashSet<(usize, Board)>,
    completed_counts: Vec<usize>,
    best_step: usize,
}

impl WorkQueueState {
    fn new() -> Self {
        Self {
            seen_boards: MetroHashSet::default(),
            completed_counts: Vec::new(),
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
        if queue.heap.push(item) {
            queue.item_count += 1;
        }
        self.condvar.notify_one();
    }
    fn record_completed(&self, step: usize) {
        let mut state = self.state.lock().unwrap();
        if step >= state.completed_counts.len() {
            state.completed_counts.resize(step + 1, 0);
        }
        state.completed_counts[step] += 1;
    }
    fn complete_item(&self) {
        let mut queue = self.queue.lock().unwrap();
        queue.processed_count += 1;
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

            if item.state.is_done() {
                self.record_completed(item.step);
            } else {
                self.add_item(item);
            }
            self.complete_item();
        }
    }
    pub fn start(initial_boards: Vec<Board>, num_steps: usize, parallel: bool) -> Arc<Self> {
        let queue = Arc::new(WorkQueue::<N> {
            index: ReverseIndex::<N>::compute(),
            queue: Mutex::new(WorkQueueInner {
                heap: PriorityQueue::default(),
                item_count: 0,
                processed_count: 0,
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
                let queue_counts = queue
                    .heap
                    .items
                    .iter()
                    .map(|list| list.len())
                    .collect::<Vec<_>>();
                let priorities = queue
                    .heap
                    .items
                    .iter()
                    .map(|list| list.last().map_or(0, |item| item.priority))
                    .collect::<Vec<_>>();
                println!(
                    "{} active items... ({} processed) \n    Queue: {:?}\n    Priorities: {:?}\n    Found: {:?}\n    Complete: {:?}\n",
                    queue.item_count,
                    queue.processed_count,
                    queue_counts,
                    priorities,
                    counts,
                    state.completed_counts
                );
            }
        }
    }
}
