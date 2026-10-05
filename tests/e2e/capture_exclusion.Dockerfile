FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl python3 passwd util-linux \
    && rm -rf /var/lib/apt/lists/*

# Frozen published v0.13.3 checksums: this experiment must exercise the writer
# that bypasses installation admission, even after newer releases are published.
RUN set -eu; \
    case "$(uname -m)" in \
        x86_64) checksum=5c567831c734341e0e3cb4c99ae057f217555ad24dc6fa40925af4fc926dcd43 ;; \
        aarch64) checksum=086f0903d2c7e0357f4bad69e55773f522ca8b641ee339ac0768ebb75483e7ff ;; \
        *) exit 1 ;; \
    esac; \
    mkdir -p /fixture/prior; \
    curl --fail --location --retry 2 --max-time 120 \
        "https://github.com/loopflowstudio/loopflow/releases/download/v0.13.3/lf-$(uname -m)-unknown-linux-gnu.tar.gz" \
        -o /fixture/released.tar.gz; \
    printf '%s  /fixture/released.tar.gz\n' "$checksum" | sha256sum --check --strict; \
    tar -xzf /fixture/released.tar.gz -C /fixture/prior lf; \
    sha256sum /fixture/prior/lf | cut -d ' ' -f 1 > /fixture/released-cli.sha256; \
    rm /fixture/released.tar.gz

COPY capture_exclusion.py /fixture/capture_exclusion.py
CMD ["sh", "-ec", "python3 /fixture/capture_exclusion.py --released-sha256 \"$(cat /fixture/released-cli.sha256)\""]
