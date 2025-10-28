# ArenaBox

A smart pointer that holds a struct with arena allocated objects and the arena in the same struct.

This is useful for creating self-referential structs.

## Simple Example

```rust
use arena_box::{ArenaBox, make_arena_version, WithLifetime};

// Data contains a message allocated in an arena.
pub struct Data<'arena> {
    msg: &'arena str,
}

// Creates a shorthand for the smart pointer type.
make_arena_version!(pub Data, ArenaData);

// Constructs a new instance using the builder closure.
let boxed = ArenaData::new(|arena| Data {
    msg: arena.alloc_str("Something"),
});

assert_eq!(boxed.get().msg, "Something");
```

## Using ArenaBox for error handling with context

`ArenaBox` is movable, which makes it easy to use as an error type where you can annotate it with additional context as it propagates up the call stack.

```rust
use arena_box::{ArenaBox, make_arena_version, WithLifetime};
use bumpalo::{Bump, collections::Vec};
use std::fmt;

// Define an error type with arena-allocated strings
struct DetailedError<'a> {
    message: &'a str,
    context: Vec<'a, &'a str>,
}

impl<'a> DetailedError<'a> {
    fn new(arena: &'a Bump, message: &'a str) -> Self {
        DetailedError {
            message,
            context: Vec::new_in(arena),
        }
    }
}

impl<'a> fmt::Display for DetailedError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Error: {}", self.message)?;
        if !self.context.is_empty() {
            writeln!(f, "\nContext:")?;
            for (i, ctx) in self.context.iter().rev().enumerate() {
                writeln!(f, "  [{}] {}", i, ctx)?;
            }
        }
        Ok(())
    }
}

// Generate the arena version
make_arena_version!(DetailedError, ArenaError);

// Helper functions that add context as errors bubble up
fn parse_value(s: &str) -> Result<i32, ArenaError> {
    s.parse().map_err(|_| {
        ArenaError::new(|arena| {
            DetailedError::new(arena, arena.alloc_str("Invalid number format"))
        })
    })
}

fn read_config(input: &str) -> Result<i32, ArenaError> {
    parse_value(input).map_err(|mut err| {
        let mut ctx = err.mutate();
        let message = ctx.arena().alloc_str("While parsing configuration");
        ctx.context.push(message);
        err
    })
}

fn process_request() -> Result<i32, ArenaError> {
    read_config("not_a_number").map_err(|mut err| {
        let mut ctx = err.mutate();
        let message = ctx.arena().alloc_str("In function process_request()");
        ctx.context.push(message);
        err
    })
}

// Usage
match process_request() {
    Ok(value) => println!("Success: {}", value),
    Err(e) => println!("{}", e),
}
// Prints:
// Error: Invalid number format
//
// Context:
//   [0] In function process_request()
//   [1] While parsing configuration
```
