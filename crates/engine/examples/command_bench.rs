//! What every command costs on the thread that runs it (the UI thread in the desktop app), apart
//! from the command's own work: `cargo run --release -p lightcraft-engine --example command_bench`
//! (`PRESETS=0,100,500` user presets, `N=2000` commands per scenario, `DISK=1` to also time
//! committed commands on a real library folder, fsync included).
//!
//! Scenarios, each with a library open and that many user presets (full-settings presets, the kind an
//! imported preset pack adds):
//! - `drag`: `develop.set` ticks inside an interaction (a slider drag: nothing is written to the log);
//! - `commit`: committed `develop.set` commands on an in-memory store (no fsync: the CPU overhead);
//! - `rate`: committed `photo.rate` commands (a small command that doesn't touch develop settings);
//! - `disk`: committed `develop.set` commands on a library folder (the fsync dominates).
//!
//! Prints the mean time per command (µs).
use std::time::Instant;

use lightcraft_catalog::MemStore;
use lightcraft_develop::{DevelopSettings, Preset, SettingsGroup};
use lightcraft_engine::Session;
use lightcraft_engine::library::LibraryStores;
use serde_json::json;

fn env_list(name: &str, default: &[usize]) -> Vec<usize> {
    std::env::var(name).ok().map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_else(|| default.to_vec())
}

/// Develop settings with most sections moved off their defaults, so a preset made from them is full.
fn busy_settings() -> DevelopSettings {
    let mut d = DevelopSettings::default();
    for c in lightcraft_develop::controls::CONTROLS {
        lightcraft_develop::controls::set(&mut d, c.id, c.default + (c.max - c.default) * 0.25);
    }
    d
}

fn add_presets(s: &mut Session, n: usize) {
    let d = busy_settings();
    for i in 0..n {
        let p = Preset::from_settings(&format!("user.bench.{i}"), &format!("Bench {i}"), "Bench Pack", &d, &SettingsGroup::ALL);
        s.presets.push(p);
    }
}

fn mem_session(presets: usize) -> Session {
    let mut s = Session::new();
    let stores = LibraryStores { dir: "bench".into(), catalog: Box::new(MemStore::new()), files: Box::new(MemStore::new()), on_disk: false };
    if let Err(e) = s.open_library_in(stores, true) {
        eprintln!("can't open an in-memory library: {e}");
        std::process::exit(1);
    }
    add_presets(&mut s, presets);
    // write presets.json once, so the timed commands find it unchanged
    let _ = s.persist();
    s
}

/// Stop when a scenario's command fails (it would time the error path).
fn check(r: lightcraft_engine::Result<serde_json::Value>, what: &str) {
    if let Err(e) = r {
        eprintln!("{what} failed: {e}");
        std::process::exit(1);
    }
}

/// Mean µs per call of `f(i)` over `n` calls.
fn time(n: usize, mut f: impl FnMut(usize)) -> f64 {
    let t = Instant::now();
    for i in 0..n {
        f(i);
    }
    t.elapsed().as_secs_f64() * 1e6 / n.max(1) as f64
}

fn value(i: usize) -> f64 {
    // stays in range and never repeats the previous value (a repeat may be skipped)
    ((i % 200) as f64 - 100.0) / 100.0
}

fn main() {
    let presets = env_list("PRESETS", &[0, 100, 500]);
    let n = env_list("N", &[2000]).first().copied().unwrap_or(2000);
    let disk = std::env::var("DISK").is_ok_and(|v| v == "1");
    println!("{n} commands per scenario; mean µs per command");
    println!("{:>8} {:>10} {:>10} {:>10} {:>10}", "presets", "drag", "commit", "rate", "disk");
    for &p in &presets {
        let mut s = mem_session(p);
        check(s.execute("develop.beginInteraction", &json!({"label": "Exposure"})), "develop.beginInteraction");
        check(s.execute("develop.set", &json!({"control": "light.exposure", "value": 0.5})), "develop.set (drag)");
        let drag = time(n, |i| {
            let _ = s.execute("develop.set", &json!({"control": "light.exposure", "value": value(i)}));
        });
        let _ = s.execute("develop.endInteraction", &json!({}));

        let mut s = mem_session(p);
        check(s.execute("develop.set", &json!({"control": "light.exposure", "value": 0.5})), "develop.set");
        check(s.execute("photo.rate", &json!({"rating": 1})), "photo.rate");
        let commit = time(n, |i| {
            let _ = s.execute("develop.set", &json!({"control": "light.exposure", "value": value(i)}));
        });

        let mut s = mem_session(p);
        let rate = time(n, |i| {
            let _ = s.execute("photo.rate", &json!({"rating": i % 6}));
        });

        let disk_us = if disk {
            let dir = std::env::temp_dir().join(format!("lc-command-bench-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let mut s = Session::new();
            match s.open_library(&dir, true) {
                Ok(_) => {
                    add_presets(&mut s, p);
                    let _ = s.persist();
                    let t = time(n.min(500), |i| {
                        let _ = s.execute("develop.set", &json!({"control": "light.exposure", "value": value(i)}));
                    });
                    let _ = s.close_library();
                    let _ = std::fs::remove_dir_all(&dir);
                    format!("{t:>10.1}")
                }
                Err(e) => format!("({e})"),
            }
        } else {
            format!("{:>10}", "-")
        };
        println!("{p:>8} {drag:>10.1} {commit:>10.1} {rate:>10.1} {disk_us}");
    }
}
