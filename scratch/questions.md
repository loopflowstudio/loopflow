# Questions

- Should prose after `skill:` keep words such as `--repo` literally?
  `lf : fix the --repo flag` still reads `flag` as the repository. A trailing
  `--repo` now reports the missing value.
- The rewrite gives Clap the shortest command line that reaches the missing
  flag. When there are several errors, this can change which one Clap reports.
  For example, `lf --bogus open --repo` reports the missing repository value
  first. The Task leaves that choice open to review.
- Should a subcommand name written directly after a value-taking flag win
  over the value? `lf task --wave create` reads the Wave as `create` and
  prints `lf task` usage. Kept Clap's reading.
