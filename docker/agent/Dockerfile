FROM node:22-bookworm-slim

COPY install-loopflow.sh /usr/local/bin/install-loopflow.sh
RUN chmod +x /usr/local/bin/install-loopflow.sh \
    && /usr/local/bin/install-loopflow.sh --install

WORKDIR /workspace
