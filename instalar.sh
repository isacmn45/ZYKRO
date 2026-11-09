#!/bin/sh
# Instala ZYKRO en ~/.local y prepara Android (preparar-android.sh).
# Vuelve a correrlo después de actualizar el repositorio.
#   ./instalar.sh               -> ZYKRO, sus ayudantes, el acceso directo y Android
#   ./instalar.sh --keymap      -> además copia nuestro mapeo de Free Fire (reemplaza el tuyo)
#   ./instalar.sh --sin-android -> sin preparar Android
#   ./instalar.sh --simular    -> muestra las dependencias sin instalar ni cambiar nada
#   ./instalar.sh --sin-dependencias -> usa las dependencias ya preparadas
set -e
cd "$(dirname "$0")"
MAPEO=0
ANDROID=1
DEPENDENCIAS=1
ZYKRO_SIMULAR=0
for opcion in "$@"; do
    case "$opcion" in
        --keymap) MAPEO=1 ;;
        --sin-android) ANDROID=0 ;;
        --sin-dependencias) DEPENDENCIAS=0 ;;
        --simular) ZYKRO_SIMULAR=1 ;;
        --ayuda|-h|--help)
            echo 'Uso: ./instalar.sh [--keymap] [--sin-android] [--sin-dependencias] [--simular]'
            exit 0 ;;
        *) echo "Opción desconocida: $opcion"; exit 2 ;;
    esac
done
. ./instalacion/sistema.sh
if [ "$DEPENDENCIAS" = 1 ]; then
    zykro_dependencias "$ANDROID"
else
    zykro_entorno
fi
if [ "$ZYKRO_SIMULAR" = 1 ]; then
    echo 'Simulación: después se compilaría ZYKRO y se prepararía Android (salvo --sin-android).'
    exit 0
fi
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
TEMPORAL=$(mktemp -d)
trap 'rm -rf "$TEMPORAL"' EXIT
gcc -O2 -static -o "$TEMPORAL/teclado-fisico" src/teclado-fisico.c
install -m 755 "$TEMPORAL/teclado-fisico" "$LIB/teclado-fisico"
install -m 755 bin/teclado-android "$BIN/teclado-android"

# Pantalla de inicio de Android (launcher/): ZYKRO la instala en Android al arrancar si falta o cambió.
install -Dm 644 launcher/zykro-inicio.apk "$HOME/.local/share/zykro/zykro-inicio.apk"

# ZYKRO: se compila con prioridad baja para no quitarle CPU a Android si está corriendo.
(cd zykro && nice -n 19 cargo build --release --locked)
install -m 755 zykro/target/release/zykro "$BIN/zykro.nuevo"
mv -f "$BIN/zykro.nuevo" "$BIN/zykro"
install -Dm 644 desktop/zykro.svg "$HOME/.local/share/icons/hicolor/scalable/apps/zykro.svg"
sed "s|@HOME@|$HOME|g" desktop/zykro.desktop > "$APPS/zykro.desktop"

update-desktop-database "$APPS" 2>/dev/null || true
command -v kbuildsycoca6 >/dev/null && kbuildsycoca6 >/dev/null 2>&1 || true

if [ "$MAPEO" = 1 ]; then
    mkdir -p "$HOME/.config/zykro/keymap"
    cp config/keymap/*.json "$HOME/.config/zykro/keymap/"
    echo "Mapeo copiado."
fi

if [ "$ANDROID" = 1 ]; then
    sh preparar-android.sh
fi

echo
echo "Listo. Abre ZYKRO desde su ícono."
if [ "${ZYKRO_REINICIAR_SESION:-0}" = 1 ]; then
    echo 'Primero cierra tu sesión de Linux y vuelve a entrar: se agregaron permisos para Docker y los controles.'
fi
