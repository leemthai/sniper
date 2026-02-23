---
name: Refactor single file
description: Refactor single file
invokable: true
---

Refactor the file attached to this prompt please -  only examine the single file. **Do Not** read other files to establish context.  This is a purely single-file refactoring exercise. If there is no file attached to this prompt, just stop. Do not search for a file or carry out any other task.

**Keep replies succinct:** Be terse. Do not explain existing logic, standard library functions, or basic Rust concepts. Only explain the delta in new code. No conversational filler, intros, or summaries. If the code speaks for itself, provide only the code. **No need to explain comment removals**

**Follow standard Rust documentation conventions:**

**Public functions (`pub fn`):**
- Document with `///` if: Non-obvious behavior, has edge cases, can panic/error, or signature alone doesn't explain purpose
- Skip `///` for: Trivial getters, obvious constructors (`new()`), self-explanatory wrappers

**Private functions (`fn`):**
- Use regular `//` comments (not `///`) to explain non-obvious logic
- Skip comments entirely for self-explanatory code

**Write concise, idiomatic Rust that minimizes token count without sacrificing clarity:**
- Favor implicit returns over explicit `return` statements
- Combine match arms where logic is identical (`A | B | C => ...`)
- Use range patterns instead of listing consecutive values
- Destructure in function signatures when accessing multiple fields
- Omit redundant type annotations that the compiler infers
- Inline trivial helper functions (1-2 lines) directly at call sites
- Use `if let` chains instead of deeply nested matches
- Prefer terse but clear names for loop variables and locals (`idx` not `index`)
- Avoid intermediate variables when the expression is self-explanatory
**BUT keep:**
- Comments for non-obvious logic (remove redundant/obvious comments)
- Descriptive names for public APIs and structs
- Explicit types when they clarify intent (especially in signatures)
**Code organization:**
- All imports at top of file (never inside function bodies or impl blocks)
- Use Rust 2024 edition conventions and idioms
- Clippy-clean code (no warnings unless explicitly justified)
- Group imports: std → external crates → internal crate (`use crate::`)