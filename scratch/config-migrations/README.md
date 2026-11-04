# Context configuration migration

Jack Heart requested personal and repository config updates, then reviewed the
viewer and accepted context.*. Nine main repositories under /Users/jack/src,
their private/Wave configs and ~/.lf/config.yaml were inspected for these keys.
Only Loopflow and Hootro had context-family config declarations, both root exclude lists.
Loopflow also has docs declarations in its gate and implement skill frontmatter;
its patch includes both. The other repository definitions and personal LF skills
had no matching declarations.
The personal file needs no context migration. These diffs preserve other fields
and existing local edits. Hootro has an unrelated legacy branch_names setting;
this patch does not remove it or claim full-file validity under the new parser.

The installed LF rejects the new mapping with “invalid type: map, expected a
sequence” in a disposable Git repository/Home. Apply these patches only after a
release supporting the namespace is installed; no installed binary was replaced.
Each diff was parsed to prove that only context-family declarations moved:
exclude to context.exclude, and skill docs to context.docs.
Task 807ef7 owns removal of the temporary released-file reader after rollout.
