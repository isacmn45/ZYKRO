#!/bin/sh
# Construye la imagen de Android de ZYKRO: swagplayer-local:13.0.0-houdini, con Houdini parcheado y la
# interfaz de Android compacta (superposiciones/). Si nada cambió, Docker reutiliza lo anterior.
set -e
cd "$(dirname "$0")"
BASE=geanswag/swagplayer:13.0.0

docker rm -f swag-houdini-tmp >/dev/null 2>&1 || true
docker create --name swag-houdini-tmp "$BASE" >/dev/null
docker cp -q swag-houdini-tmp:/system/lib64/libhoudini.so libhoudini.so
docker rm swag-houdini-tmp >/dev/null

python3 parchear.py libhoudini.so
cp ../superposiciones/configuracion.apk ../superposiciones/sistema.apk .
docker build -q -t swagplayer-local:13.0.0-houdini . >/dev/null
rm -f libhoudini.so configuracion.apk sistema.apk
echo "Imagen lista: swagplayer-local:13.0.0-houdini"
