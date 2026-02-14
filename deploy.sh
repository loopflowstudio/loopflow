#!/bin/bash
# Deploy lfd stack to a remote host.
#
# Usage: ./deploy.sh [host]
#
# Expects a git clone of this repo on the remote host at ~/loopflow.
# Pulls the latest code, copies .env, and restarts the stack.
#
# First-time setup on the instance:
#   git clone https://github.com/loopflowstudio/loopflow.git ~/loopflow
#   cp .env ~/loopflow/.env  # (with LFD_AUTH_TOKEN set)
#
# Prerequisites:
#   - SSH access configured (~/.ssh/config entry for the host)
#   - .env file on the instance with LFD_AUTH_TOKEN set

set -euo pipefail

HOST="${1:-lfd-dev}"

echo "Deploying to $HOST..."

# Pull latest and restart
ssh "$HOST" 'cd ~/loopflow && git pull'
scp .env "$HOST":~/loopflow/.env
ssh "$HOST" 'cd ~/loopflow && docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d --build'
ssh "$HOST" 'cd ~/loopflow && docker compose -f docker-compose.yml -f docker-compose.prod.yml ps'
