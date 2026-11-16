// Expose command return racing the signal handler after its owned child dies.
// Linux-only test interposition; it does not change the delivered signal.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <signal.h>
#include <stdio.h>
#include <unistd.h>

int kill(pid_t pid, int sig) {
    int (*real_kill)(pid_t, int) = dlsym(RTLD_NEXT, "kill");
    int result = real_kill(pid, sig);
    if (pid < -1 && sig == SIGKILL && result == 0) {
        fprintf(stderr, "test ordering: owned group killed before interrupt hook returns\n");
        usleep(200000);
    }
    return result;
}
