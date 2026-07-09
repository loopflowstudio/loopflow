# Non-blocking GUI process environment

## Intent

Show the Mac window without waiting for interactive shell startup while keeping
one guarantee: every child process that needs user-installed tools launches with
the resolved shell PATH.

## Shape

- `GUIProcessEnvironment` becomes the single asynchronous resolver.
- App bootstrap installs the fixed fallback PATH synchronously, starts shell
  discovery in the background, and updates the inherited process PATH when it
  resolves.
- One cached task performs shell discovery. Concurrent lfd, `lf`, tmux, and wave
  launch requests await the same result.
- Child launchers pass the resolved environment explicitly. Global `setenv` is
  retained only for subprocesses owned by embedded/external terminal plumbing.
- Shell failure or the three-second deadline resolves to the existing fixed
  candidates; it never fails app startup.

## Done when

- `LoopflowApp.init()` performs no interactive-shell wait.
- Bundled lfd, registry reads, tmux sessions, and wave launches await the shared
  environment before spawning.
- Tests prove preparation returns while shell discovery is still blocked,
  concurrent callers share one discovery, fallback behavior stays intact, and
  the real login-shell PATH reaches child processes.
- The focused Swift tests and full Swift package suite pass.

