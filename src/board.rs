use std::fmt::Debug;

use bitvec::{slice::BitSlice, vec::BitVec};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Board {
    bits: BitVec,
    stride: usize,
}

const MIN_SIZE: usize = 4;

impl Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for row in self.iter_rows() {
            for cell in row {
                write!(f, "{}", if *cell { '#' } else { '.' })?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl Board {
    pub fn new(bits: BitVec, stride: usize) -> Self {
        debug_assert!(bits.len() % stride == 0, "Invalid stride for given bits");
        Board { bits, stride }
    }
    fn iter_rows(&self) -> impl Iterator<Item = &BitSlice> {
        self.bits.chunks(self.stride)
    }
    pub fn load(path: &str) -> Result<Vec<Self>, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let mut result = Vec::new();
        for item in content.split("\n\n") {
            let mut board = BitVec::new();
            let mut stride = 0;
            for line in item.lines() {
                board.extend(line.chars().map(|c| c == '#'));
                if stride == 0 {
                    stride = line.len();
                } else {
                    assert!(
                        line.len() == stride,
                        "Inconsistent line length in board file"
                    );
                }
            }
            result.push(Board::new(board, stride));
        }
        Ok(result)
    }
    pub fn width(&self) -> usize {
        self.stride
    }
    pub fn height(&self) -> usize {
        self.bits.len() / self.stride
    }
    pub fn trim(&mut self) {
        let mut w = self.width();
        let mut h = self.height();
        while self.iter_rows().all(|row| !row[w - 1]) && w > MIN_SIZE {
            self.bits.retain(|x, _| x % self.stride != w - 1);
            self.stride -= 1;
            w -= 1;
        }
        while self.iter_rows().all(|row| !row[0]) && w > MIN_SIZE {
            self.bits.retain(|x, _| x % self.stride != 0);
            self.stride -= 1;
            w -= 1;
        }
        while !self.bits[(self.bits.len() - self.stride)..self.bits.len()].any() && h > MIN_SIZE {
            self.bits.truncate(self.bits.len() - self.stride);
            h -= 1;
        }
        while !self.bits[0..self.stride].any() && h > MIN_SIZE {
            self.bits.drain(0..self.stride);
            h -= 1;
        }
    }
    pub fn live_count(&self) -> usize {
        self.bits.count_ones()
    }
    pub fn size(&self) -> usize {
        let y0 = self.bits.first_one().unwrap_or(0) / self.stride;
        let y1 = self.bits.last_one().unwrap_or(0) / self.stride;
        let x0 = self
            .iter_rows()
            .filter_map(|row| row.first_one())
            .min()
            .unwrap_or(0);
        let x1 = self
            .iter_rows()
            .filter_map(|row| row.last_one())
            .max()
            .unwrap_or(0);
        (y1 - y0 + 1) * (x1 - x0 + 1)
    }
    pub fn get(&self, x: usize, y: usize) -> bool {
        if y < self.height() && x < self.width() {
            self.bits[y * self.stride + x]
        } else {
            false
        }
    }
    pub fn simulate(&self) -> Self {
        let mut new_board = BitVec::new();
        for y in 0..self.height() + 2 {
            for x in 0..self.width() + 2 {
                let live_neighbors = [
                    (0, 0),
                    (0, 1),
                    (0, 2),
                    (1, 0),
                    (1, 2),
                    (2, 0),
                    (2, 1),
                    (2, 2),
                ]
                .into_iter()
                .map(|(dx, dy)| self.get(x.wrapping_sub(dx), y.wrapping_sub(dy)) as usize)
                .sum::<usize>();
                new_board.push(
                    live_neighbors == 3
                        || (self.get(x.wrapping_sub(1), y.wrapping_sub(1)) && live_neighbors == 2),
                );
            }
        }
        let mut result = Self {
            bits: new_board,
            stride: self.stride + 2,
        };
        result.trim();
        result
    }
}
