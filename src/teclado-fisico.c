// Crea un teclado "físico" vacío dentro de Android (uinput) para que Android
// oculte el teclado en pantalla, como hace BlueStacks. No envía ninguna tecla.
// Se ejecuta dentro del contenedor redroid; al terminar, el teclado desaparece.
#include <dirent.h>
#include <fcntl.h>
#include <linux/uinput.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <unistd.h>

static char nodo[64];

static void salir(int sig) {
    (void)sig;
    if (nodo[0]) unlink(nodo);
    _exit(0);
}

// Redroid no crea el /dev/input/eventN por sí solo: lo creamos a mano
// leyendo el major:minor que el kernel publica en sysfs.
static int crear_nodo(const char *sysname) {
    char dir[128], ruta[256];
    snprintf(dir, sizeof dir, "/sys/devices/virtual/input/%s", sysname);
    for (int intento = 0; intento < 50; intento++) {
        DIR *d = opendir(dir);
        struct dirent *e;
        while (d && (e = readdir(d))) {
            if (strncmp(e->d_name, "event", 5) != 0) continue;
            unsigned ma, mi;
            snprintf(ruta, sizeof ruta, "%s/%s/dev", dir, e->d_name);
            FILE *f = fopen(ruta, "r");
            if (!f) continue;
            int ok = fscanf(f, "%u:%u", &ma, &mi) == 2;
            fclose(f);
            if (!ok) continue;
            snprintf(nodo, sizeof nodo, "/dev/input/%s", e->d_name);
            closedir(d);
            mkdir("/dev/input", 0755);
            unlink(nodo);
            // Permisos abiertos desde el inicio: EventHub lo abre apenas aparece.
            umask(0);
            return mknod(nodo, S_IFCHR | 0666, makedev(ma, mi));
        }
        if (d) closedir(d);
        usleep(100000);
    }
    return -1;
}

int main(void) {
    int fd = open("/dev/uinput", O_WRONLY | O_NONBLOCK);
    if (fd < 0) return 1;
    ioctl(fd, UI_SET_EVBIT, EV_KEY);
    for (int k = KEY_ESC; k <= KEY_KPDOT; k++) ioctl(fd, UI_SET_KEYBIT, k);

    struct uinput_setup us;
    memset(&us, 0, sizeof us);
    us.id.bustype = BUS_USB;
    us.id.vendor = 0x5357;   // "SW"
    us.id.product = 0x4b42;  // "KB"
    strcpy(us.name, "SwagPlayer Teclado Fisico");
    if (ioctl(fd, UI_DEV_SETUP, &us) < 0 || ioctl(fd, UI_DEV_CREATE) < 0) return 1;

    char sysname[64] = {0};
    if (ioctl(fd, UI_GET_SYSNAME(sizeof sysname), sysname) < 0 || crear_nodo(sysname) < 0)
        return 1;

    signal(SIGTERM, salir);
    signal(SIGINT, salir);
    for (;;) pause();
}
