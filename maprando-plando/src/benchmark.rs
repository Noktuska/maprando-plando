use std::time::{Duration, Instant};

use hashbrown::HashMap;

pub struct Benchmark {
    splits: HashMap<String, Duration>,
    timer: Instant,
    total_timer: Instant,
}

pub struct BenchmarkResult {
    pub splits: HashMap<String, Duration>,
    pub total_time: Duration,
    pub empty_time: Duration
}

impl Benchmark {
    pub fn new() -> Self {
        Benchmark {
            splits: HashMap::new(),
            timer: Instant::now(),
            total_timer: Instant::now()
        }
    }

    pub fn start(&mut self) {
        for (_, dur) in self.splits.iter_mut() {
            *dur = Duration::ZERO;
        }
        self.total_timer = Instant::now();
        self.timer = Instant::now();
    }

    pub fn split<S: Into<String>>(&mut self, label: S) {
        let dur = self.timer.elapsed();
        self.splits.insert(label.into(), dur);
        self.timer = Instant::now();
    }

    pub fn evaluate(&mut self) -> BenchmarkResult {
        let dur = self.total_timer.elapsed();
        let collected_dur = self.splits.iter().map(|x| x.1.clone()).sum::<Duration>();
        let empty_time = if collected_dur >= dur {
            Duration::ZERO
        } else {
            dur - collected_dur
        };
        BenchmarkResult {
            splits: self.splits.clone(),
            total_time: dur,
            empty_time
        }
    }
}