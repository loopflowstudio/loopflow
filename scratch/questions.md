- LOO-872: `assert_no_empty_writes(store, run)` returns `run`'s result and
  prints "took the write lock and changed nothing:" with one site per line,
  matching the `growing_scans` assert style. The doc example uses it.
