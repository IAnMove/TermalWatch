# 󰔏 ThermalWatch

A lightweight, beautiful hardware and thermal monitor crafted for **Omarchy** and **Hyprland**, built with **Rust** (GTK4 + Libadwaita + Cairo) and featuring a native status bar widget for the Omarchy shell (`Quickshell`).

---

## ✨ Features

- 🌡️ **Dynamic Thermal Gauge:**
  - Radial temperature arc that transitions across cold, temperate, warm, hot, and critical colors.
  - Flame pulse and ember animation activated during high thermal strain (88°C+).
  - Reads CPU thermal zones (`k10temp` / `coretemp` / `acpitz`), GPU, NVMe, and cooling fan RPMs.
- 📊 **Real-Time Resource Metrics:**
  - **CPU:** Total usage percentage and system load average with live sparkline history.
  - **RAM:** Active memory consumption, percentage, and total system RAM with trend graph.
  - **Disk I/O:** Real-time write and read throughput (KB/s or MB/s) with throughput sparkline.
  - **Process Inspector:** Sort top processes by CPU, RAM, or Disk write activity with quick-kill capability.
- 🎨 **Native Omarchy Theming:**
  - Automatically loads the active Omarchy theme colors (`~/.local/state/omarchy/current/theme/colors.toml`) and typography (`JetBrainsMono Nerd Font`).
  - Hot-reloads in real time without restarts whenever you run `omarchy theme set <theme>`.
  - Minimalist sharp-cornered aesthetics matching the terminal and status bar.
- 🚨 **Emergency Panic-Kill Popup (`SUPER + SHIFT + K`):**
  - Floats centered above all windows during freezing or runaway load situations.
  - Identifies the highest resource hogs across **RAM** (`R`), **CPU** (`C`), and **Disk** (`D`).
  - Press a single key (`R`, `C`, or `D`) to immediately terminate the process tree (`SIGKILL`) with zero confirmation delays.
  - Dismiss anytime with `Esc` or by clicking anywhere outside the popup.
- 🌐 **Automatic Multi-Language:**
  - Detects system locale (`LANG`, `LC_MESSAGES`, `/etc/locale.conf`).
  - Built-in support for **English**, **Spanish**, **French**, **German**, **Portuguese**, and **Italian**.
- 🧩 **Native Omarchy Bar Widget:**
  - Sits directly in the top status bar showing live temperature badges (e.g. `󰔏 65°` or `🔥 91°`).
  - **Left-Click:** Open or focus the full ThermalWatch monitor.
  - **Right-Click:** Instantly trigger the emergency panic-kill popup.

---

## 🚀 Quick Install

### 1. As an Omarchy Shell Bar Plugin

Add the widget directly to your Omarchy status bar:

```bash
omarchy plugin add https://github.com/IAnMove/TermalWatch --enable
```

To move the widget to your preferred position (left, center, or right):

```bash
omarchy bar move io.github.ianmove.thermalwatch --section right
```

---

### 2. Standalone Application & Keybindings (Rust)

Clone the repository and run the automated installer:

```bash
git clone https://github.com/IAnMove/TermalWatch.git
cd TermalWatch
./install.sh
```

What `install.sh` does:
1. Compiles the optimized release binary via `cargo build --release`.
2. Installs the binary into `~/.local/bin/thermalwatch`.
3. Registers the desktop entry `~/.local/share/applications/dev.ina.thermalwatch.desktop`.
4. Adds the floating centered window rule to `~/.config/hypr/hyprland.lua`.
5. Adds the `SUPER + SHIFT + K` panic shortcut to `~/.config/hypr/bindings.lua`.
6. Reloads Hyprland configuration cleanly.

---

## ⌨️ Keybindings

| Shortcut | Action | Description |
| :--- | :--- | :--- |
| `SUPER + SHIFT + K` | **Panic Kill Popup** | Opens the emergency process-terminator dialog. |
| `R` *(inside popup)* | **Kill Top RAM** | Immediately terminates the highest memory consumer. |
| `C` *(inside popup)* | **Kill Top CPU** | Immediately terminates the highest CPU consumer. |
| `D` *(inside popup)* | **Kill Top Disk** | Immediately terminates the highest disk writer. |
| `Esc` / *Click outside* | **Dismiss** | Closes the popup safely without killing any processes. |

---

## 🛠️ Tech Stack & Architecture

- **Backend / Core GUI:** Rust 2024 with `gtk4-rs`, `libadwaita-rs`, and `cairo-rs`.
- **Bar Widget:** QML / Quickshell integrated into `omarchy-shell`.
- **System Telemetry:** Direct non-blocking readers for `/proc/stat`, `/proc/meminfo`, `/proc/diskstats`, and `/sys/class/hwmon/` with zero third-party daemon dependencies.
- **Adaptive Performance:** Dynamic render-throttling (30 FPS max during animations, sleeping at 0 FPS when idle) preventing CPU overhead on 144Hz+ high-refresh displays.

---

## 📄 License

Distributed under the [MIT License](LICENSE).
