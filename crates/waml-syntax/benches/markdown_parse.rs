//! Stage 0 parse-time baseline for the Markdown front end.
//!
//! Deliberately dependency-free: `harness = false` plus `std::time::Instant`,
//! because the whole point of the pulldown-cmark removal is fewer third-party
//! crates, and adding criterion or divan to measure that would be absurd.
//!
//! Native-only by construction. Benches never run on wasm, which matters here
//! because this project's wasm target has no clock at all.
//!
//! Run with: `cargo bench -p waml-syntax`

use std::{fs, hint::black_box, path::Path, time::Instant};

use waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

/// Only the `markdown` field is needed; `serde` ignores the rest of each entry.
#[derive(serde::Deserialize)]
struct Example {
    markdown: String,
}

fn corpus() -> Vec<String> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/commonmark-0.31.2/spec.json");
    let source = fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read CommonMark fixture {}: {error}", fixture.display()));
    let examples: Vec<Example> =
        serde_json::from_str(&source).expect("deserialize CommonMark fixture");
    examples
        .into_iter()
        .map(|example| example.markdown)
        .collect()
}

fn parse_corpus(corpus: &[String], dialect: MarkdownDialect) {
    for markdown in corpus {
        let text = SourceText::new(markdown.as_str()).expect("fixture source is valid");
        let snapshot =
            parse_markdown(DocumentRevision::INITIAL, text, dialect).expect("fixture parses");
        black_box(snapshot);
    }
}

fn main() {
    const WARMUP: u32 = 2;
    const ROUNDS: u32 = 10;

    let corpus = corpus();
    let bytes: usize = corpus.iter().map(String::len).sum();
    println!("corpus: {} examples, {bytes} bytes", corpus.len());

    for (label, dialect) in [
        ("commonmark", MarkdownDialect::COMMONMARK_0_31_2),
        ("waml", MarkdownDialect::WAML_DEFAULT),
    ] {
        for _ in 0..WARMUP {
            parse_corpus(&corpus, dialect);
        }
        let mut best = f64::MAX;
        let mut total = 0.0_f64;
        for _ in 0..ROUNDS {
            let started = Instant::now();
            parse_corpus(&corpus, dialect);
            let elapsed = started.elapsed().as_secs_f64();
            best = best.min(elapsed);
            total += elapsed;
        }
        let mean = total / f64::from(ROUNDS);
        let throughput = (bytes as f64 / (1024.0 * 1024.0)) / best;
        println!(
            "{label:<11} best {:>9.3} ms   mean {:>9.3} ms   {throughput:>7.1} MiB/s",
            best * 1000.0,
            mean * 1000.0,
        );
    }
}
