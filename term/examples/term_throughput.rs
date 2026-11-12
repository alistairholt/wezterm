//! Feeds files through the escape parser and terminal model the way
//! the mux parser thread does, and reports throughput and the heap
//! held by the terminal afterwards (mostly scrollback).
//!
//! cargo run --release -p wezterm-term --example term_throughput -- \
//!     [--scrollback N] [--cols N] [--rows N] FILE...
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(
            new_size as isize - layout.size() as isize,
            Ordering::Relaxed,
        );
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[derive(Debug)]
struct Config {
    scrollback: usize,
}
impl TerminalConfiguration for Config {
    fn scrollback_size(&self) -> usize {
        self.scrollback
    }
    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
}

/// Matches the default mux_output_parser_buffer_size.
const CHUNK: usize = 128 * 1024;

fn main() {
    let mut scrollback = 3500;
    let mut cols = 120;
    let mut rows = 40;
    let mut files = vec![];
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scrollback" => scrollback = args.next().unwrap().parse().unwrap(),
            "--cols" => cols = args.next().unwrap().parse().unwrap(),
            "--rows" => rows = args.next().unwrap().parse().unwrap(),
            _ => files.push(a),
        }
    }

    for file in files {
        // A pty in its default mode (onlcr) turns \n into \r\n
        let raw = std::fs::read(&file).unwrap();
        let mut data = Vec::with_capacity(raw.len() + raw.len() / 32);
        for &b in &raw {
            if b == b'\n' {
                data.push(b'\r');
            }
            data.push(b);
        }
        let base = LIVE.load(Ordering::Relaxed);
        let mut term = Terminal::new(
            TerminalSize {
                rows,
                cols,
                pixel_width: cols * 8,
                pixel_height: rows * 16,
                dpi: 0,
            },
            Arc::new(Config { scrollback }),
            "WezTerm",
            "bench",
            Box::new(std::io::sink()),
        );
        let mut parser = termwiz::escape::parser::Parser::new();

        let start = Instant::now();
        for chunk in data.chunks(CHUNK) {
            let mut actions = vec![];
            parser.parse(chunk, |action| action.append_to(&mut actions));
            term.perform_actions(actions);
        }
        let elapsed = start.elapsed();
        let held = LIVE.load(Ordering::Relaxed) - base;
        let lines = term.screen().scrollback_rows();
        println!(
            "{:<12} {:>7.1} MB/s  {:>7.0} ms  held {:>7.1} MB  {:>6} lines  {:>5} B/line",
            std::path::Path::new(&file)
                .file_name()
                .unwrap()
                .to_string_lossy(),
            data.len() as f64 / 1e6 / elapsed.as_secs_f64(),
            elapsed.as_secs_f64() * 1000.,
            held as f64 / 1e6,
            lines,
            held as usize / lines.max(1),
        );
    }
}
