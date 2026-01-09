Review and run tests for this branch.

## Process

1. **Identify key behavior changes**
   - Run `git diff main...HEAD` to see what changed
   - What user-visible behaviors does this diff affect?
   - Are those behaviors well tested? Add tests if not.

2. **Review new tests**
   - Are tests written for this code worth maintaining?
   - Do they test user behavior, not implementation details?
   - Are they simple and focused? Delete any that require elaborate mocking.

3. **Review related tests**
   - Look at tests in nearby modules
   - Is there drift that would be better served by consolidating?
   - Would reorganizing make current behaviors clearer?

4. **Run tests and fix failures**
   - Run the full test suite
   - If tests fail, first determine: is it broken test code or broken real code?
   - Fix them one by one, maximizing iteration speed—run single tests while debugging

## Standards

From STYLE.md:
- Test user behavior, not implementation details
- Keep tests short and focused on one behavior
- Delete flaky tests rather than adding retries

**Mocking**: Use mocks to prevent side effects (network, subprocess, file I/O) and speed up tests. But don't write tests that just verify mock calls—if your assertion is `mock.assert_called_with(...)`, you're testing implementation, not behavior. Mock the dependency, then assert on the *result* of the function under test.
