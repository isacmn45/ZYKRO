#define _GNU_SOURCE
#include <pthread.h>
#include <dlfcn.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <dirent.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
static int (*real_join)(pthread_t, void **);
static pthread_once_t once = PTHREAD_ONCE_INIT;
static void load_join(void) { *(void **)(&real_join) = dlsym(RTLD_NEXT, "pthread_join"); }
int pthread_join(pthread_t thread, void **result) {
    pthread_once(&once, load_join);
    char name[16] = {0};
    if (pthread_getname_np(thread, name, sizeof(name)) == 0 &&
        (!strncmp(name, "swag-video-deco", 15) || !strncmp(name, "swag-clipboard-", 15))) {
        DIR *dir = opendir("/proc/self/fd");
        if (dir) {
            struct dirent *entry;
            int released = 0;
            while ((entry = readdir(dir))) {
                char *end;
                long fd = strtol(entry->d_name, &end, 10);
                if (!*entry->d_name || *end || fd < 0) continue;
                struct sockaddr_un peer = {0};
                socklen_t size = sizeof(peer);
                if (!getpeername((int)fd, (struct sockaddr *)&peer, &size) &&
                    peer.sun_family == AF_UNIX &&
                    !strncmp(peer.sun_path, "/data/local/tmp/swag_sockets/", 28)) {
                    if (!shutdown((int)fd, SHUT_RDWR)) released++;
                }
            }
            closedir(dir);
            fprintf(stderr, "[swag-closefix] joining %s: released %d stream sockets\n", name, released);
        }
    }
    return real_join(thread, result);
}
