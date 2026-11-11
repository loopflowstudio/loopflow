// Measurement only: hold wall time fixed without changing elapsed/monotonic time.
#include <time.h>
#include <sys/time.h>
#include <stdlib.h>
#include <stdint.h>
static time_t frozen(void) { return (time_t)atoll(getenv("MEASURE_EPOCH")); }
static int fixed_clock(clockid_t id, struct timespec *out) {
    if (id == CLOCK_REALTIME) { out->tv_sec = frozen(); out->tv_nsec = 0; return 0; }
    return clock_gettime(id, out);
}
static int fixed_timeval(struct timeval *out, void *zone) {
    (void)zone; out->tv_sec = frozen(); out->tv_usec = 0; return 0;
}
static time_t fixed_time(time_t *out) { time_t t = frozen(); if (out) *out=t; return t; }
#define INTERPOSE(replacement, original) \
 __attribute__((used)) static struct { const void *a; const void *b; } \
 pair_##original __attribute__((section("__DATA,__interpose"))) = \
 {(const void *)(uintptr_t)&replacement, (const void *)(uintptr_t)&original}
INTERPOSE(fixed_clock, clock_gettime);
INTERPOSE(fixed_timeval, gettimeofday);
INTERPOSE(fixed_time, time);
