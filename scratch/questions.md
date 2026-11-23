# Choices made for review

- Use Modal’s published minimum of 0.125 core and 128 MiB. The field brief also suggested 256–512 MiB; the request can be raised later with a measured workload.
- Resolve packaged Linux builds on the caller, where authenticated dev artifacts are available. Baking a moving stream into the shared image would require rebuilding per release.
- A named Machine opens an exact remote-only Session without searching every Machine. Mobile reaches that owner through the already paired Machine.
- Make overwriting forwarded files deliberate. A targeted Session restart is opt-in so rotating one login does not interrupt unrelated work.
