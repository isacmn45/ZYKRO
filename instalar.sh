#!/bin/sh
# Instala nuestras mejoras de SwagPlayer en ~/.local.
# Vuelve a correrlo después de cambiar algo en el repositorio.
#   ./instalar.sh            -> scripts, programas compilados y acceso directo
#   ./instalar.sh --keymap   -> además copia nuestro mapeo de Free Fire
set -e
cd "$(dirname "$0")"
BIN="$HOME/.local/bin"
LIB="$HOME/.local/lib/swagplayer"
APPS="$HOME/.local/share/applications"
mkdir -p "$BIN" "$LIB" "$APPS"

# Programas en C. `install` reemplaza el archivo sin tocar el que esté en uso.
gcc -O2 -static -o /tmp/teclado-fisico.$$ src/teclado-fisico.c
install -m 755 /tmp/teclado-fisico.$$ "$LIB/teclado-fisico"
gcc -O2 -shared -fPIC -o /tmp/closefix.$$.so src/swag-closefix.c -ldl -lpthread
install -m 755 /tmp/closefix.$$.so "$LIB/closefix.so"
rm -f /tmp/teclado-fisico.$$ /tmp/closefix.$$.so

for f in bin/*; do install -m 755 "$f" "$BIN/"; done

sed "s|@HOME@|$HOME|g" desktop/com.swagplayer.SwagPlayer.desktop > "$APPS/com.swagplayer.SwagPlayer.desktop"
update-desktop-database "$APPS" 2>/dev/null || true

if [ "$1" = "--keymap" ]; then
    mkdir -p "$HOME/.config/swagplayer/keymap"
    cp config/keymap/*.json "$HOME/.config/swagplayer/keymap/"
    echo "Mapeo copiado."
fi

echo "Listo. Abre SwagPlayer desde su ícono."
