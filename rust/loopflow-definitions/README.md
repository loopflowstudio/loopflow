# Definition content

```bash
cargo test -p loopflow-definitions --lib builtins::tests::
```

Builtin skill/Graph text, catalogue generation, portable names and frontmatter
text parsing live here. The application owns discovery from external files,
settings resolution, captured inputs and Graph execution. Those operations
retain their shared observation and request attribution.

The build script watches only builtin content. Generated maps embed text values,
so their source is identical across checkouts. The generated maps carry no
absolute include path, Git revision, native archive or executable source identity.
