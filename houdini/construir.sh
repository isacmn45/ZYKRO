#!/bin/sh
# Construye la imagen de Android con Houdini parcheado: swagplayer-local:13.0.0-houdini
set -e
cd "$(dirname "$0")"
BASE=geanswag/swagplayer:13.0.0

docker rm -f swag-houdini-tmp >/dev/null 2>&1 || true
docker create --name swag-houdini-tmp "$BASE" >/dev/null
docker cp -q swag-houdini-tmp:/system/lib64/libhoudini.so libhoudini.so
docker rm swag-houdini-tmp >/dev/null

python3 parchear.py libhoudini.so
docker build -t swagplayer-local:13.0.0-houdini .
rm -f libhoudini.so
echo "Imagen lista. En ~/.config/swagplayer/state.json: \"docker_image\": \"swagplayer-local:13.0.0-houdini\""
