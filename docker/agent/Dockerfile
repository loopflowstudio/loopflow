FROM node:22-bookworm-slim

# Core tools — just enough to run agents and work with repos
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    curl \
    ca-certificates \
    openssh-client \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Coding agents
RUN npm install -g \
    @anthropic-ai/claude-code \
    @openai/codex \
    @google/gemini-cli \
    && npm cache clean --force

# OpenCode
RUN ARCH=$(dpkg --print-architecture) \
    && case "$ARCH" in \
        amd64) OC_ARCH="x86_64" ;; \
        arm64) OC_ARCH="aarch64" ;; \
        *) echo "unsupported arch: $ARCH" && exit 1 ;; \
    esac \
    && curl -fsSL "https://github.com/anomalyco/opencode/releases/latest/download/opencode-linux-${OC_ARCH}.tar.gz" \
       | tar -xz -C /usr/local/bin opencode \
    && chmod +x /usr/local/bin/opencode

# Git defaults
RUN git config --system init.defaultBranch main \
    && git config --system safe.directory /workspace

WORKDIR /workspace
