#!/usr/bin/env bash
set -euo pipefail

echo "=== Instalando ThermalWatch ==="

# 1. Compilar binario en Rust
echo "==> Compilando binario en Rust..."
cargo build --release

# 2. Instalar binario en ~/.local/bin
mkdir -p "$HOME/.local/bin"
install -Dm755 target/release/thermalwatch "$HOME/.local/bin/thermalwatch-bin"

cat > "$HOME/.local/bin/thermalwatch" <<'EOF'
#!/bin/sh
exec "$HOME/.local/bin/thermalwatch-bin" "$@"
EOF
chmod +x "$HOME/.local/bin/thermalwatch"
echo "==> Binario instalado en ~/.local/bin/thermalwatch"

# 3. Crear archivo .desktop
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/dev.ina.thermalwatch.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=ThermalWatch
Comment=Hardware and temperature monitor with quick kill emergency popup
Exec=thermalwatch
Icon=utilities-system-monitor
Categories=System;Monitor;
StartupWMClass=dev.ina.thermalwatch
EOF

# 4. Regla de ventana flotante en Hyprland si no existe
HYPR_CONF="$HOME/.config/hypr/hyprland.lua"
if [ -f "$HYPR_CONF" ]; then
    if ! grep -q "dev.ina.thermalwatch.panic" "$HYPR_CONF"; then
        echo "==> Añadiendo regla de ventana en $HYPR_CONF..."
        echo -e '\n-- ThermalWatch: popup de kill de emergencia (SUPER + SHIFT + K)\no.window("dev.ina.thermalwatch.panic", { float = true, center = true, size = { 560, 240 }, stay_focused = true })' >> "$HYPR_CONF"
    fi
fi

# 5. Atajo de teclado en Hyprland si no existe
HYPR_BINDS="$HOME/.config/hypr/bindings.lua"
if [ -f "$HYPR_BINDS" ]; then
    if ! grep -q "thermalwatch --panic" "$HYPR_BINDS"; then
        echo "==> Añadiendo atajo de teclado en $HYPR_BINDS..."
        echo -e '\n-- ThermalWatch: popup con r/c/d para matar el proceso que más RAM/CPU/disco usa (sin confirmación)\no.bind("SUPER + SHIFT + K", "Kill top RAM/CPU/disco", "thermalwatch --panic")' >> "$HYPR_BINDS"
    fi
fi

if command -v hyprctl >/dev/null 2>&1; then
    hyprctl reload >/dev/null 2>&1 || true
fi

echo "=== ¡Instalación completada! ==="
echo "Ejecuta: thermalwatch"
echo "Popup de emergencia: SUPER + SHIFT + K (o 'thermalwatch --panic')"
