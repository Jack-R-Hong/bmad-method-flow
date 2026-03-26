## Quality Rules

- All existing tests must pass before marking work complete
- Follow the project's established patterns and conventions
- Report blockers immediately rather than working around them
- Respect step dependencies — never skip prerequisite steps
- Use structured output matching the expected step result format
- No unwrap() or expect() in non-test code — map errors to appropriate types
- cargo clippy -- -D warnings and cargo fmt --check must pass
