use config::configuration;
use config::lua::get_or_create_sub_module;
use config::lua::mlua::Lua;
use hdrhistogram::Histogram;
use metrics::{Counter, Gauge, Key, KeyName, Metadata, Recorder, SharedString, Unit};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tabout::{tabulate_output, Alignment, Column};

/// Like `metrics::histogram!` with a literal name, but looks the handle up
/// in the recorder only once per call site. `histogram!` does that lookup
/// (a lock and a map search) on every call, which adds up in code that
/// runs per glyph or per line. Only use it after `Stats::init`: a handle
/// looked up before the recorder is installed stays a no-op.
macro_rules! cached_histogram {
    ($name:literal) => {{
        static HANDLE: std::sync::LazyLock<metrics::Histogram> =
            std::sync::LazyLock::new(|| metrics::histogram!($name));
        &*HANDLE
    }};
}
pub(crate) use cached_histogram;

static ENABLE_STAT_PRINT: AtomicBool = AtomicBool::new(true);
lazy_static::lazy_static! {
    static ref INNER: Arc<Mutex<Inner>> = make_inner();
}

/// Counts events per second, for metrics named `*.rate`.
///
/// Cache hits and misses record into these on every lookup, many times a
/// frame, so `record` only adds to the count. It used to read the clock on
/// every call to see whether the one-second window had ended, which cost
/// about 7ms of main thread time per second while output scrolls. The
/// stats thread now closes the window once a second instead (`roll`).
struct ThroughputInner {
    hist: Histogram<u64>,
    /// Events in the window that is still open.
    count: u64,
    /// Events in the last window that closed.
    last_window: u64,
}

struct Throughput {
    inner: Mutex<ThroughputInner>,
}

impl Throughput {
    fn new() -> Self {
        Self {
            inner: Mutex::new(ThroughputInner {
                hist: Histogram::new(2).expect("failed to create histogram"),
                count: 0,
                last_window: 0,
            }),
        }
    }

    /// Events in the last full second.
    fn current(&self) -> u64 {
        self.inner.lock().last_window
    }

    /// Closes the current one-second window. Windows with no events are
    /// not recorded, so idle time doesn't drag the percentiles to zero.
    fn roll(&self) {
        let mut inner = self.inner.lock();
        let count = std::mem::take(&mut inner.count);
        inner.last_window = count;
        if count > 0 {
            inner.hist.record(count).ok();
        }
    }

    fn percentiles(&self) -> (u64, u64, u64) {
        let inner = self.inner.lock();
        let p50 = inner.hist.value_at_percentile(50.);
        let p75 = inner.hist.value_at_percentile(75.);
        let p95 = inner.hist.value_at_percentile(95.);
        (p50, p75, p95)
    }
}

impl metrics::HistogramFn for Throughput {
    fn record(&self, value: f64) {
        self.inner.lock().count += value as u64;
    }
}

struct ScaledHistogram {
    hist: Mutex<Histogram<u64>>,
    scale: f64,
}

impl ScaledHistogram {
    fn new(scale: f64) -> Arc<Self> {
        Arc::new(Self {
            hist: Mutex::new(Histogram::new(2).expect("failed to create new Histogram")),
            scale,
        })
    }
    fn percentiles(&self) -> (u64, u64, u64) {
        let hist = self.hist.lock();
        let p50 = hist.value_at_percentile(50.);
        let p75 = hist.value_at_percentile(75.);
        let p95 = hist.value_at_percentile(95.);
        (p50, p75, p95)
    }

    fn latency_percentiles(&self) -> (Duration, Duration, Duration) {
        let hist = self.hist.lock();
        let p50 = pctile_latency(&*hist, 50.);
        let p75 = pctile_latency(&*hist, 75.);
        let p95 = pctile_latency(&*hist, 95.);
        (p50, p75, p95)
    }
}

impl metrics::HistogramFn for ScaledHistogram {
    fn record(&self, value: f64) {
        self.hist.lock().record((value * self.scale) as u64).ok();
    }
}

fn pctile_latency(histogram: &Histogram<u64>, p: f64) -> Duration {
    Duration::from_nanos(histogram.value_at_percentile(p))
}

struct MyCounter {
    value: AtomicUsize,
}

impl metrics::CounterFn for MyCounter {
    fn increment(&self, value: u64) {
        self.value.fetch_add(value as usize, Ordering::Relaxed);
    }

    fn absolute(&self, value: u64) {
        self.value.store(value as usize, Ordering::Relaxed);
    }
}

struct Inner {
    histograms: HashMap<Key, Arc<ScaledHistogram>>,
    throughput: HashMap<Key, Arc<Throughput>>,
    counters: HashMap<Key, Arc<MyCounter>>,
}

impl Inner {
    fn run(inner: Arc<Mutex<Inner>>) {
        let mut last_print = Instant::now();

        let rate_cols = vec![
            Column {
                name: "STAT".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "current".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p50".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p75".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p95".to_string(),
                alignment: Alignment::Left,
            },
        ];
        let cols = vec![
            Column {
                name: "STAT".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p50".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p75".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p95".to_string(),
                alignment: Alignment::Left,
            },
        ];
        let count_cols = vec![
            Column {
                name: "STAT".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "COUNT".to_string(),
                alignment: Alignment::Left,
            },
        ];

        loop {
            std::thread::sleep(Duration::from_secs(1));

            if !ENABLE_STAT_PRINT.load(Ordering::Acquire) {
                break;
            }

            for tput in inner.lock().throughput.values() {
                tput.roll();
            }

            let seconds = configuration().periodic_stat_logging;
            if seconds == 0 {
                continue;
            }
            if last_print.elapsed() >= Duration::from_secs(seconds) {
                let mut data = vec![];

                let inner = inner.lock();
                for (key, tput) in &inner.throughput {
                    let current = tput.current();
                    let (p50, p75, p95) = tput.percentiles();
                    data.push(vec![
                        key.to_string(),
                        format!("{:.2?}", current),
                        format!("{:.2?}", p50),
                        format!("{:.2?}", p75),
                        format!("{:.2?}", p95),
                    ]);
                }
                data.sort_by(|a, b| a[0].cmp(&b[0]));
                eprintln!();
                tabulate_output(&rate_cols, &data, &mut std::io::stderr().lock()).ok();

                data.clear();
                for (key, histogram) in &inner.histograms {
                    if key.name().ends_with(".size") {
                        let (p50, p75, p95) = histogram.percentiles();
                        data.push(vec![
                            key.to_string(),
                            format!("{:.2?}", p50),
                            format!("{:.2?}", p75),
                            format!("{:.2?}", p95),
                        ]);
                    } else {
                        let (p50, p75, p95) = histogram.latency_percentiles();
                        data.push(vec![
                            key.to_string(),
                            format!("{:.2?}", p50),
                            format!("{:.2?}", p75),
                            format!("{:.2?}", p95),
                        ]);
                    }
                }
                data.sort_by(|a, b| a[0].cmp(&b[0]));
                eprintln!();
                tabulate_output(&cols, &data, &mut std::io::stderr().lock()).ok();

                data.clear();
                for (key, count) in &inner.counters {
                    data.push(vec![
                        key.to_string(),
                        count.value.load(Ordering::Relaxed).to_string(),
                    ]);
                }
                data.sort_by(|a, b| a[0].cmp(&b[0]));
                eprintln!();
                tabulate_output(&count_cols, &data, &mut std::io::stderr().lock()).ok();

                last_print = Instant::now();
            }
        }
    }
}

fn make_inner() -> Arc<Mutex<Inner>> {
    Arc::new(Mutex::new(Inner {
        histograms: HashMap::new(),
        throughput: HashMap::new(),
        counters: HashMap::new(),
    }))
}

pub struct Stats {
    inner: Arc<Mutex<Inner>>,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            inner: Arc::clone(&INNER),
        }
    }

    pub fn init() -> anyhow::Result<()> {
        let stats = Self::new();
        let inner = Arc::clone(&stats.inner);
        std::thread::spawn(move || Inner::run(inner));
        metrics::set_global_recorder(stats)
            .map_err(|e| anyhow::anyhow!("Failed to set metrics recorder:{}", e))
    }
}

impl Recorder for Stats {
    fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn register_counter(&self, key: &Key, _metadata: &Metadata) -> Counter {
        let mut inner = self.inner.lock();
        match inner.counters.get(key) {
            Some(existing) => Counter::from_arc(existing.clone()),
            None => {
                let counter = Arc::new(MyCounter {
                    value: AtomicUsize::new(0),
                });
                inner.counters.insert(key.clone(), counter.clone());
                metrics::Counter::from_arc(counter)
            }
        }
    }

    fn register_gauge(&self, _key: &Key, _metadata: &Metadata) -> Gauge {
        Gauge::noop()
    }

    fn register_histogram(&self, key: &Key, _metadata: &Metadata) -> metrics::Histogram {
        let mut inner = self.inner.lock();
        if key.name().ends_with(".rate") {
            match inner.throughput.get(key) {
                Some(existing) => metrics::Histogram::from_arc(existing.clone()),
                None => {
                    let tput = Arc::new(Throughput::new());
                    inner.throughput.insert(key.clone(), tput.clone());

                    metrics::Histogram::from_arc(tput)
                }
            }
        } else {
            match inner.histograms.get(key) {
                Some(existing) => metrics::Histogram::from_arc(existing.clone()),
                None => {
                    let scale = if key.name().ends_with(".size") {
                        1.0
                    } else {
                        // Assume seconds; convert to nanoseconds
                        1_000_000_000.0
                    };

                    let histogram = ScaledHistogram::new(scale);
                    inner.histograms.insert(key.clone(), histogram.clone());

                    metrics::Histogram::from_arc(histogram)
                }
            }
        }
    }
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let metrics_mod = get_or_create_sub_module(lua, "metrics")?;
    metrics_mod.set(
        "get_counters",
        lua.create_function(|_, _: ()| {
            let inner = INNER.lock();
            let counters: HashMap<String, usize> = inner
                .counters
                .iter()
                .map(|(k, v)| (k.name().to_string(), v.value.load(Ordering::Relaxed)))
                .collect();
            Ok(counters)
        })?,
    )?;
    metrics_mod.set(
        "get_throughput",
        lua.create_function(|_, _: ()| {
            let inner = INNER.lock();
            let counters: HashMap<String, HashMap<String, u64>> = inner
                .throughput
                .iter()
                .map(|(k, tput)| {
                    let mut res = HashMap::new();
                    res.insert("current".to_string(), tput.current());
                    let (p50, p75, p95) = tput.percentiles();
                    res.insert("p50".to_string(), p50);
                    res.insert("p75".to_string(), p75);
                    res.insert("p95".to_string(), p95);
                    (k.name().to_string(), res)
                })
                .collect();
            Ok(counters)
        })?,
    )?;
    metrics_mod.set(
        "get_sizes",
        lua.create_function(|_, _: ()| {
            let mut inner = INNER.lock();
            let counters: HashMap<String, HashMap<String, u64>> = inner
                .histograms
                .iter_mut()
                .filter_map(|(key, hist)| {
                    if key.name().ends_with(".size") {
                        let mut res = HashMap::new();
                        let (p50, p75, p95) = hist.percentiles();
                        res.insert("p50".to_string(), p50);
                        res.insert("p75".to_string(), p75);
                        res.insert("p95".to_string(), p95);
                        Some((key.name().to_string(), res))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(counters)
        })?,
    )?;
    metrics_mod.set(
        "get_latency",
        lua.create_function(|_, _: ()| {
            let mut inner = INNER.lock();
            let counters: HashMap<String, HashMap<String, String>> = inner
                .histograms
                .iter_mut()
                .filter_map(|(key, hist)| {
                    if !key.name().ends_with(".size") {
                        let mut res = HashMap::new();
                        let (p50, p75, p95) = hist.latency_percentiles();
                        res.insert("p50".to_string(), format!("{p50:?}"));
                        res.insert("p75".to_string(), format!("{p75:?}"));
                        res.insert("p95".to_string(), format!("{p95:?}"));
                        Some((key.name().to_string(), res))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(counters)
        })?,
    )?;
    Ok(())
}
