# 🎙️ Voicemeeter Auto Restart (VBAR)

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Slint](https://img.shields.io/badge/GUI-Slint%20M3-blue.svg)](https://slint.dev/)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows-lightgrey.svg)]()

**Voicemeeter Auto Restart (VBAR)** is a lightweight, standalone Windows watchdog service developed in **Rust** with a modern **Slint Material Design 3** interface. It monitors VoiceMeeter, diagnoses crashes in real-time, releases locked audio drivers (WASAPI/ASIO), and restarts the application seamlessly.

---

## ✨ Features

- ⚡ **Zero CPU Overhead:** Asynchronous Win32 kernel event monitoring (`WaitForSingleObject`) with 0% idle CPU usage.
- 🚀 **Autostart with Windows:** Optional startup registration in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` without requiring administrator privileges.
- 📥 **System Tray:** Minimizes cleanly to the Windows notification area with a quick-access context menu.
- 🛡️ **Crash Loop Protection:** Detects rapid consecutive crashes and halts the restart loop to prevent system overload.
- 🔍 **Exit Code Diagnostics:** Decodes and translates Windows crash codes (e.g. `0xC0000005 - Access Violation`).
- 🌐 **Bilingual (EN 🇺🇸 / PT 🇧🇷):** Instant language switching with a floating context menu.
- 📁 **Clean AppData Persistence:** Settings and rotating diagnostic logs are saved strictly in `%APPDATA%\VBAR\`.

---

## 🚀 How to Build

### Prerequisites
- [Rust (1.75+)](https://rustup.rs/)
- Windows 10 / 11

### Build:
```powershell
# Clone the repository
git clone https://github.com/Aio-G/VoicemeeterAutoRestart.git
cd VoicemeeterAutoRestart

# Run tests
cargo test

# Build optimized release binary
cargo build --release
```

The compiled binary will be generated at `target/release/voicemeeter-auto-restart.exe`.

---

## ⚙️ Configuration

Configuration is automatically saved in `%APPDATA%\VBAR\config.json`:

```json
{
  "language": "en",
  "process_name": "voicemeeter8x64.exe",
  "process_path": "C:\\Program Files (x86)\\VB\\Voicemeeter\\voicemeeter8x64.exe",
  "check_interval_secs": 3.0,
  "start_minimized": false,
  "autostart_with_windows": false,
  "crash_protection_enabled": true,
  "max_consecutive_crashes": 3
}
```

---

## 📄 License

Distributed under the [MIT License](LICENSE).
