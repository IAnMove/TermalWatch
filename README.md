# 󰔏 ThermalWatch

Monitor de temperatura, CPU, RAM y escritura de disco diseñado para **Omarchy** y **Hyprland**, escrito en **Rust** (GTK4 + Libadwaita + Cairo) con widget nativo para la barra de Omarchy (`Quickshell`).

---

## ✨ Características

- 🌡️ **Medidor de temperatura dinámico:** Arco térmico que cambia de color y tiembla/pulsa con animación de fuego y brasas al superar temperaturas críticas (88°C+).
- 📊 **Métricas clave:**
  - Temperatura CPU (Tctl / Coretemp / ACPI).
  - Sensores secundarios: GPU, NVMe y ventiladores (RPM).
  - % CPU y carga del sistema.
  - Memoria RAM usada / total.
  - Tasa de escritura y lectura de disco (KB/s o MB/s).
- ⚡ **Skin nativo de Omarchy:** Adopta automáticamente la paleta de colores y la tipografía (`JetBrainsMono Nerd Font`) del tema activo de Omarchy (`~/.local/state/omarchy/current/theme/colors.toml`) y se recolorea en caliente si cambias de tema.
- 🚨 **Popup de pánico para emergencias (`SUPER + SHIFT + K`):**
  - Muestra al instante el proceso que más RAM (`R`), CPU (`C`) o disco (`D`) está consumiendo.
  - Pulsa una sola tecla (`R`, `C` o `D`) para terminar el proceso inmediatamente (`SIGKILL`) sin diálogos ni bloqueos.
  - Pulsa `Esc` o haz clic fuera para salir.
- 🌐 **Multilenguaje automático:** Detecta el idioma del sistema (`en`, `es`, `fr`, `de`, `pt`, `it`).
- 🧩 **Plugin de barra para Omarchy:** Widget en la barra superior con temperatura en vivo y accesos directos.

---

## 🚀 Instalación como Plugin de Omarchy

Para añadir el widget a la barra superior de Omarchy:

```bash
omarchy plugin add https://github.com/IAnMove/TermalWatch --enable
```

Para mover el widget en la barra (izquierda, centro o derecha):

```bash
omarchy bar move io.github.ianmove.thermalwatch --section right
```

---

## 🛠️ Instalación de la Aplicación y Atajos (Rust)

Clona el repositorio y ejecuta el instalador:

```bash
git clone https://github.com/IAnMove/TermalWatch.git
cd TermalWatch
./install.sh
```

El script compila el binario en Release, lo instala en `~/.local/bin/thermalwatch`, crea el lanzador `.desktop` y registra el atajo de teclado en Hyprland.

---

## ⌨️ Atajos de teclado

- **`SUPER + SHIFT + K`**: Abre el popup de eliminación rápida.
  - **`R`**: Termina inmediatamente el mayor consumidor de RAM.
  - **`C`**: Termina inmediatamente el mayor consumidor de CPU.
  - **`D`**: Termina inmediatamente el mayor consumidor de disco.
  - **`Esc`** o clic fuera: Cancelar.

---

## 📜 Licencia

MIT License © 2026 IAnMove
