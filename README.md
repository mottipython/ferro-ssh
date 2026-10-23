# FerroSSH ⚡

A secure, high-performance, and feature-rich graphical SSH client written in Rust with `egui` (`eframe`).

---

## Features

- **🛡️ Cryptographic Host Key Verification (TOFU):**
  - Intercepts unrecognized SSH host fingerprints and presents a Trust-On-First-Use confirmation modal.
  - Options to **Accept & Save to `~/.ssh/known_hosts`**, **Accept Once (Session Only)**, or **Reject & Abort**.
  - Shows key type (`ED25519`, `RSA`, `ECDSA`) and full SHA-256 fingerprint.
- **🔐 Password & Secret Security:**
  - Secure password authentication modal with masked fields.
  - Credentials in memory are wrapped in `zeroize::Zeroize` to ensure passwords and passphrases are securely scrubbed from RAM immediately upon release.
  - Redacted debug representations to avoid secret leakage in logs.
- **📟 Embedded Multi-Session Terminal:**
  - Real PTY terminal emulator powered by `portable-pty` and `vte`.
  - ANSI 16-color and 256-color palette support, bold and underline styling.
  - Multi-tab session manager with independent terminal buffers and scrollback.
  - Full keyboard mapping including arrow keys, function keys, and control sequences (`Ctrl+C`, `Ctrl+D`, `Ctrl+Z`, `Ctrl+L`).
- **🗂️ Connection Profile Manager:**
  - Quick Connect bar (`user@host[:port]`).
  - Saved profiles with color badges, groups, notes, and port configuration.
  - Auto-discovery of existing private keys in `~/.ssh/` (`id_ed25519`, `id_rsa`, `id_ecdsa`).
- **📁 Integrated SFTP Remote File Browser:**
  - Browse remote directories with permissions, file sizes, and modification dates.
  - Native file dialogs for uploading and downloading files.
- **🔀 Port Forwarding & Tunnel Manager:**
  - Configure and manage local port forwards (`-L local_port:remote_host:remote_port`).
- **✅ Snyk Security Validated:**
  - Passed `snyk code test` with 0 security issues or vulnerabilities.

---

## Building and Running

### Prerequisites
* Rust 1.80+ (`cargo`, `rustc`)
* OpenSSH client (`ssh`, `scp`)

### Run Development Build
```bash
cargo run
```

### Build Optimized Release
```bash
cargo build --release
```
The binary will be located at `target/release/ferro-ssh`.
