# Open questions

## LOO-837: published parents (2026-11-13)

The earlier headless assumption that a published parent must already exist on
origin is superseded by the iteration direction: “Implement publication for
that case while respecting the parent’s commit-fence behavior”.

Stacking restores a missing remote branch through the existing commit fence.
A changed head revokes Auto remotely before clearing the merge request and
pushing; the same clean head retains the request. An existing published remote
branch stays untouched; the child reports local commits left out.
