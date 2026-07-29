//! Reproducible, report-only parser characterization for the checked-in corpus.

use serde_json::{json, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const REQUIRED_MEASUREMENTS: [&str; 6] = [
    "parse_nanoseconds_median",
    "parse_nanoseconds_p95",
    "parse_nanoseconds_mad",
    "peak_live_bytes",
    "allocated_bytes",
    "allocation_count",
];
const REQUIRED_HARDWARE: [&str; 4] = ["os", "arch", "rustc", "cpu_count"];

struct CountingAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Debug, PartialEq, Eq)]
struct ReallocationAccounting {
    live_increase: usize,
    live_decrease: usize,
    allocated_bytes: usize,
    allocation_count: usize,
}

fn reallocation_accounting(old_size: usize, new_size: usize) -> ReallocationAccounting {
    ReallocationAccounting {
        live_increase: new_size.saturating_sub(old_size),
        live_decrease: old_size.saturating_sub(new_size),
        // A successful realloc is one allocation request for `new_size` bytes,
        // regardless of whether it grows or shrinks the live block.
        allocated_bytes: new_size,
        allocation_count: 1,
    }
}

fn record_live_increase(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_live_increase(layout.size());
            ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
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
            let accounting = reallocation_accounting(layout.size(), size);
            if accounting.live_increase != 0 {
                record_live_increase(accounting.live_increase);
            }
            if accounting.live_decrease != 0 {
                LIVE.fetch_sub(accounting.live_decrease, Ordering::Relaxed);
            }
            ALLOCATED.fetch_add(accounting.allocated_bytes, Ordering::Relaxed);
            ALLOCATIONS.fetch_add(accounting.allocation_count, Ordering::Relaxed);
        }
        replacement
    }
}

#[derive(Debug)]
struct Method {
    enforcement: String,
    fixtures: Vec<String>,
    expected_corpus_identity: String,
    warmup_runs: usize,
    sample_runs: usize,
    hardware_fields: Vec<String>,
}

fn string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing or invalid string field '{key}'"))
}

fn usize_field(value: &Value, key: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| format!("missing or invalid integer field '{key}'"))
}

fn string_array(value: &Value, key: &str) -> Result<Vec<String>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing or invalid array field '{key}'"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("non-string item in '{key}'"))
        })
        .collect()
}

impl Method {
    fn parse(source: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(source)
            .map_err(|error| format!("invalid method JSON: {error}"))?;
        if string(&value, "method")? != "parser-platform-post-okf" {
            return Err("unexpected method identifier".into());
        }
        let enforcement = string(&value, "enforcement")?;
        if enforcement != "report-only" {
            return Err("enforcement must be 'report-only'".into());
        }
        let warmup_runs = usize_field(&value, "warmup_runs")?;
        let sample_runs = usize_field(&value, "sample_runs")?;
        if warmup_runs != 5 || sample_runs != 30 {
            return Err("method requires exactly 5 warmups and 30 samples".into());
        }
        let fixtures = string_array(&value, "fixtures")?;
        if fixtures.is_empty()
            || fixtures
                .iter()
                .any(|path| path != &path.replace('\\', "/") || Path::new(path).is_absolute())
        {
            return Err("fixtures must be non-empty normalized relative paths".into());
        }
        let corpus = value
            .get("corpus_identity")
            .ok_or_else(|| "missing corpus_identity".to_string())?;
        for (field, expected) in [
            ("algorithm", "fnv1a-64"),
            ("offset_basis", "cbf29ce484222325"),
            ("prime", "00000100000001b3"),
            ("path_separator", "00"),
            ("source_separator", "ff"),
        ] {
            if string(corpus, field)? != expected {
                return Err(format!("corpus_identity.{field} must be '{expected}'"));
            }
        }
        let expected_corpus_identity = string(corpus, "expected")?;
        if expected_corpus_identity.len() != 16
            || !expected_corpus_identity
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("corpus_identity.expected must be 16 lowercase hex digits".into());
        }
        let measurements = string_array(&value, "measurement_fields")?;
        if measurements != REQUIRED_MEASUREMENTS {
            return Err("measurement_fields do not match the required stable order".into());
        }
        let hardware_fields = string_array(&value, "hardware_fingerprint_fields")?;
        if hardware_fields != REQUIRED_HARDWARE {
            return Err(
                "hardware_fingerprint_fields do not match the required stable order".into(),
            );
        }
        Ok(Method {
            enforcement,
            fixtures,
            expected_corpus_identity,
            warmup_runs,
            sample_runs,
            hardware_fields,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Hardware {
    os: String,
    arch: String,
    rustc: String,
    cpu_count: usize,
}

impl Hardware {
    fn current() -> Result<Self, String> {
        let rustc_program = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(&rustc_program)
            .arg("-Vv")
            .output()
            .map_err(|error| format!("run {} -Vv: {error}", rustc_program.to_string_lossy()))?;
        if !output.status.success() {
            return Err(format!(
                "{} -Vv exited unsuccessfully",
                rustc_program.to_string_lossy()
            ));
        }
        let verbose = String::from_utf8(output.stdout)
            .map_err(|error| format!("rustc version is not UTF-8: {error}"))?
            .trim()
            .to_owned();
        if verbose.is_empty() {
            return Err("rustc -Vv returned an empty version".into());
        }
        let rustc = format!("program: {}\n{verbose}", rustc_program.to_string_lossy());
        Ok(Hardware {
            os: env::consts::OS.to_owned(),
            arch: env::consts::ARCH.to_owned(),
            rustc,
            cpu_count: std::thread::available_parallelism()
                .map(|count| count.get())
                .unwrap_or(1),
        })
    }

    fn from_value(value: &Value) -> Result<Self, String> {
        Ok(Hardware {
            os: string(value, "os")?,
            arch: string(value, "arch")?,
            rustc: string(value, "rustc")?,
            cpu_count: usize_field(value, "cpu_count")?,
        })
    }

    fn json(&self) -> Value {
        json!({
            "arch": self.arch,
            "cpu_count": self.cpu_count,
            "os": self.os,
            "rustc": self.rustc,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Observation {
    allocation_count: usize,
    allocated_bytes: usize,
    corpus_identity: String,
    hardware: Hardware,
    parse_nanoseconds_mad: u128,
    parse_nanoseconds_median: u128,
    parse_nanoseconds_p95: u128,
    peak_live_bytes: usize,
}

fn u128_field(value: &Value, key: &str) -> Result<u128, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map(u128::from)
        .ok_or_else(|| format!("missing or invalid integer field '{key}'"))
}

impl Observation {
    fn parse(source: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(source)
            .map_err(|error| format!("invalid observation JSON: {error}"))?;
        if string(&value, "allocator_scope")? != "process" {
            return Err("allocator_scope must be 'process'".into());
        }
        let observation = Observation {
            allocation_count: usize_field(&value, "allocation_count")?,
            allocated_bytes: usize_field(&value, "allocated_bytes")?,
            corpus_identity: string(&value, "corpus_identity")?,
            hardware: Hardware::from_value(
                value
                    .get("hardware")
                    .ok_or_else(|| "missing hardware".to_string())?,
            )?,
            parse_nanoseconds_mad: u128_field(&value, "parse_nanoseconds_mad")?,
            parse_nanoseconds_median: u128_field(&value, "parse_nanoseconds_median")?,
            parse_nanoseconds_p95: u128_field(&value, "parse_nanoseconds_p95")?,
            peak_live_bytes: usize_field(&value, "peak_live_bytes")?,
        };
        if observation.allocation_count == 0
            || observation.allocated_bytes == 0
            || observation.parse_nanoseconds_median == 0
            || observation.parse_nanoseconds_p95 == 0
            || observation.peak_live_bytes == 0
        {
            return Err("observation measurements must be non-zero".into());
        }
        Ok(observation)
    }

    fn json(&self) -> String {
        serde_json::to_string_pretty(&json!({
            "allocation_count": self.allocation_count,
            "allocated_bytes": self.allocated_bytes,
            "allocator_scope": "process",
            "corpus_identity": self.corpus_identity,
            "hardware": self.hardware.json(),
            "parse_nanoseconds_mad": self.parse_nanoseconds_mad,
            "parse_nanoseconds_median": self.parse_nanoseconds_median,
            "parse_nanoseconds_p95": self.parse_nanoseconds_p95,
            "peak_live_bytes": self.peak_live_bytes,
        }))
        .expect("observation fields serialize")
            + "\n"
    }

    #[cfg(test)]
    fn test_sample(
        hardware: Hardware,
        median: u128,
        p95: u128,
        mad: u128,
        peak: usize,
        allocated: usize,
        allocations: usize,
    ) -> Self {
        Observation {
            allocation_count: allocations,
            allocated_bytes: allocated,
            corpus_identity: "0123456789abcdef".into(),
            hardware,
            parse_nanoseconds_mad: mad,
            parse_nanoseconds_median: median,
            parse_nanoseconds_p95: p95,
            peak_live_bytes: peak,
        }
    }
}

fn signed_delta(current: u128, prior: u128) -> i128 {
    if current >= prior {
        i128::try_from(current - prior).unwrap_or(i128::MAX)
    } else {
        -i128::try_from(prior - current).unwrap_or(i128::MAX)
    }
}

fn comparison_report(current: &Observation, prior: &Observation) -> String {
    if current.corpus_identity != prior.corpus_identity {
        return "LATENCY_SKIPPED_CORPUS_MISMATCH".into();
    }
    if current.hardware != prior.hardware {
        return "LATENCY_SKIPPED_HARDWARE_MISMATCH".into();
    }
    format!(
        "median_ns_delta={} p95_ns_delta={} mad_ns_delta={} peak_live_bytes_delta={} allocated_bytes_delta={} allocation_count_delta={} LATENCY_REPORT_ONLY",
        signed_delta(
            current.parse_nanoseconds_median,
            prior.parse_nanoseconds_median
        ),
        signed_delta(current.parse_nanoseconds_p95, prior.parse_nanoseconds_p95),
        signed_delta(current.parse_nanoseconds_mad, prior.parse_nanoseconds_mad),
        signed_delta(
            current.peak_live_bytes as u128,
            prior.peak_live_bytes as u128
        ),
        signed_delta(
            current.allocated_bytes as u128,
            prior.allocated_bytes as u128
        ),
        signed_delta(
            current.allocation_count as u128,
            prior.allocation_count as u128
        ),
    )
}

fn compare_prior(current: &Observation, source: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(source)
        .map_err(|error| format!("invalid observation JSON: {error}"))?;
    let Some(hardware) = value.get("hardware") else {
        return Ok("LATENCY_SKIPPED_HARDWARE_MISMATCH".into());
    };
    let Ok(prior_hardware) = Hardware::from_value(hardware) else {
        return Ok("LATENCY_SKIPPED_HARDWARE_MISMATCH".into());
    };
    if prior_hardware != current.hardware {
        return Ok("LATENCY_SKIPPED_HARDWARE_MISMATCH".into());
    }
    let prior = Observation::parse(source)?;
    Ok(comparison_report(current, &prior))
}

fn compare_if_present(prior: &Path, current: &Path) -> Result<String, String> {
    let current_source = fs::read_to_string(current)
        .map_err(|error| format!("read current observation {}: {error}", current.display()))?;
    let current = Observation::parse(&current_source)?;
    let prior_source = match fs::read_to_string(prior) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok("LATENCY_SKIPPED_BASELINE_ABSENT".into());
        }
        Err(error) => return Err(format!("read prior observation {}: {error}", prior.display())),
    };
    compare_prior(&current, &prior_source)
}

fn fnv1a64(fixtures: &[(String, String)]) -> String {
    let mut value = FNV_OFFSET;
    for (path, source) in fixtures {
        for byte in path
            .replace('\\', "/")
            .bytes()
            .chain([0])
            .chain(source.bytes())
            .chain([0xff])
        {
            value ^= u64::from(byte);
            value = value.wrapping_mul(FNV_PRIME);
        }
    }
    format!("{value:016x}")
}

fn percentile(samples: &mut [u128], numerator: usize, denominator: usize) -> u128 {
    samples.sort_unstable();
    samples[(samples.len() - 1) * numerator / denominator]
}

fn analyze_once(source: &str) {
    let bundle =
        waml::source::SourceBundle::try_from_pairs([("benchmark.md", source.to_owned())]).unwrap();
    let _ = waml::analysis::prepare_candidate(bundle, None, 0);
}

fn measure(method: &Method, fixtures: &[(String, String)]) -> Result<Observation, String> {
    for _ in 0..method.warmup_runs {
        for (_, source) in fixtures {
            analyze_once(source);
        }
    }
    let mut samples = Vec::with_capacity(method.sample_runs);
    let live_before = LIVE.load(Ordering::Relaxed);
    PEAK.store(live_before, Ordering::Relaxed);
    ALLOCATED.store(0, Ordering::Relaxed);
    ALLOCATIONS.store(0, Ordering::Relaxed);
    for _ in 0..method.sample_runs {
        let started = Instant::now();
        for (_, source) in fixtures {
            analyze_once(source);
        }
        samples.push(started.elapsed().as_nanos());
    }
    let allocated_bytes = ALLOCATED.load(Ordering::Relaxed);
    let allocation_count = ALLOCATIONS.load(Ordering::Relaxed);
    let peak_live_bytes = PEAK.load(Ordering::Relaxed);
    let mut ordered = samples.clone();
    let median = percentile(&mut ordered, 1, 2);
    let mut deviations: Vec<_> = samples
        .iter()
        .map(|sample| sample.abs_diff(median))
        .collect();
    let mad = percentile(&mut deviations, 1, 2);
    let mut p95_samples = samples;
    let p95 = percentile(&mut p95_samples, 95, 100);
    Ok(Observation {
        allocation_count,
        allocated_bytes,
        corpus_identity: fnv1a64(fixtures),
        hardware: Hardware::current()?,
        parse_nanoseconds_mad: mad,
        parse_nanoseconds_median: median,
        parse_nanoseconds_p95: p95,
        peak_live_bytes,
    })
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let method_at = args
        .iter()
        .position(|arg| arg == "--method")
        .ok_or_else(|| "missing --method PATH".to_string())?
        + 1;
    let method_path = PathBuf::from(
        args.get(method_at)
            .ok_or_else(|| "missing --method PATH".to_string())?,
    );
    let method = Method::parse(
        &fs::read_to_string(&method_path)
            .map_err(|error| format!("read method record {}: {error}", method_path.display()))?,
    )?;
    debug_assert_eq!(method.enforcement, "report-only");
    debug_assert_eq!(method.hardware_fields, REQUIRED_HARDWARE);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parser-platform");
    let fixtures: Vec<_> = method
        .fixtures
        .iter()
        .map(|name| {
            let bytes = fs::read(root.join(name))
                .map_err(|error| format!("read fixture {name}: {error}"))?;
            let source =
                String::from_utf8(bytes).map_err(|error| format!("fixture {name}: {error}"))?;
            Ok((name.clone(), source))
        })
        .collect::<Result<_, String>>()?;
    let actual_identity = fnv1a64(&fixtures);
    if actual_identity != method.expected_corpus_identity {
        return Err(format!(
            "corpus identity mismatch: method={} actual={actual_identity}",
            method.expected_corpus_identity
        ));
    }
    let observation = measure(&method, &fixtures)?;
    let json = observation.json();
    if let Some(index) = args.iter().position(|arg| arg == "--record") {
        let path = args
            .get(index + 1)
            .ok_or_else(|| "missing --record PATH".to_string())?;
        fs::write(path, &json).map_err(|error| format!("write observation {path}: {error}"))?;
    }
    if let Some(index) = args.iter().position(|arg| arg == "--compare-if-present") {
        let prior = Path::new(
            args.get(index + 1)
                .ok_or_else(|| "missing --compare-if-present PRIOR CURRENT".to_string())?,
        );
        let current = Path::new(
            args.get(index + 2)
                .ok_or_else(|| "missing --compare-if-present PRIOR CURRENT".to_string())?,
        );
        println!("{}", compare_if_present(prior, current)?);
    } else if let Some(index) = args.iter().position(|arg| arg == "--compare") {
        let path = args
            .get(index + 1)
            .ok_or_else(|| "missing --compare PATH".to_string())?;
        let prior = fs::read_to_string(path)
            .map_err(|error| format!("read prior observation {path}: {error}"))?;
        println!("{}", compare_prior(&observation, &prior)?);
    } else {
        print!("{json}");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("parser-platform baseline: {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METHOD: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/superpowers/baselines/2026-07-28-parser-platform-method.json"
    ));

    #[test]
    fn parser_platform_baseline_method_is_structured_and_exact() {
        let method = Method::parse(METHOD).expect("checked-in method");
        assert_eq!(method.enforcement, "report-only");
        assert_eq!(method.warmup_runs, 5);
        assert_eq!(method.sample_runs, 30);
        assert_eq!(method.expected_corpus_identity, "bc26136a556e6fa3");
        assert_eq!(method.hardware_fields, ["os", "arch", "rustc", "cpu_count"]);

        let broken = METHOD.replace("\"prime\": \"00000100000001b3\"", "\"prime\": \"deadbeef\"");
        assert!(Method::parse(&broken).unwrap_err().contains("prime"));
        let with_numeric_budget = METHOD.replacen(
            "\"enforcement\": \"report-only\"",
            "\"enforcement\": \"report-only\", \"latency_budget\": 0",
            1,
        );
        assert!(Method::parse(&with_numeric_budget).is_ok());
    }

    #[test]
    fn parser_platform_baseline_reallocation_accounting_is_explicit() {
        assert_eq!(
            reallocation_accounting(8, 20),
            ReallocationAccounting {
                live_increase: 12,
                live_decrease: 0,
                allocated_bytes: 20,
                allocation_count: 1,
            }
        );
        assert_eq!(
            reallocation_accounting(20, 8),
            ReallocationAccounting {
                live_increase: 0,
                live_decrease: 12,
                allocated_bytes: 8,
                allocation_count: 1,
            }
        );
    }

    #[test]
    fn parser_platform_baseline_compiler_fingerprint_is_verbose() {
        let rustc = Hardware::current().unwrap().rustc;
        assert!(rustc.contains("release:"), "{rustc}");
        assert!(rustc.contains("host:"), "{rustc}");
        assert!(rustc.contains("program:"), "{rustc}");
    }

    #[test]
    fn parser_platform_baseline_compare_uses_full_hardware_fingerprint_and_deltas() {
        let current = Observation::test_sample(
            Hardware {
                os: "windows".into(),
                arch: "x86_64".into(),
                rustc: "rustc 1.90.0".into(),
                cpu_count: 24,
            },
            120,
            150,
            10,
            500,
            1000,
            20,
        );
        let prior = Observation::test_sample(current.hardware.clone(), 100, 140, 8, 450, 900, 18);
        let report = comparison_report(&current, &prior);
        for field in [
            "median_ns_delta=20",
            "p95_ns_delta=10",
            "mad_ns_delta=2",
            "peak_live_bytes_delta=50",
            "allocated_bytes_delta=100",
            "allocation_count_delta=2",
            "LATENCY_REPORT_ONLY",
        ] {
            assert!(report.contains(field), "comparison field {field}: {report}");
        }

        let mut mismatch = prior.clone();
        mismatch.hardware.cpu_count += 1;
        assert_eq!(
            comparison_report(&current, &mismatch),
            "LATENCY_SKIPPED_HARDWARE_MISMATCH"
        );
        assert_eq!(
            compare_prior(&current, r#"{"corpus_identity":"0123456789abcdef"}"#).unwrap(),
            "LATENCY_SKIPPED_HARDWARE_MISMATCH"
        );
    }

    fn comparison_file(name: &str) -> PathBuf {
        env::temp_dir().join(format!("parser-platform-baseline-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn parser_platform_baseline_compare_if_present_absent_prior_skips() {
        let prior = comparison_file("absent-prior");
        let current = comparison_file("current");
        fs::write(
            &current,
            Observation::test_sample(Hardware::current().unwrap(), 120, 150, 10, 500, 1000, 20)
                .json(),
        )
        .unwrap();

        assert_eq!(
            compare_if_present(&prior, &current).unwrap(),
            "LATENCY_SKIPPED_BASELINE_ABSENT"
        );
        fs::remove_file(current).unwrap();
    }

    #[test]
    fn parser_platform_baseline_compare_if_present_reports_matching_evidence() {
        let prior = comparison_file("matching-prior");
        let current = comparison_file("matching-current");
        let hardware = Hardware::current().unwrap();
        fs::write(
            &prior,
            Observation::test_sample(hardware.clone(), 100, 140, 8, 450, 900, 18).json(),
        )
        .unwrap();
        fs::write(
            &current,
            Observation::test_sample(hardware, 120, 150, 10, 500, 1000, 20).json(),
        )
        .unwrap();

        assert!(
            compare_if_present(&prior, &current)
                .unwrap()
                .contains("LATENCY_REPORT_ONLY")
        );
        fs::remove_file(prior).unwrap();
        fs::remove_file(current).unwrap();
    }

    #[test]
    fn parser_platform_baseline_compare_if_present_skips_hardware_mismatch() {
        let prior = comparison_file("mismatch-prior");
        let current = comparison_file("mismatch-current");
        let mut prior_hardware = Hardware::current().unwrap();
        prior_hardware.cpu_count += 1;
        fs::write(
            &prior,
            Observation::test_sample(prior_hardware, 100, 140, 8, 450, 900, 18).json(),
        )
        .unwrap();
        fs::write(
            &current,
            Observation::test_sample(Hardware::current().unwrap(), 120, 150, 10, 500, 1000, 20)
                .json(),
        )
        .unwrap();

        assert_eq!(
            compare_if_present(&prior, &current).unwrap(),
            "LATENCY_SKIPPED_HARDWARE_MISMATCH"
        );
        fs::remove_file(prior).unwrap();
        fs::remove_file(current).unwrap();
    }

    #[test]
    fn parser_platform_baseline_compare_if_present_rejects_missing_or_malformed_evidence() {
        let prior = comparison_file("malformed-prior");
        let current = comparison_file("missing-current");
        assert!(compare_if_present(&prior, &current).unwrap_err().contains("read current observation"));

        fs::write(&current, "not-json").unwrap();
        assert!(compare_if_present(&prior, &current).unwrap_err().contains("invalid observation JSON"));

        fs::write(&prior, "not-json").unwrap();
        fs::write(
            &current,
            Observation::test_sample(Hardware::current().unwrap(), 120, 150, 10, 500, 1000, 20)
                .json(),
        )
        .unwrap();
        assert!(compare_if_present(&prior, &current).unwrap_err().contains("invalid observation JSON"));
        fs::remove_file(prior).unwrap();
        fs::remove_file(current).unwrap();
    }
}
