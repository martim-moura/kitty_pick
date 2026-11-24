//! Enumerate only. This does not measure skim matching or rendered frames.
use std::{path::PathBuf, time::Instant};

use anyhow::Result;
use picc_menu::source::{Files, Source};

fn main() -> Result<()> {
    let root =
        PathBuf::from(std::env::args_os().nth(1).unwrap_or_else(|| ".".into())).canonicalize()?;
    let started = Instant::now();
    let mut count = 0_u64;
    let mut first = None;
    Box::new(Files {
        root,
        hidden: false,
    })
    .produce(&mut |_| {
        first.get_or_insert_with(|| started.elapsed());
        count += 1;
        true
    })?;
    println!(
        "files={count} first_us={} total_ms={:.3}",
        first.map_or(0, |time| time.as_micros()),
        started.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
