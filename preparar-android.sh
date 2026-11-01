#!/bin/sh
# Prepara el Android de ZYKRO: Docker, los dispositivos binder, uinput y la imagen de Android
# (imagen/). El contenedor lo crea ZYKRO la primera vez que se abre. Cada paso se salta si ya está
# hecho, así que se puede volver a correr. Pide tu contraseña (sudo) solo para lo que es del sistema.
# Lo llama instalar.sh.
set -e
cd "$(dirname "$0")"

paso() { printf '\n== %s\n' "$1"; }

paso "Docker"
if ! command -v docker >/dev/null; then
    echo "Falta Docker. En Arch o CachyOS: sudo pacman -S docker"
    exit 1
fi
if ! systemctl is-active -q docker; then
    echo "Docker no está corriendo: se activa para que arranque siempre (pide tu contraseña)."
    sudo systemctl enable --now docker
fi
if ! docker info >/dev/null 2>&1; then
    echo "Tu usuario no puede usar Docker. Agrégalo al grupo y vuelve a iniciar sesión:"
    echo "  sudo usermod -aG docker,input $USER"
    exit 1
fi
echo "Listo."

paso "Binder (Android lo necesita para arrancar)"
if [ -e /dev/binderfs/binder ]; then
    echo "Listo."
else
    if ! grep -qw binder /proc/filesystems; then
        echo "Tu kernel no trae binder (CONFIG_ANDROID_BINDERFS). Usa un kernel que lo tenga, como el de CachyOS."
        exit 1
    fi
    if systemctl cat swagplayer-binderfs.service >/dev/null 2>&1; then
        servicio=swagplayer-binderfs.service
    else
        servicio=zykro-binderfs.service
        echo "Se instala el servicio $servicio, que monta binder en cada arranque (pide tu contraseña)."
        sudo tee /etc/systemd/system/$servicio >/dev/null <<'FIN'
[Unit]
Description=Dispositivos binder para el Android de ZYKRO
DefaultDependencies=no
Before=docker.service
ConditionPathExists=!/dev/binderfs/binder

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/bin/sh -c 'mkdir -p /dev/binderfs && (mountpoint -q /dev/binderfs || mount -t binder binder /dev/binderfs) && chmod 755 /dev/binderfs && find /dev/binderfs -maxdepth 1 -type c -exec chmod 666 {} +'

[Install]
WantedBy=multi-user.target
FIN
        sudo systemctl daemon-reload
        sudo systemctl enable $servicio
    fi
    sudo systemctl start $servicio
    echo "Listo."
fi

paso "uinput (para que Android no muestre su teclado en pantalla)"
if ! grep -rqsx uinput /etc/modules-load.d /usr/lib/modules-load.d; then
    echo "Se carga uinput en cada arranque (pide tu contraseña)."
    echo uinput | sudo tee /etc/modules-load.d/zykro.conf >/dev/null
fi
[ -e /dev/uinput ] || sudo modprobe uinput
echo "Listo."

paso "Imagen de Android"
# Android 13 de redroid (código abierto) con Houdini 14, Google Play y el servidor de ZYKRO. La primera
# vez baja unos 1.1 GB. Si algo cambió, la imagen se actualiza y ZYKRO recrea el contenedor con ella
# la próxima vez que lo abras (mismos datos).
if ! docker image inspect redroid/redroid:13.0.0-latest >/dev/null 2>&1; then
    echo "Se descarga Android 13 (redroid, unos 840 MB)."
    docker pull redroid/redroid:13.0.0-latest
fi
docker image inspect alpine:3.20 >/dev/null 2>&1 || docker pull -q alpine:3.20 >/dev/null
sh imagen/construir.sh
