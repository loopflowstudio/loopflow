# Open questions

- The design doc's suggested verification command (`grep -r 'solo\|team\b\|ci\b' docs/ deploy/ docker/`) overmatches unrelated CI references in general docs and even binary screenshot assets. I validated the deployment-facing files directly (`docs/lfd.md`, `docs/getting-started.md`, `deploy/README.md`, `docker/docker-compose.yml`, `deploy/docker-compose.prod.yml`) instead.
