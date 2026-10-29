// Loopflow's reads inside an app, in Ghostty's embedding shape. Each
// subscription serves one `lf <read> --json --watch`: `operation` is a JSON
// `Operation` (`{"operation":"desktop_plan"}`). Loopflow never hands the app
// data from its own threads: it calls `wakeup` when the result may have
// changed, and the app calls `lf_pull` on its own queue, which reads there and
// returns the frame line that command would print next, or nothing. A burst
// of changes coalesces into one frame.
#ifndef LOOPFLOW_H
#define LOOPFLOW_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct LfSubscription LfSubscription;

// Called from any thread, carrying no data: a pull may find a frame. Keep it
// cheap; schedule the pull, don't perform it here.
typedef void (*LfWakeup)(void *context);

// Start serving `operation`. Returns NULL and sets `*error` (free it with
// `lf_string_free`) when the operation is not one Loopflow serves.
LfSubscription *lf_subscribe(const char *operation, LfWakeup wakeup, void *context, char **error);

// One request line, as on the stdin of the equivalent `lf` watch.
void lf_request(LfSubscription *subscription, const uint8_t *line, size_t length);

// The newest changed frame line, or NULL when nothing changed. Any read
// snapshot is released before it returns. Free the frame with
// `lf_frame_free(frame, *length)`. A failed watch returns its error as a
// last frame with `unavailable` set.
uint8_t *lf_pull(LfSubscription *subscription, size_t *length);

void lf_frame_free(uint8_t *frame, size_t length);

// End the watch; `wakeup` is not called after this returns.
void lf_unsubscribe(LfSubscription *subscription);

void lf_string_free(char *string);

#endif
