#!/bin/sh
# Construye la imagen de Android de ZYKRO (zykro-android:13, ver Dockerfile). Baja una vez, y verifica,
# Houdini 14 y MindTheGapps; los guarda en ~/.cache/zykro/descargas. Si nada cambió, Docker reutiliza
# lo anterior.
set -e
cd "$(dirname "$0")"
IMAGEN=zykro-android:13
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/zykro/descargas"
mkdir -p "$CACHE" descargas

bajar() { # archivo url sha256
    if ! echo "$3  $CACHE/$1" | sha256sum -c --status 2>/dev/null; then
        echo "Descargando $1…"
        curl -fL --retry 3 -o "$CACHE/$1.parcial" "$2"
        echo "$3  $CACHE/$1.parcial" | sha256sum -c --status || { echo "La descarga de $1 no coincide"; exit 1; }
        mv "$CACHE/$1.parcial" "$CACHE/$1"
    fi
    ln -f "$CACHE/$1" "descargas/$1" 2>/dev/null || cp "$CACHE/$1" "descargas/$1"
}
# Houdini 14 de Intel, del repositorio que usan Waydroid y waydroid_script.
bajar houdini13.zip https://github.com/supremegamers/vendor_intel_proprietary_houdini/archive/2f8f088671182e17e67321e098e8411a3972a628.zip \
    2e82cdc88ddc4d418f7fb861aeebb7c49c83a91b97f57da10510b5e1146a4ed5
bajar gapps13.zip https://github.com/MindTheGapps/13.0.0-x86_64/releases/download/MindTheGapps-13.0.0-x86_64-20231025_201203/MindTheGapps-13.0.0-x86_64-20231025_201203.zip \
    2076179bcb6f30e78853d52cf70a4bd1d27502c3852332195e5356816cddfdd9

cp ../servidor/zykro-servidor.jar ../superposiciones/configuracion.apk ../superposiciones/sistema.apk \
    ../superposiciones/marco.apk .
# Sin buildx, Docker avisa que su constructor viejo está obsoleto (funciona igual): solo se
# muestran los errores.
if ! salida=$(docker build -q -t "$IMAGEN" . 2>&1); then
    echo "$salida" | grep -v -E "DEPRECATED|legacy builder|buildx|^ *$"
    exit 1
fi
rm -f zykro-servidor.jar configuracion.apk sistema.apk marco.apk
echo "Imagen lista: $IMAGEN"
