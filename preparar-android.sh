#!/bin/sh
# Prepara el Android de ZYKRO: Docker, los dispositivos binder, uinput y la imagen de Android
# (imagen/). El contenedor lo crea ZYKRO la primera vez que se abre. Cada paso se salta si ya está
# hecho, así que se puede volver a correr. Pide tu contraseña (sudo) solo para lo que es del sistema.
# Lo llama instalar.sh.
set -e
cd "$(dirname "$0")"
. ./instalacion/sistema.sh

paso() { printf '\n== %s\n' "$1"; }

paso "Docker"
if ! command -v docker >/dev/null; then
    echo 'Falta Docker. Ejecuta ./instalar.sh para instalar las dependencias automáticamente.'
    exit 1
fi
if ! command -v systemctl >/dev/null; then
    echo 'La preparación automática necesita una distribución con systemd.'
    exit 1
fi
if ! systemctl is-active -q docker; then
    echo "Docker no está corriendo: se activa para que arranque siempre (pide tu contraseña)."
    zykro_raiz systemctl enable --now docker
fi
if ! docker info >/dev/null 2>&1; then
    # Los grupos nuevos no llegan a esta sesión. Completar la preparación con Docker como root,
    # conservando las descargas, el código y la compilación en manos del usuario.
    if zykro_raiz -H docker info >/dev/null 2>&1; then
        ZYKRO_DOCKER_SUDO=1
        export ZYKRO_DOCKER_SUDO
        echo 'Se completará la preparación de Docker; vuelve a iniciar sesión antes de abrir ZYKRO.'
    else
        echo 'Docker no responde. Revisa el servicio con: systemctl status docker'
        exit 1
    fi
fi
# Las versiones nuevas de Docker necesitan el complemento de construcción.
if ! zykro_docker buildx version >/dev/null 2>&1; then
    zykro_distribucion
    case "$ZYKRO_FAMILIA" in
        arch|fedora) ZYKRO_PAQUETES=docker-buildx; zykro_instalar_paquetes ;;
        debian)
            if apt-cache show docker-buildx >/dev/null 2>&1; then
                ZYKRO_PAQUETES=docker-buildx; zykro_instalar_paquetes
            fi ;;
    esac
fi
echo "Listo."

paso "Binder (Android lo necesita para arrancar)"
if [ -e /dev/binderfs/binder ]; then
    echo "Listo."
else
    # Algunos kernels (Ubuntu, Debian) lo traen como módulo: se carga ahora y en cada arranque.
    if ! grep -qw binder /proc/filesystems && zykro_binder_extra; then
        echo "Se carga el módulo binder_linux, también en cada arranque (pide tu contraseña)."
        echo binder_linux | zykro_archivo_sistema /etc/modules-load.d/zykro-binder.conf
        zykro_raiz modprobe binder_linux
    fi
    if ! grep -qw binder /proc/filesystems; then
        echo 'El kernel actual no ofrece binder (CONFIG_ANDROID_BINDERFS), necesario para Android.'
        echo 'Instala o arranca un kernel compatible y vuelve a ejecutar ./instalar.sh.'
        exit 1
    fi
    servicio=zykro-binderfs.service
    if ! systemctl cat $servicio >/dev/null 2>&1; then
        echo "Se instala el servicio $servicio, que monta binder en cada arranque (pide tu contraseña)."
        zykro_archivo_sistema /etc/systemd/system/$servicio <<'FIN'
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
        zykro_raiz systemctl daemon-reload
        zykro_raiz systemctl enable "$servicio"
    fi
    zykro_raiz systemctl start "$servicio"
    echo "Listo."
fi

paso "uinput (para que Android no muestre su teclado en pantalla)"
if ! grep -rqsx uinput /etc/modules-load.d /usr/lib/modules-load.d; then
    echo "Se carga uinput en cada arranque (pide tu contraseña)."
    echo uinput | zykro_archivo_sistema /etc/modules-load.d/zykro.conf
fi
[ -e /dev/uinput ] || zykro_raiz modprobe uinput
echo "Listo."

paso "Imagen de Android"
# Android 13 de redroid (código abierto) con Houdini 14, Google Play y el servidor de ZYKRO. La primera
# vez baja unos 1.1 GB. Si algo cambió, la imagen se actualiza y ZYKRO recrea el contenedor con ella
# la próxima vez que lo abras (mismos datos).
if ! zykro_docker image inspect redroid/redroid:13.0.0-latest >/dev/null 2>&1; then
    echo "Se descarga Android 13 (redroid, unos 840 MB)."
    zykro_docker pull redroid/redroid:13.0.0-latest
fi
zykro_docker image inspect alpine:3.20 >/dev/null 2>&1 || zykro_docker pull -q alpine:3.20 >/dev/null
sh imagen/construir.sh
