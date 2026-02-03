# Learning Rust Through lf-core

A guide to Rust concepts using the `lf-core` crate as teaching material.

---

## 1. Project Structure

Rust uses **Cargo** for package management (like `uv` for Python). The key files:

```
Cargo.toml          # Workspace root - like a monorepo pyproject.toml
rust/lf-core/
  Cargo.toml        # Crate (package) definition
  src/
    lib.rs          # Crate entry point - declares modules
    *.rs            # Individual modules
```

In `Cargo.toml` at the root:
```toml
[workspace]
members = ["rust/lf-core"]   # Lists all crates in the workspace
```

In `rust/lf-core/src/lib.rs`:
```rust
pub mod error;      // Declares "error" module, loads from error.rs
pub mod flow;       // Each `mod X` looks for X.rs or X/mod.rs
pub mod prompt;

// Re-exports for public API
pub use error::{CoreError, LoadError};
pub use flow::{load_flow, next_action, Flow, FlowAction, Step};
```

**Python equivalent:** `lib.rs` is like `__init__.py`, but Rust requires explicit module declarations. The `pub` keyword = "public". Without it, things are private to the module.

---

## 2. Ownership and Borrowing (The Big One)

Rust's killer feature is compile-time memory safety without garbage collection. Look at `flow.rs` and `prompt.rs`:

**The rules:**
- `String` = owned string (like Python's `str`, you own the memory)
- `&str` = borrowed string slice (a view into someone else's string)
- `&` = immutable borrow (read-only reference)
- `&mut` = mutable borrow (read-write reference)

In `flow.rs:18`:
```rust
pub struct Step {
    pub name: String,                    // Step OWNS this string
    pub model: Option<String>,           // Optional owned string
    pub directions: Vec<String>,         // Owns a vector of owned strings
    pub content: Option<String>,
}
```

**Why this matters:** When `Step` is dropped (goes out of scope), all its `String`s are automatically freed. No GC, no manual `free()`.

---

## 3. Enums and Pattern Matching

Rust enums are **way** more powerful than Python enums. They can hold data. See `flow.rs:30`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum FlowItem {
    Step(Step),                          // Holds a Step struct
    Fork {                               // Holds named fields
        branches: Vec<FlowItem>,
        synthesize: Option<String>,
    },
    Choose {
        prompt: String,
        options: HashMap<String, Vec<FlowItem>>,
    },
}
```

**Python equivalent:** This is like a tagged union. In Python you'd use a base class with subclasses, or `@dataclass` with a `type` field. Rust makes this first-class.

Pattern matching with `match` (like Python's `match` but exhaustive):

```rust
let action = match next_action(&flow, 0) {
    FlowAction::RunStep { step } => step,
    FlowAction::WaitInteractive { step } => step,
    FlowAction::Complete => return,
    _ => return,
};
```

The compiler **forces** you to handle all variants. Can't forget a case.

---

## 4. Result and Option (No Exceptions)

Rust doesn't have exceptions. Instead, functions return `Result<T, E>` or `Option<T>`.

```rust
// From flow.rs:56
pub fn load_flow(name: &str, repo: &Path) -> Result<Flow, LoadError> {
    let flow_path = find_flow_path(name, repo)?;   // ? = early return on error
    let content = fs::read_to_string(&flow_path)?; // ? propagates errors
    // ...
    Ok(Flow { name: name.to_string(), items })     // Success case
}
```

**The `?` operator:** If the result is `Err`, return early with that error. If `Ok`, unwrap the value. It's like:
```python
# Python equivalent
result = find_flow_path(name, repo)
if isinstance(result, Err):
    return result
flow_path = result.unwrap()
```

`Option<T>` is for nullable values:
```rust
pub model: Option<String>,  // Either Some(string) or None
```

From `flow.rs:169`:
```rust
let model = map
    .get(&Value::String("model".to_string()))  // Returns Option
    .and_then(|val| val.as_str())              // Chain if Some
    .map(|val| val.to_string());               // Transform if Some
```

---

## 5. Derive Macros

Those `#[derive(...)]` annotations auto-generate code:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Step {
```

- `Debug` = can print with `{:?}` (like Python's `__repr__`)
- `Clone` = can call `.clone()` to copy
- `Serialize/Deserialize` = serde JSON/YAML support
- `PartialEq, Eq` = can use `==`

**Python equivalent:** Like `@dataclass` auto-generating `__eq__`, `__repr__`, etc.

---

## 7. Error Handling with thiserror

In `error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("flow not found: {0}")]        // {0} = first field
    FlowNotFound(String),
    #[error("step not found: {0}")]
    StepNotFound(String),
    #[error("io error: {0}")]
    Io(String),
}
```

`thiserror` generates the `std::error::Error` impl and `Display` formatting. The `#[error("...")]` becomes the error message.

The `From` impls enable automatic conversion:
```rust
impl From<std::io::Error> for LoadError {
    fn from(err: std::io::Error) -> Self {
        LoadError::Io(err.to_string())
    }
}
```

Now `?` can convert `io::Error` -> `LoadError` automatically.

---

## 8. Key Technical Decisions

| Decision | Why (Rust-specific reason) |
|----------|---------------------------|
| `FlowItem` enum | Tagged unions are idiomatic. Compiler enforces handling all cases. |
| `&str` in function args | Avoids unnecessary allocations. Accept borrowed data when you don't need ownership. |
| `thiserror` for errors | Idiomatic error handling. Integrates with `?` operator. |
| `Option<String>` not `String` | No null pointers. Compiler forces you to handle missing values. |
| Workspace with single crate | Room to grow (could add `lf-cli` crate later). Shared dependencies. |

---

## 9. Idiomatic Patterns in This Code

**Builder-style chaining** (`flow.rs:169`):
```rust
let model = map
    .get(&key)
    .and_then(|val| val.as_str())
    .map(|s| s.to_string());
```

**Early returns with `?`** - keeps the happy path unindented.

**Explicit `pub` visibility** - default is private, you opt into public.

**`impl From<X> for Y`** - enables automatic conversions with `?`.

---

## 10. Common Gotchas for Python Developers

### Strings are complicated
```rust
let s: String = "hello".to_string();  // Owned, heap-allocated
let s: &str = "hello";                 // Borrowed, usually static
let s: &String = &owned_string;        // Borrow of owned string
```

### No implicit returns from blocks with semicolons
```rust
fn foo() -> i32 {
    42      // Returns 42 (no semicolon)
}

fn bar() -> i32 {
    42;     // ERROR: returns () because semicolon makes it a statement
}
```

### Clone isn't free
```rust
let copy = original.clone();  // Deep copy - allocates new memory
```

In Python, assignment is reference. In Rust, it's a move (or copy for simple types).

### Mutability is explicit
```rust
let x = 5;        // Immutable
let mut y = 5;    // Mutable
y += 1;           // OK
x += 1;           // ERROR
```
