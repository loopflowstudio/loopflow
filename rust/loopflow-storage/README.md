# Synchronous storage

`loopflow-storage` owns saved records, SQLite operations, migrations and files
beside the database. Application acquisition supplies configuration, process
observations, caller identity, build provenance and lazy credential keys.
Operations compare those saved inputs and commit their own transactions.

An application-only edit reuses this library. A saved Task record or SQL edit
rebuilds storage and its application callers; model and terminal remain reusable.

The migration catalogue, canonical schema and empty test database image are
built here together. [Migration rules](src/MIGRATIONS.md) describe ordering and
upgrade behavior. First additions in both historical and current source paths
contribute to the same order; moving a file cannot change its migration identity.
The application receives unmerged slugs as Cargo build metadata and owns its
runtime build identity.

Compile storage fixtures independently before compiling application fixtures:

```bash
cargo test -p loopflow-storage --lib --no-run
cargo test -p loopflow --lib --no-run
```

The application uses `test-support` only as a dev dependency. It exposes prepared
fixtures and existing inspection helpers, including raw fixture connections;
ordinary builds expose saved operations. Storage tests have no application
dependency. Application lifecycle and capture-manifest projection tests stay
with their application owners.
