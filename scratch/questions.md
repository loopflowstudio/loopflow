# LOO-389 assumptions — October 6, 2026

- The kickoff chooses DigitalOcean and a CLI-first experience as reversible
  implementation decisions. No evidence establishes that Jack has a usable
  DigitalOcean account. Live acceptance needs an explicitly authorized account,
  Doppler credential reference and billing period (or authorized export); no
  account connection, real rotation or resource change is authorized here.
- Local fixtures can prove the implementation but cannot satisfy the authorized
  real-period import criterion. That criterion remains open in the design.
- `lf commit` failed with exit 1 and only `Error:` while preserving the supplied
  Spend files. Originals were copied outside the checkout before memory edits;
  no raw git write was substituted. The design and memory remain local edits.
