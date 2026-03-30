# Open questions

- Gstack browser/bin asset references now point at `.lf/steps/gstack/...` for consistency with the new namespaced layout, but the actual browser daemon binaries and helper scripts are still not imported in this milestone. Browser-oriented steps (`browse`, `qa`, `connect-chrome`, etc.) will still need separate asset packaging or clearer runtime errors.
