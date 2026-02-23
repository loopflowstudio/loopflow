FROM node:22-bookworm-slim

COPY install-loopflow.sh /usr/local/bin/install-loopflow.sh
RUN chmod +x /usr/local/bin/install-loopflow.sh \
    && /usr/local/bin/install-loopflow.sh --install
RUN groupadd --system agent \
    && useradd --system --gid agent --create-home agent \
    && mkdir -p /workspace \
    && chown -R agent:agent /workspace /home/agent

WORKDIR /workspace
USER agent
