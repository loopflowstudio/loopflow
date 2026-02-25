FROM rust:1.88-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY rust ./rust
RUN cargo build -p loopflow --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl libssl3 git docker.io python3 python3-pip \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/lf /usr/local/bin/lf
COPY --from=builder /build/target/release/lfd /usr/local/bin/lfd

COPY pyproject.toml README.md /build/
COPY python /build/python
RUN SETUPTOOLS_SCM_PRETEND_VERSION=0.0.0 pip3 install --break-system-packages /build \
    && rm -rf /build

RUN mkdir -p /root/.lf
WORKDIR /root

EXPOSE 2486
ENTRYPOINT ["lfd"]
CMD ["run"]
