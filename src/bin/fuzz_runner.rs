use ferro_ssh::app::parse_quick_connect_str;
use ferro_ssh::security::{shell_quote, KnownHostsManager};
use ferro_ssh::session::{extract_fingerprint, extract_key_type};
use ferro_ssh::sftp::parse_ls_output;
use ferro_ssh::terminal::TerminalScreen;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

struct FastRng {
    state: u64,
}

impl FastRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline(always)]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        min + (self.next_u32() as usize % (max - min + 1))
    }

    fn mutate_bytes(&mut self, data: &mut Vec<u8>) {
        if data.is_empty() {
            data.push(self.next_u32() as u8);
            return;
        }
        match self.next_range(0, 4) {
            0 => {
                // Bit flip
                let idx = self.next_range(0, data.len() - 1);
                let bit = 1 << self.next_range(0, 7);
                data[idx] ^= bit;
            }
            1 => {
                // Byte overwrite with special token
                let tokens: &[u8] = b"\x00\xff\x1b\r\n\t\x08;\"'$()[]{}";
                let idx = self.next_range(0, data.len() - 1);
                data[idx] = tokens[self.next_range(0, tokens.len() - 1)];
            }
            2 => {
                // Insert random byte
                let idx = self.next_range(0, data.len());
                data.insert(idx, self.next_u32() as u8);
            }
            3 if data.len() > 1 => {
                // Delete byte
                let idx = self.next_range(0, data.len() - 1);
                data.remove(idx);
            }
            _ => {
                // Append ANSI escape sequence
                let sequences = [
                    &b"\x1b[2J"[..],
                    &b"\x1b[9999;9999H"[..],
                    &b"\x1b[38;2;255;0;255m"[..],
                    &b"\x1b]0;Title\x07"[..],
                    &b"\x1b[1000A"[..],
                ];
                let seq = sequences[self.next_range(0, sequences.len() - 1)];
                data.extend_from_slice(seq);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let duration_secs: u64 = args
        .iter()
        .position(|a| a == "--duration" || a == "-d")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let max_target_iters: u64 = args
        .iter()
        .position(|a| a == "--iterations" || a == "-n")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);

    let num_threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

    println!("============================================================");
    println!("⚡ FerroSSH Hard Fuzzing Engine");
    println!("============================================================");
    println!("Threads:      {}", num_threads);
    println!("Target Ops:   {} iterations (or max {} seconds)", max_target_iters, duration_secs);
    println!("Targets:      VTE Engine, Session Prompts, Shell Quote, SFTP, Quick Connect, Known Hosts");
    println!("------------------------------------------------------------");

    let total_ops = Arc::new(AtomicU64::new(0));
    let panic_count = Arc::new(AtomicU64::new(0));
    let stop_signal = Arc::new(AtomicBool::new(false));

    let start_time = Instant::now();
    let mut handles = Vec::new();

    for thread_idx in 0..num_threads {
        let ops = Arc::clone(&total_ops);
        let panics = Arc::clone(&panic_count);
        let stop = Arc::clone(&stop_signal);

        let handle = thread::spawn(move || {
            let mut rng = FastRng::new(0x1337_0000 + thread_idx as u64);
            let mut screen = TerminalScreen::new(80, 24);
            let mut vte_corpus = b"Hello, World!\x1b[1;32mGreen\x1b[0m \r\nls -la\n".to_vec();

            while !stop.load(Ordering::Relaxed) {
                // Batch of 100 fuzz operations
                for _ in 0..100 {
                    match rng.next_range(0, 5) {
                        0 => {
                            // Target 1: VTE & Terminal Processor
                            rng.mutate_bytes(&mut vte_corpus);
                            if vte_corpus.len() > 4096 {
                                vte_corpus.truncate(512);
                            }
                            let bytes = vte_corpus.clone();
                            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                                screen.process_bytes(&bytes);
                                if rng.next_range(0, 50) == 0 {
                                    let c = rng.next_range(1, 300);
                                    let r = rng.next_range(1, 150);
                                    screen.resize(c, r);
                                }
                            }));
                            if res.is_err() {
                                panics.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        1 => {
                            // Target 2: Shell Quoting Engine
                            let mut s = String::new();
                            for _ in 0..rng.next_range(1, 60) {
                                s.push((rng.next_range(32, 126) as u8) as char);
                            }
                            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                                let q = shell_quote(&s);
                                assert!(q.starts_with('\'') && q.ends_with('\''));
                            }));
                            if res.is_err() {
                                panics.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        2 => {
                            // Target 3: Quick Connect Parser
                            let mut s = String::new();
                            for _ in 0..rng.next_range(1, 50) {
                                s.push((rng.next_range(32, 126) as u8) as char);
                            }
                            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                                let (u, h, p) = parse_quick_connect_str(&s);
                                assert!(!u.is_empty() && !h.is_empty() && p > 0);
                            }));
                            if res.is_err() {
                                panics.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        3 => {
                            // Target 4: SFTP Directory Parser
                            let mut ls_out = String::new();
                            for _ in 0..rng.next_range(1, 10) {
                                ls_out.push_str("drwxr-xr-x 2 user group 4096 Oct 23 12:00 dir\n");
                                ls_out.push_str("-rw-r--r-- 1 user group 12345 Oct 23 12:00 file.txt\n");
                            }
                            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                                let _ = parse_ls_output(&ls_out);
                            }));
                            if res.is_err() {
                                panics.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        4 => {
                            // Target 5: Prompt & Fingerprint Scanner
                            let text = format!(
                                "The authenticity of host 'test' can't be established.\nED25519 key fingerprint is SHA256:{}.\nAre you sure?",
                                rng.next_u32()
                            );
                            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                                let _ = extract_fingerprint(&text);
                                let _ = extract_key_type(&text);
                            }));
                            if res.is_err() {
                                panics.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        _ => {
                            // Target 6: Known Hosts Fingerprint Calculator
                            let mut key_str = String::new();
                            for _ in 0..rng.next_range(10, 80) {
                                key_str.push((rng.next_range(48, 122) as u8) as char);
                            }
                            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                                let _ = KnownHostsManager::calculate_fingerprint(&key_str);
                            }));
                            if res.is_err() {
                                panics.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                }
                ops.fetch_add(100, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    // Monitor thread reporting progress every 1 second
    let monitor_start = Instant::now();
    let mut last_ops = 0u64;

    while monitor_start.elapsed().as_secs() < duration_secs {
        thread::sleep(Duration::from_secs(1));
        let current_ops = total_ops.load(Ordering::Relaxed);
        let delta = current_ops.saturating_sub(last_ops);
        last_ops = current_ops;

        let elapsed = monitor_start.elapsed().as_secs_f64();
        let p_count = panic_count.load(Ordering::Relaxed);

        println!(
            "[{:02.0}s] Total: {:>9} ops | {:>8} ops/sec | Panics: {}",
            elapsed, current_ops, delta, p_count
        );

        if current_ops >= max_target_iters {
            println!("Reached target iterations ({} ops)!", current_ops);
            break;
        }
    }

    stop_signal.store(true, Ordering::SeqCst);
    for h in handles {
        let _ = h.join();
    }

    let final_ops = total_ops.load(Ordering::SeqCst);
    let final_panics = panic_count.load(Ordering::SeqCst);
    let total_duration = start_time.elapsed().as_secs_f64();
    let avg_rate = (final_ops as f64) / total_duration;

    println!("------------------------------------------------------------");
    println!("🏁 Fuzzing Complete!");
    println!("Elapsed Time:     {:.2} seconds", total_duration);
    println!("Total Operations: {} ops", final_ops);
    println!("Average Rate:     {:.0} ops/sec", avg_rate);
    println!("Panics / Crashes: {}", final_panics);
    println!("Status:           {}", if final_panics == 0 { "✅ PASS (ROCK SOLID)" } else { "❌ FAILED" });
    println!("============================================================");

    if final_panics > 0 {
        std::process::exit(1);
    }
}
