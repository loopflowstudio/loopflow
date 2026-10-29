// Loopflow's reads inside an app, in Ghostty's embedding shape. Every entry
// point takes a JSON `Read` naming one `lf` command
// (`{"read":"task_file","issue":"FIX-1","path":"README.md"}`). `lf_read`
// returns what `lf <read>` prints, on the calling thread. `lf_subscribe` serves
// `lf <read> --watch` for a read whose command takes it. Loopflow never hands
// the app data from its own threads: it calls `wakeup` when the result may have
// changed, and the app calls `lf_pull` on its own queue, which reads there and
// returns the frame line that command would print next, or nothing. A burst of
// changes coalesces into one frame. `lf_write` takes a JSON `Write`
// (`{"write":"task_complete","directory":"/repo","issue":"FIX-1"}`)
// and makes it as `lf <write>` would, recording a Process for a Work change.
// The library opens only the store `lf` prepared; it never creates or migrates one.
#ifndef LOOPFLOW_H
#define LOOPFLOW_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct LfSubscription LfSubscription;

// Called from any thread, carrying no data: a pull may find a frame. Keep it
// cheap; schedule the pull, don't perform it here.
typedef void (*LfWakeup)(void *context);

// Start serving `read`'s watch. Returns NULL and sets `*error` (free it with
// `lf_string_free`) when it is not a read Loopflow serves, or its command
// takes no `--watch`.
LfSubscription *lf_subscribe(const char *read, LfWakeup wakeup, void *context, char **error);

// One request line, as on the stdin of the equivalent `lf` watch.
void lf_request(LfSubscription *subscription, const uint8_t *line, size_t length);

// The newest changed frame line, or NULL when nothing changed. Any read
// snapshot is released before it returns. Free the frame with
// `lf_frame_free(frame, *length)`. A failed watch returns its error as a
// last frame with `unavailable` set.
uint8_t *lf_pull(LfSubscription *subscription, size_t *length);

void lf_frame_free(uint8_t *frame, size_t length);

// True once a failed watch's last frame was pulled, as its `lf` command would
// have exited: nothing follows. Unsubscribe it.
bool lf_ended(LfSubscription *subscription);

// Read `read` on this thread. Returns what its `lf` command prints, or NULL
// with `*error` set. Free either with `lf_string_free`.
char *lf_read(const char *read, char **error);

// Make `write` on this thread. Returns what its `lf` command prints, or NULL
// with `*error` set. Free either with `lf_string_free`. Never call it on the
// app's main thread: a write may wait on the store or the network.
char *lf_write(const char *write, char **error);

// End the watch; `wakeup` is not called after this returns.
void lf_unsubscribe(LfSubscription *subscription);

void lf_string_free(char *string);

#endif
