# Decisions — 2026-11-03

No earlier design was supplied. Jack Heart's expanded steer is the acceptance
contract. Reversible choice: main-checkout Session promotion binds only the
conversation; it does not allocate the shared checkout. Continuing that bound
Session retains cwd until an explicit Task checkout exists. Repository/Wave
primary conversations require a separate ordinary Session, since their scope
cannot silently be reassigned.
