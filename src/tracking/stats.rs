#[derive(Default)]
pub struct MinMax {
    min: Option<f64>,
    max: Option<f64>,
}

impl MinMax {
    pub fn sample(&mut self, x: f64) {
        self.min = Some(self.min.map_or(x, |min| min.min(x)));
        self.max = Some(self.max.map_or(x, |max| max.max(x)));
    }

    pub fn min(&self) -> Option<f64> {
        self.min
    }

    pub fn max(&self) -> Option<f64> {
        self.max
    }
}

#[derive(Default)]
pub struct MeanStdDev {
    count: usize,
    mean: Option<f64>,
    m2: f64,
}

impl MeanStdDev {
    pub fn sample(&mut self, x: f64) {
        self.count += 1;

        match self.mean {
            None => self.mean = Some(x),
            Some(mean) => {
                let delta = x - mean;
                let updated_mean = mean + delta / self.count as f64;
                let delta_after = x - updated_mean;

                self.m2 += delta * delta_after;
                self.mean = Some(updated_mean);
            }
        }
    }

    pub fn mean(&self) -> Option<f64> {
        self.mean
    }

    pub fn std_dev(&self) -> Option<f64> {
        match self.count {
            0 => None,
            _ => Some(f64::sqrt(self.m2 / self.count as f64)),
        }
    }
}

#[derive(Default)]
pub struct MinMaxMeanStdDev {
    min_max: MinMax,
    mean_std_dev: MeanStdDev,
}

impl MinMaxMeanStdDev {
    pub fn sample(&mut self, x: f64) {
        self.min_max.sample(x);
        self.mean_std_dev.sample(x);
    }

    pub fn min(&self) -> Option<f64> {
        self.min_max.min()
    }

    pub fn max(&self) -> Option<f64> {
        self.min_max.max()
    }

    pub fn mean(&self) -> Option<f64> {
        self.mean_std_dev.mean()
    }

    pub fn std_dev(&self) -> Option<f64> {
        self.mean_std_dev.std_dev()
    }
}

#[derive(Default)]
pub struct BaselineDelta {
    baseline: Option<f64>,
    delta: Option<f64>,
}

impl BaselineDelta {
    pub fn sample(&mut self, x: f64) {
        let baseline = *self.baseline.get_or_insert(x);
        self.delta = Some(x - baseline);
    }

    pub fn baseline(&self) -> Option<f64> {
        self.baseline
    }

    pub fn delta(&self) -> Option<f64> {
        self.delta
    }
}

#[derive(Default)]
pub struct LatencyJitter {
    previous: Option<i64>,
    latency: MinMaxMeanStdDev,
}

impl LatencyJitter {
    pub fn sample(&mut self, stamp: i64) {
        if let Some(previous) = self.previous {
            self.latency.sample((stamp - previous) as f64)
        }
        self.previous = Some(stamp);
    }

    pub fn latency_min(&self) -> Option<f64> {
        self.latency.min()
    }

    pub fn latency_max(&self) -> Option<f64> {
        self.latency.max()
    }

    pub fn latency_mean(&self) -> Option<f64> {
        self.latency.mean()
    }

    pub fn jitter(&self) -> Option<f64> {
        self.latency.std_dev()
    }
}
