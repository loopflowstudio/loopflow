FROM rust:1.88-bookworm AS builder
WORKDIR /build

# --- Dependency layer (cached unless Cargo.lock changes) ---
COPY Cargo.toml Cargo.lock ./
COPY rust/loopflow/Cargo.toml rust/loopflow/Cargo.toml
COPY rust/loopflow-test-support/Cargo.toml rust/loopflow-test-support/Cargo.toml
RUN mkdir -p rust/loopflow/src/bin rust/loopflow-test-support/src \
    && touch rust/loopflow/src/lib.rs \
    && echo "fn main() {}" > rust/loopflow/src/bin/lf.rs \
    && echo "fn main() {}" > rust/loopflow/src/bin/lfd.rs \
    && echo "fn main() {}" > rust/loopflow/src/bin/lf-prompt.rs \
    && touch rust/loopflow-test-support/src/lib.rs
RUN cargo build -p loopflow --release

# --- Source layer (only your code recompiles) ---
COPY rust ./rust
RUN cargo build -p loopflow --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl libssl3 git docker.io \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/lf /usr/local/bin/lf
COPY --from=builder /build/target/release/lfd /usr/local/bin/lfd

RUN addgroup --system lfd && adduser --system --ingroup lfd lfd
RUN mkdir -p /app/.lf && chown -R lfd:lfd /app
WORKDIR /app
USER lfd

EXPOSE 2486
ENTRYPOINT ["lfd"]
CMD ["run"]
