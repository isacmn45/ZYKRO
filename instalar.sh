#!/bin/sh
# Instala ZYKRO en ~/.local.
# Vuelve a correrlo después de actualizar el repositorio.
#   ./instalar.sh            -> ZYKRO, su ayudante del teclado y el acceso directo
#   ./instalar.sh --keymap   -> además copia nuestro mapeo de Free Fire (reemplaza el tuyo)
set -e
cd "$(dirname "$0")"
BIN="$HOME/.local/bin"
LIB="$HOME/.local/lib/zykro"
APPS="$HOME/.local/share/applications"
mkdir -p "$BIN" "$LIB" "$APPS"

if ! command -v cargo >/dev/null; then
    echo "Hace falta Rust (cargo) para compilar ZYKRO."
    exit 1
fi

# Teclado "físico" vacío para que Android oculte su teclado en pantalla (lo usa teclado-android).
# `install` reemplaza el archivo sin tocar el que esté en uso.
gcc -O2 -static -o /tmp/teclado-fisico.$$ src/teclado-fisico.c
install -m 755 /tmp/teclado-fisico.$$ "$LIB/teclado-fisico"
rm -f /tmp/teclado-fisico.$$
install -m 755 bin/teclado-android "$BIN/teclado-android"

# ZYKRO: se compila con prioridad baja para no quitarle CPU a Android si está corriendo.
(cd zykro && nice -n 19 cargo build --release)
install -m 755 zykro/target/release/zykro "$BIN/zykro"
install -Dm 644 desktop/zykro.svg "$HOME/.local/share/icons/hicolor/scalable/apps/zykro.svg"
sed "s|@HOME@|$HOME|g" desktop/zykro.desktop > "$APPS/zykro.desktop"

update-desktop-database "$APPS" 2>/dev/null || true
command -v kbuildsycoca6 >/dev/null && kbuildsycoca6 >/dev/null 2>&1 || true

if [ "$1" = "--keymap" ]; then
    mkdir -p "$HOME/.config/zykro/keymap"
    cp config/keymap/*.json "$HOME/.config/zykro/keymap/"
    echo "Mapeo copiado."
fi

echo "Listo. Abre ZYKRO desde su ícono."
