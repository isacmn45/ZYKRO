#!/bin/sh
# Construye la imagen de Android de ZYKRO (zykro-android:9, ver Dockerfile). Baja una vez, y verifica,
# Houdini de Android-x86 y OpenGApps; los guarda en ~/.cache/zykro/descargas. Si nada cambió,
# Docker reutiliza lo anterior.
set -e
cd "$(dirname "$0")"
IMAGEN=zykro-android:9
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
bajar houdini.sfs http://dl.android-x86.org/houdini/9_y/houdini.sfs \
    b73d60265096acbeba374d45732f52ef9601577ae2093c950b5e0a61b575d375
bajar gapps.zip https://sourceforge.net/projects/opengapps/files/x86_64/20220503/open_gapps-x86_64-9.0-pico-20220503.zip/download \
    319272f336679b32c5ada48a4b69eef35088be61e6cc73a533f66642f3ced244

cp ../servidor/zykro-servidor.jar ../superposiciones/marco.apk .
# Sin buildx, Docker avisa que su constructor viejo está obsoleto (funciona igual): solo se
# muestran los errores.
if ! salida=$(docker build -q -t "$IMAGEN" . 2>&1); then
    echo "$salida" | grep -v -E "DEPRECATED|legacy builder|buildx|^ *$"
    exit 1
fi
rm -f zykro-servidor.jar marco.apk
echo "Imagen lista: $IMAGEN"
