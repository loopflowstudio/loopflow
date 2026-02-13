# Open questions / follow-ups

- Image pipeline currently shells out to `docker build` CLI instead of using the Docker API directly. This assumes the Docker CLI binary is installed alongside a reachable daemon.
