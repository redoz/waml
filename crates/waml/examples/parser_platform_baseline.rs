//! Reproducible, report-only parser characterization for the checked-in corpus.

use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct CountingAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let replacement = unsafe { System.realloc(pointer, layout, size) };
        if !replacement.is_null() {
            if size >= layout.size() {
                let delta = size - layout.size();
                let live = LIVE.fetch_add(delta, Ordering::Relaxed) + delta;
                ALLOCATED.fetch_add(delta, Ordering::Relaxed);
                PEAK.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE.fetch_sub(layout.size() - size, Ordering::Relaxed);
            }
        }
        replacement
    }
}

fn fixture_names(method: &str) -> Vec<String> {
    let start = method.find("\"fixtures\"").expect("method fixtures") ;
    let body = &method[start..];
    let open = body.find('[').expect("fixtures array");
    let close = body[open..].find(']').expect("fixtures close") + open;
    body[open + 1..close]
        .split(',')
        .map(|entry| entry.trim().trim_matches('"').to_owned())
        .filter(|entry| !entry.is_empty())
        .collect()
}

fn fnv1a64(fixtures: &[(String, String)]) -> String {
    let mut value = 0xcbf29ce484222325u64;
    for (path, source) in fixtures {
        for byte in path.bytes().chain([0]).chain(source.bytes()).chain([0xff]) {
            value ^= u64::from(byte);
            value = value.wrapping_mul(0x100000001b3);
        }
    }
    format!("{value:016x}")
}

fn percentile(samples: &mut [u128], numerator: usize, denominator: usize) -> u128 {
    samples.sort_unstable();
    samples[(samples.len() - 1) * numerator / denominator]
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let method_at = args.iter().position(|arg| arg == "--method").expect("--method PATH") + 1;
    let method_path = PathBuf::from(&args[method_at]);
    let method = fs::read_to_string(&method_path).expect("read method record");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parser-platform");
    let fixtures: Vec<_> = fixture_names(&method).into_iter().map(|name| {
        let source = fs::read_to_string(root.join(&name)).expect("read checked-in fixture");
        (name, source)
    }).collect();
    let corpus_identity = fnv1a64(&fixtures);

    for _ in 0..5 { for (_, source) in &fixtures { let _ = waml::parse::parse_document(source); } }
    let mut samples = Vec::with_capacity(30);
    let live_before = LIVE.load(Ordering::Relaxed);
    PEAK.store(live_before, Ordering::Relaxed);
    ALLOCATED.store(0, Ordering::Relaxed);
    ALLOCATIONS.store(0, Ordering::Relaxed);
    for _ in 0..30 {
        let started = Instant::now();
        for (_, source) in &fixtures { let _ = waml::parse::parse_document(source); }
        samples.push(started.elapsed().as_nanos());
    }
    let mut ordered = samples.clone();
    let median = percentile(&mut ordered, 1, 2);
    let mut deviations: Vec<_> = samples.iter().map(|sample| sample.abs_diff(median)).collect();
    let mad = percentile(&mut deviations, 1, 2);
    let mut p95_samples = samples.clone();
    let p95 = percentile(&mut p95_samples, 95, 100);
    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(live_before);
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let cpu_count = std::thread::available_parallelism().map(|count| count.get()).unwrap_or(1);
    let rustc = option_env!("RUSTC_VERSION").unwrap_or("rustc-unrecorded");
    let json = format!(
        concat!("{{\n", "  \"allocation_count\": {allocations},\n", "  \"allocated_bytes\": {allocated},\n", "  \"corpus_identity\": \"{corpus}\",\n", "  \"hardware\": {{\"arch\":\"{arch}\",\"cpu_count\":{cpu_count},\"os\":\"{os}\",\"rustc\":\"{rustc}\"}},\n", "  \"parse_nanoseconds_mad\": {mad},\n", "  \"parse_nanoseconds_median\": {median},\n", "  \"parse_nanoseconds_p95\": {p95},\n", "  \"peak_live_bytes\": {peak}\n", "}}\n"),
        allocations = ALLOCATIONS.load(Ordering::Relaxed), allocated = ALLOCATED.load(Ordering::Relaxed), corpus = corpus_identity, arch = arch, cpu_count = cpu_count, os = os, rustc = rustc, mad = mad, median = median, p95 = p95, peak = peak);
    if let Some(index) = args.iter().position(|arg| arg == "--record") { fs::write(&args[index + 1], &json).expect("write observation"); }
    if let Some(index) = args.iter().position(|arg| arg == "--compare") {
        let prior = fs::read_to_string(&args[index + 1]).expect("read prior observation");
        if !prior.contains(&format!("\"corpus_identity\": \"{corpus_identity}\"")) || !prior.contains(&format!("\"os\":\"{os}\"")) || !prior.contains(&format!("\"arch\":\"{arch}\"")) { println!("LATENCY_SKIPPED_HARDWARE_MISMATCH"); }
        else { println!("LATENCY_REPORT_ONLY current_median_ns={median}"); }
    } else { print!("{json}"); }
}
