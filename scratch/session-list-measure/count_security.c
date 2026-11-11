// Excluded attribution runs only. Log successful child PIDs, never argv or output.
#include <spawn.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdio.h>
static void record_spawn(const char *path, pid_t *pid, int result) {
    const char *name = strrchr(path, '/');
    name = name ? name + 1 : path;
    const char *log = getenv("MEASURE_SECURITY_SPAWNS");
    if (result || !pid || !log || strcmp(name, "security")) return;
    int fd = open(log, O_WRONLY | O_APPEND | O_CREAT, 0600);
    if (fd >= 0) {
        char buf[64];
        int n = snprintf(buf, sizeof(buf), "%d %d\n", getpid(), *pid);
        write(fd, buf, n);
        close(fd);
    }
}
static int counted_spawn(pid_t *pid, const char *path, const posix_spawn_file_actions_t *actions,
                         const posix_spawnattr_t *attr, char *const argv[], char *const envp[]) {
    int result = posix_spawn(pid, path, actions, attr, argv, envp);
    record_spawn(path, pid, result);
    return result;
}
static int counted_spawnp(pid_t *pid, const char *path, const posix_spawn_file_actions_t *actions,
                          const posix_spawnattr_t *attr, char *const argv[], char *const envp[]) {
    int result = posix_spawnp(pid, path, actions, attr, argv, envp);
    record_spawn(path, pid, result);
    return result;
}
#define INTERPOSE(replacement, original) \
 __attribute__((used)) static struct { const void *a; const void *b; } \
 pair_##original __attribute__((section("__DATA,__interpose"))) = \
 {(const void *)(uintptr_t)&replacement, (const void *)(uintptr_t)&original}
INTERPOSE(counted_spawn, posix_spawn);
INTERPOSE(counted_spawnp, posix_spawnp);
