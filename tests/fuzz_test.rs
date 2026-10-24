use ferro_ssh::app::parse_quick_connect_str;
use ferro_ssh::security::{shell_quote, KnownHostsManager};
use ferro_ssh::session::{extract_fingerprint, extract_key_type};
use ferro_ssh::sftp::parse_ls_output;
use ferro_ssh::terminal::TerminalScreen;
use std::panic::{self, AssertUnwindSafe};

// Pseudo-random Linear Congruential Generator for deterministic, fast fuzzing without external rand crate
struct FuzzRng {
    state: u64,
}

impl FuzzRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        min + (self.next_u32() as usize % (max - min + 1))
    }

    fn next_bytes(&mut self, len: usize) -> Vec<u8> {
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            buf.push(self.next_u32() as u8);
        }
        buf
    }

    fn next_ascii(&mut self, len: usize) -> String {
        let mut s = String::with_capacity(len);
        let chars = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_./:;@[]{}()!@#$%^&*+= \t\n\r'\"\\";
        for _ in 0..len {
            let idx = self.next_range(0, chars.len() - 1);
            s.push(chars[idx] as char);
        }
        s
    }

    fn next_unicode_string(&mut self, len: usize) -> String {
        let corpus = ["🦀", "⚡", "🔒", "日", "本", "語", "Привет", "世界", "\u{0000}", "\u{FEFF}", "\u{200B}", "\u{202E}", "A", " ", "\n", "'", "\""];
        let mut s = String::new();
        for _ in 0..len {
            let idx = self.next_range(0, corpus.len() - 1);
            s.push_str(corpus[idx]);
        }
        s
    }
}

#[test]
fn test_fuzz_terminal_vte_random_noise() {
    let mut rng = FuzzRng::new(0xDEADBEEF);
    let mut screen = TerminalScreen::new(80, 24);

    for iteration in 0..2000 {
        let chunk_size = rng.next_range(1, 1024);
        let bytes = rng.next_bytes(chunk_size);

        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            screen.process_bytes(&bytes);
        }));

        assert!(
            result.is_ok(),
            "TerminalScreen panicked on random noise at iteration {}",
            iteration
        );

        // Invariant: cursor position must always be within screen boundaries
        assert!(
            screen.cursor_x < screen.cols,
            "cursor_x {} exceeded cols {} at iteration {}",
            screen.cursor_x,
            screen.cols,
            iteration
        );
        assert!(
            screen.cursor_y < screen.rows,
            "cursor_y {} exceeded rows {} at iteration {}",
            screen.cursor_y,
            screen.rows,
            iteration
        );
    }
}

#[test]
fn test_fuzz_terminal_vte_ansi_escape_bomb() {
    let mut rng = FuzzRng::new(0xCAFEBABE);
    let mut screen = TerminalScreen::new(100, 40);

    let ansi_tokens = [
        "\x1b[H",
        "\x1b[2J",
        "\x1b[3J",
        "\x1b[0m",
        "\x1b[1;31;42m",
        "\x1b[38;5;196m",
        "\x1b[48;2;255;128;64m",
        "\x1b[999999;999999H",
        "\x1b[0;0H",
        "\x1b[1000A",
        "\x1b[1000B",
        "\x1b[1000C",
        "\x1b[1000D",
        "\x1b[?25h",
        "\x1b[?25l",
        "\x1b]0;Title\x07",
        "\x1b]2;LongTitleBomb\x1b\\",
        "\r\n",
        "\t",
        "\x08",
        "\x07",
        "\x1b[s",
        "\x1b[u",
        "\x1b[K",
        "\x1b[1K",
        "\x1b[2K",
    ];

    for _ in 0..1500 {
        let num_tokens = rng.next_range(5, 50);
        let mut payload = String::new();
        for _ in 0..num_tokens {
            let idx = rng.next_range(0, ansi_tokens.len() - 1);
            payload.push_str(ansi_tokens[idx]);
            if rng.next_range(0, 3) == 0 {
                let sz = rng.next_range(1, 5);
                payload.push_str(&rng.next_unicode_string(sz));
            }
        }

        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            screen.process_bytes(payload.as_bytes());
        }));

        assert!(result.is_ok(), "TerminalScreen panicked on ANSI escape bomb");
    }
}

#[test]
fn test_fuzz_terminal_rapid_resizing_under_load() {
    let mut rng = FuzzRng::new(0x12345678);
    let mut screen = TerminalScreen::new(80, 24);

    for _ in 0..1000 {
        // Random resize (including extreme bounds like 1x1, 1x500, 500x1)
        let new_cols = rng.next_range(1, 300);
        let new_rows = rng.next_range(1, 150);
        screen.resize(new_cols, new_rows);

        // Process bytes
        let count = rng.next_range(10, 256);
        let bytes = rng.next_bytes(count);
        screen.process_bytes(&bytes);

        // Verify grid dimensions match exactly
        assert_eq!(screen.grid.len(), screen.rows);
        for row in &screen.grid {
            assert_eq!(row.len(), screen.cols);
        }
        assert!(screen.cursor_x < screen.cols);
        assert!(screen.cursor_y < screen.rows);
    }
}

#[test]
fn test_fuzz_session_buffer_drain_utf8_boundaries() {
    // Tests that draining buffer history on multi-byte characters never causes UTF-8 char boundary panics
    let mut rng = FuzzRng::new(0xABCDEF01);
    let mut buffer = String::new();

    for _ in 0..500 {
        let count = rng.next_range(50, 200);
        let chunk = rng.next_unicode_string(count);
        buffer.push_str(&chunk);

        if buffer.len() > 8192 {
            let mut cut = buffer.len() - 4096;
            while !buffer.is_char_boundary(cut) && cut < buffer.len() {
                cut += 1;
            }
            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                buffer.drain(..cut);
            }));
            assert!(res.is_ok(), "Buffer drain panicked on UTF-8 char boundary!");
        }

        // Test prompt extraction on the buffer
        let _ = extract_fingerprint(&buffer);
        let _ = extract_key_type(&buffer);
    }
}

#[test]
fn test_fuzz_shell_quote_comprehensive() {
    let mut rng = FuzzRng::new(0x98765432);

    for _ in 0..5000 {
        let len = rng.next_range(0, 120);
        let s = rng.next_ascii(len);
        let quoted = shell_quote(&s);

        // Invariant 1: Quoted string must start and end with single quotes
        assert!(quoted.starts_with('\''), "Quoted string must start with single quote");
        assert!(quoted.ends_with('\''), "Quoted string must end with single quote");

        // Invariant 2: Must never contain unescaped single quotes
        // In POSIX shell, single-quoted string cannot break out unless an unescaped ' exists
        let inner = &quoted[1..quoted.len() - 1];
        // Any quote inside must be part of the '\''' sequence
        let split: Vec<&str> = inner.split("'\\''").collect();
        for piece in split {
            assert!(
                !piece.contains('\''),
                "Found unescaped single quote in shell_quote output: {}",
                quoted
            );
        }
    }
}

#[test]
fn test_fuzz_quick_connect_parser() {
    let mut rng = FuzzRng::new(0xFEDCBA98);

    for _ in 0..5000 {
        let len = rng.next_range(0, 80);
        let input = rng.next_ascii(len);

        let res = panic::catch_unwind(AssertUnwindSafe(|| {
            parse_quick_connect_str(&input)
        }));

        assert!(res.is_ok(), "parse_quick_connect_str panicked on input: {:?}", input);
        let (user, _host, port) = res.unwrap();
        assert!(!user.is_empty(), "Username must never be empty");
        assert!(port > 0, "Port must be a valid positive port");
    }

    // Explicit edge case verification
    let (u, h, p) = parse_quick_connect_str("[::1]:2222");
    assert_eq!(u, "root");
    assert_eq!(h, "::1");
    assert_eq!(p, 2222);

    let (u, h, p) = parse_quick_connect_str("admin@[2001:db8::1]:8022");
    assert_eq!(u, "admin");
    assert_eq!(h, "2001:db8::1");
    assert_eq!(p, 8022);

    let (u, h, p) = parse_quick_connect_str("   ");
    assert_eq!(u, "root");
    assert_eq!(h, "localhost");
    assert_eq!(p, 22);

    let (u, h, p) = parse_quick_connect_str("@");
    assert_eq!(u, "root");
    assert_eq!(h, "localhost");
    assert_eq!(p, 22);

    let (u, h, p) = parse_quick_connect_str("user@:0");
    assert_eq!(u, "user");
    assert_eq!(h, "localhost");
    assert_eq!(p, 22);
}

#[test]
fn test_fuzz_sftp_ls_parser() {
    let mut rng = FuzzRng::new(0x55AA55AA);

    for _ in 0..2000 {
        let num_lines = rng.next_range(0, 50);
        let mut stdout = String::new();
        for _ in 0..num_lines {
            if rng.next_range(0, 3) == 0 {
                // Malformed line
                let sz = rng.next_range(5, 40);
                stdout.push_str(&rng.next_ascii(sz));
            } else {
                // Semi-structured line with random values
                let perms = if rng.next_range(0, 1) == 0 { "drwxr-xr-x" } else { "-rw-r--r--" };
                let size = rng.next_u32();
                let sz = rng.next_range(1, 20);
                let name = rng.next_ascii(sz);
                stdout.push_str(&format!("{} 1 user group {} Oct 23 12:00 {}\n", perms, size, name));
            }
            stdout.push('\n');
        }

        let res = panic::catch_unwind(AssertUnwindSafe(|| {
            parse_ls_output(&stdout)
        }));

        assert!(res.is_ok(), "parse_ls_output panicked on fuzzed input");
        let entries = res.unwrap();

        // Invariant: entries must be properly sorted (directories before files, .. first)
        let mut seen_file = false;
        for entry in entries {
            if entry.is_dir {
                assert!(!seen_file || entry.name == "..", "Directory appeared after regular files!");
            } else {
                seen_file = true;
            }
        }
    }
}

#[test]
fn test_fuzz_known_hosts_calculate_fingerprint() {
    let mut rng = FuzzRng::new(0x33445566);

    for _ in 0..3000 {
        let sz = rng.next_range(0, 200);
        let key = rng.next_ascii(sz);
        let res = panic::catch_unwind(AssertUnwindSafe(|| {
            KnownHostsManager::calculate_fingerprint(&key)
        }));

        assert!(res.is_ok(), "calculate_fingerprint panicked on fuzzed key");
        if let Ok(Some(fp)) = res {
            assert!(fp.starts_with("SHA256:"));
        }
    }
}
