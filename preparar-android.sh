#!/bin/sh
# Prepara el Android de ZYKRO sin SwagPlayer: Docker, los dispositivos binder, uinput, la imagen de
# Android y su contenedor. Cada paso se salta si ya está hecho, así que se puede volver a correr.
# Pide tu contraseña (sudo) solo para lo que es del sistema. Lo llama instalar.sh.
set -e
cd "$(dirname "$0")"

# Se pueden cambiar para hacer pruebas sin tocar el Android de verdad.
CONTENEDOR="${ZYKRO_CONTENEDOR:-redroid-redroid-1}"
VOLUMEN="${ZYKRO_VOLUMEN:-redroid_redroid_data}"
SOCKETS="${ZYKRO_SOCKETS:-/tmp/swag_sockets}"
PUERTO_ADB="${ZYKRO_PUERTO_ADB-127.0.0.1:5555}"

# Android 13 de SwagPlayer (redroid con swag-rust-server, el servidor de video y control que usa ZYKRO)
# y nuestra versión con Houdini parcheado (houdini/).
BASE=geanswag/swagplayer:13.0.0
IMAGEN=swagplayer-local:13.0.0-houdini

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
if docker image inspect "$IMAGEN" >/dev/null 2>&1; then
    echo "Listo: $IMAGEN"
else
    if ! docker image inspect "$BASE" >/dev/null 2>&1; then
        echo "Se descarga $BASE (unos 6 GB; tarda según tu internet)."
        docker pull "$BASE"
    fi
    sh houdini/construir.sh
fi

paso "Contenedor de Android"
if docker container inspect "$CONTENEDOR" >/dev/null 2>&1; then
    echo "Listo: $CONTENEDOR (tus apps y datos se conservan)."
else
    mkdir -p "$SOCKETS"
    # Los mismos ajustes que usaba SwagPlayer. Resolución, DPI, FPS y texturas salen de los valores
    # iniciales de ZYKRO y se cambian desde Configuración. Se hace pasar por un Sony Xperia 1 V. Usa
    # una batería falsa fija: si no, lee la de la laptop y se apaga solo al verla en 0 % sin cargador.
    set -- --name "$CONTENEDOR" --privileged --security-opt label=disable --shm-size 64m \
        -v "$SOCKETS:/data/local/tmp/swag_sockets" -v "$VOLUMEN:/data" --device /dev:/dev:rwm \
        --device-cgroup-rule 'c 10:223 rmw' --device-cgroup-rule 'c 13:* rmw'
    [ -e /dev/kvm ] && set -- "$@" --device /dev/kvm:/dev/kvm:rwm
    [ -n "$PUERTO_ADB" ] && set -- "$@" -p "$PUERTO_ADB:5555"
    docker create "$@" --entrypoint /init "$IMAGEN" \
        androidboot.hardware=redroid \
        androidboot.redroid_width=1280 androidboot.redroid_height=720 androidboot.redroid_dpi=240 \
        androidboot.redroid_fps=60 androidboot.qemu.vsync=60 \
        androidboot.swag_container=1 androidboot.redroid_gpu_mode=host \
        androidboot.fake_battery=1 \
        debug.mesa.swag.lod.bias.enabled=1 debug.mesa.swag.lod.bias=0.50 \
        ro.product.cpu.abilist=x86_64,arm64-v8a,x86,armeabi-v7a,armeabi \
        ro.product.cpu.abilist64=x86_64,arm64-v8a ro.product.cpu.abilist32=x86,armeabi-v7a,armeabi \
        ro.dalvik.vm.isa.arm=x86 ro.dalvik.vm.isa.arm64=x86_64 \
        ro.enable.native.bridge.exec=1 ro.enable.native.bridge.exec64=1 ro.dalvik.vm.native.bridge=libhoudini.so \
        debug.sf.latch_unsignaled=1 ro.surface_flinger.max_frame_buffer_acquired_buffers=3 \
        dalvik.vm.heapsize=512m dalvik.vm.heapgrowthlimit=256m \
        ro.product.model=XQ-DQ72 ro.product.brand=Sony ro.product.name=XQ-DQ72 ro.product.device=XQ-DQ72 ro.product.manufacturer=Sony \
        ro.product.system.model=XQ-DQ72 ro.product.system.brand=Sony ro.product.system.name=XQ-DQ72 ro.product.system.device=XQ-DQ72 ro.product.system.manufacturer=Sony \
        ro.product.vendor.model=XQ-DQ72 ro.product.vendor.brand=Sony ro.product.vendor.name=XQ-DQ72 ro.product.vendor.device=XQ-DQ72 ro.product.vendor.manufacturer=Sony \
        ro.product.product.model=XQ-DQ72 ro.product.product.brand=Sony ro.product.product.name=XQ-DQ72 ro.product.product.device=XQ-DQ72 ro.product.product.manufacturer=Sony \
        ro.product.odm.model=XQ-DQ72 ro.product.odm.brand=Sony ro.product.odm.name=XQ-DQ72 ro.product.odm.device=XQ-DQ72 ro.product.odm.manufacturer=Sony \
        ro.build.product=XQ-DQ72 ro.build.flavor=XQ-DQ72-user ro.build.tags=release-keys ro.build.type=user \
        "ro.build.description=XQ-DQ72-user 13 67.0.A.4.51 067000A004005100 release-keys" \
        ro.build.fingerprint=Sony/XQ-DQ72/XQ-DQ72:13/67.0.A.4.51/067000A004005100:user/release-keys \
        >/dev/null
    echo "Creado: $CONTENEDOR. El primer arranque de Android tarda un par de minutos."
fi
