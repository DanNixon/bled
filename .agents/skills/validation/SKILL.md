---
name: validation
description: Validate changes to the codebase.
---

# Validation of Changes

Use this skill to validate changes to the codebase.

From the root of the repository, run:

```sh
nix develop -c just check
nix develop -c just build
```

This will run all the checks, linting, tests and builds each component.
