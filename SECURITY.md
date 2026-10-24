# Security Policy and Architecture Specification

FerroSSH approaches security with engineering precision. Security is not a vague badge or an absolute promise—it is a defined set of architectural boundaries, cryptographic guarantees, and active mitigations against specific threat vectors.

This document details FerroSSH's security architecture, explicit threat model, concrete mitigations, and limitations.

---

## 1. Core Architectural Security Principles

### 1.1 Transport Delegation to Audited OpenSSH
FerroSSH **does not implement a custom cryptographic protocol stack**. Implementing SSH from scratch risks introducing cipher negotiation bugs, timing side-channels, or flawed state machines.

Instead, FerroSSH delegates the cryptographic transport, key exchange, and session integrity to the platform's native OpenSSH client (`ssh` / `scp`). This ensures:
- Transport encryption uses audited implementations of ChaCha20-Poly1305, AES-GCM, and Curve25519.
- Cryptographic primitives and cipher negotiation conform to the security policies set by your operating system's OpenSSH configuration (`/etc/ssh/ssh_config` and `~/.ssh/config`).
- Session rekeying, packet MACs, and PFS (Perfect Forward Secrecy) are handled strictly by OpenSSH.

### 1.2 Direct Process Invocation (No Shell Interpretation)
Local command execution (`ssh`, `scp`) is performed using direct OS system calls (`execve` on Unix, `CreateProcess` on Windows) via Rust's `std::process::Command` and `portable_pty::CommandBuilder`.
- Commands are **never** passed to an intermediate shell (e.g. `/bin/sh -c` or `cmd.exe /c`).
- Arguments are passed as discrete vector elements rather than concatenated strings, preventing local command injection via shell metacharacters (`;`, `&`, `|`, `$()`, backticks).

### 1.3 Command-Line Option Injection Protection
When passing user-supplied input (such as server hostnames or usernames) to command-line tools like OpenSSH or SCP, an untrusted host starting with a leading dash (`-`) could trick the program into treating it as a flag (e.g., `-oProxyCommand=...` or `-oLocalCommand=...`).

FerroSSH protects against this by explicitly injecting the POSIX end-of-options delimiter (`--`) before target destinations:
```rust
cmd.arg("-p").arg(port.to_string());
cmd.arg("--"); // Halts option processing
cmd.arg(format!("{}@{}", username, host));
```

### 1.4 Remote Shell Quoting & Sanitization
OpenSSH invokes remote commands by sending them to the remote user's login shell via `$SHELL -c "command"`. In features like the SFTP remote file browser, directory paths are queried using non-interactive remote commands (`ls -la <path>`).

To guarantee that directory names containing shell metacharacters, spaces, or control characters cannot break out into arbitrary remote execution, FerroSSH encapsulates all path arguments with POSIX single-quote escaping:
```rust
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
```
In POSIX shells, no character within single quotes is expanded or treated as a metacharacter.

### 1.5 In-Memory Secret Zeroization & Scrubbing
Passwords entered in the GUI modal are protected using defense-in-depth memory hygiene:
- Credentials in memory are encapsulated in `SecureSecret`, implementing `zeroize::Zeroize` and `zeroize::ZeroizeOnDrop`.
- As soon as the password bytes are dispatched to the PTY, the local heap buffers are explicitly wiped with zeroes (`.zeroize()`).
- Keystrokes handled by the PTY writer thread are scrubbed immediately after being written to the file descriptor.
- `SecureSecret` overrides `std::fmt::Debug` to output `SecureSecret([REDACTED])`, ensuring credentials are never exposed in debug logging, console traces, or error dumps.

### 1.6 Trust-On-First-Use (TOFU) Host Key Enforcement
FerroSSH enforces strict host key verification:
- When connecting to a host whose public key is not in `~/.ssh/known_hosts`, the client intercepts the connection and displays a security modal.
- The modal presents the cryptographic key type (`ED25519`, `RSA`, `ECDSA`) and full SHA-256 fingerprint.
- The user must explicitly choose to **Accept & Save**, **Accept Once (Session Only)**, or **Reject & Abort**.
- The client does not bypass or disable OpenSSH's `StrictHostKeyChecking` without explicit user interaction.

### 1.7 Privacy and Clean-Slate Defaults
- **Zero Telemetry**: FerroSSH does not collect analytics, telemetry, or crash reports.
- **Zero Outbound Network Traffic**: No external HTTP/S requests or analytics pings are made. Network traffic occurs only to the SSH hosts you explicitly configure.
- **Anonymized Fresh Installs**: Fresh installations contain no bundled third-party server profiles, test hosts, or preloaded tunnels.

### 1.8 Adversarial Fuzz Testing & Parser Hardening
All untrusted input parsers are subject to continuous property-based and mutation-based fuzz testing to eliminate denial-of-service vectors, parser desynchronization, and memory boundary errors:
- **Terminal VTE Engine**: Fuzzed against malformed ANSI/VT100 escape sequences, out-of-bounds coordinates, color space overflows, and rapid runtime screen resizing.
- **History Buffer Draining**: Hardened against UTF-8 multi-byte character boundary corruption during sliding window trimming.
- **SFTP Directory Listing Parser**: Verified for strict weak ordering compliance to prevent undefined sorting behavior or denial-of-service on malformed directory listings.
- **Quick Connect Parser**: Hardened against empty components, malformed IPv6 bracket notations, and port boundary violations.
- **Stress Testing Campaign**: Verified across 1,000,000+ concurrent operations with 0 crashes or unhandled panics (`src/bin/fuzz_runner.rs`).

---

## 2. Threat Model

### In Scope (Protected Against)
| Threat Vector | Mitigation Strategy |
|---|---|
| **Man-in-the-Middle (MitM) Attacks** | Authenticated host key verification (TOFU) and OpenSSH cryptographic channel encryption. |
| **Local Option Injection** | `--` delimiter passed before all dynamic host and destination arguments to `ssh` and `scp`. |
| **Remote Shell Injection** | Strict POSIX single-quote encapsulation (`shell_quote`) for all remote command arguments. |
| **Residual Heap Secret Exposure** | Immediate `.zeroize()` wiping of password and input buffers upon transmission. |
| **Debug Log Credential Leakage** | `SecureSecret` debug redaction to `[REDACTED]`. |
| **Parser Denial of Service / Crashes** | 1,000,000+ cycle mutation fuzz testing across terminal, SFTP, prompt, and connection parsers. |
| **Supply Chain Vulnerabilities** | Automated dependency scanning verifying 0 known CVEs across the Rust crate tree. |

### Out of Scope (Explicit Boundaries & Limitations)
| Threat / Scenario | Explanation & Boundary |
|---|---|
| **Compromised Host OS / Kernel** | If the machine running FerroSSH has an active rootkit, kernel keylogger, or unauthorized administrative user using `ptrace` / memory inspection, FerroSSH cannot protect against in-memory keylogging. |
| **Physical Memory Analysis / Cold Boot** | FerroSSH clears credentials from heap memory as quickly as possible, but cannot guarantee protection against live hypervisor snapshots or physical cold-boot hardware attacks while passwords are actively being entered. |
| **Compromised Remote Host** | Once connected to a remote SSH server, the integrity of commands executed inside that server depends on the security posture of the remote host itself. |
| **Weak Remote Passwords / Keys** | FerroSSH enforces client-side hygiene but cannot prevent authentication failures resulting from weak passwords or exposed private keys on the local filesystem. |

---

## 3. Reporting a Security Vulnerability

If you discover a potential security vulnerability in FerroSSH, please report it responsibly:

- **GitHub Security Advisories (Preferred)**: Navigate to the repository's **Security** tab and click **Report a vulnerability** to open a confidential advisory.
- **Direct Contact**: If GitHub Advisories are unavailable, contact the project maintainers via the email address listed on the GitHub profile.

Please include:
1. A description of the vulnerability and potential impact.
2. Step-by-step reproduction instructions or a proof-of-concept.
3. Your assessment of affected platforms (Linux, macOS, Windows).

We commit to acknowledging reports within 48 hours and providing a mitigation timeline prior to public disclosure.
