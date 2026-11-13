- LOO-872: `empty_writes` returns `Vec<String>`; `assert_no_empty_writes`
  prints "took the write lock and changed nothing:" with one site per line,
  matching the `growing_scans` assert style. The doc example uses it.
