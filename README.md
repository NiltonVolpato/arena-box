# ArenaBox

A smart pointer that holds a struct with arena allocated objects and the arena in the same struct.

This is useful for creating self-referential structs.

## Simple Example

```rust
use arena_box::*;

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

## Using ArenaBox as an Error

`ArenaBox` is movable, which makes it easy to use as an error type where you can annotate it with additional context as it propagates up the call stack.

```rust
use arena_box::*;
use core::fmt;

#[derive(Debug, PartialEq)]
struct MyError<'arena> {
    message: &'arena str,
    details: &'arena str,
}

impl fmt::Display for MyError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error: {}\nDetails: {}", self.message, self.details)
    }
}

make_arena_version!(pub MyError, ArenaMyError);

fn return_err() -> Result<(), ArenaMyError> {
    Err(ArenaMyError::new(|arena| MyError {
        message: arena.alloc_str("an error happened"),
        details: "",
    }))
}

let result = return_err().map_err(|mut e| {
    // Handle the error here
    let mut handle = e.mutate();
    handle.details = handle.arena().alloc_str("while running this test case");
    e
});

let Err(e) = result else {
    panic!("Expected an error");
};

assert_eq!(
    *e.get(),
    MyError {
        message: "an error happened",
        details: "while running this test case",
    }
);

assert_eq!("an error happened", format!("{}", e));

assert_eq!(
    r#"MyError { message: "an error happened", details: "while running this test case" }"#,
    format!("{:?}", e)
);
```
