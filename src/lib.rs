#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use bumpalo::Bump;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr::NonNull;

pub trait WithLifetime {
    type With<'a>;
}

#[macro_export]
macro_rules! make_arena_version {
    ($vis:vis $name:ident, $alias:ident) => {
        $vis type $alias = ArenaBox<$name<'static>>;

        impl WithLifetime for $name<'static> {
            type With<'a> = $name<'a>;
        }
    };
}

pub struct MutHandle<'b, T: WithLifetime> {
    data: &'b mut <T as WithLifetime>::With<'b>,
    arena: &'b Bump,
}

impl<'b, T: WithLifetime> MutHandle<'b, T> {
    pub fn arena(&self) -> &'b Bump {
        self.arena
    }
}

impl<'b, T: WithLifetime> Deref for MutHandle<'b, T> {
    type Target = <T as WithLifetime>::With<'b>;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'b, T: WithLifetime> DerefMut for MutHandle<'b, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

pub struct ArenaBox<T: WithLifetime> {
    arena: Pin<Box<Bump>>,
    data: NonNull<T>,
}

impl<T: WithLifetime> ArenaBox<T> {
    pub fn new<F>(build: F) -> Self
    where
        F: for<'a> FnOnce(&'a Bump) -> <T as WithLifetime>::With<'a>,
    {
        let arena = Box::pin(Bump::new());
        let arena_ref: &Bump = arena.as_ref().get_ref();
        let data_ref = arena_ref.alloc(build(arena_ref));
        let data = unsafe {
            NonNull::new_unchecked(data_ref as *mut <T as WithLifetime>::With<'_> as *mut T)
        };
        ArenaBox { arena, data }
    }

    /// Get a reference to the data within the arena.
    ///
    /// # Safety
    ///
    /// The returned reference is valid for the lifetime of the `ArenaBox`.
    /// # Negative compilation test
    ///
    /// The following code should fail to compile because the lifetime of the
    /// reference to the `Data` struct is tied to the `ArenaData` instance, and
    /// cannot be extended beyond the lifetime of the `ArenaData` instance.
    ///
    /// ```compile_fail
    /// # use arena_box::*;
    /// # pub struct Data<'a> {
    /// #     msg: &'a str,
    /// # }
    /// # make_arena_version!(Data, ArenaData);
    /// let message: &str;
    /// {
    ///     let boxed = ArenaData::new(|arena| Data {
    ///         msg: arena.alloc_str("Something"),
    ///     });
    ///     message = boxed.get().msg; // Should fail: cannot extend lifetime
    /// }
    /// assert_eq!(message, "Something");
    /// ```
    pub fn get<'b>(&'b self) -> &'b <T as WithLifetime>::With<'b> {
        unsafe { &*(self.data.as_ptr() as *const <T as WithLifetime>::With<'b>) }
    }

    pub fn mutate<'b>(&'b mut self) -> MutHandle<'b, T> {
        let data = unsafe { &mut *(self.data.as_ptr() as *mut <T as WithLifetime>::With<'b>) };
        let arena = self.arena.as_ref().get_ref();
        MutHandle { data, arena }
    }
}

impl<T: core::fmt::Display + WithLifetime> core::fmt::Display for ArenaBox<T>
where
    for<'a> T::With<'a>: core::fmt::Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.get(), f)
    }
}

impl<T: core::fmt::Debug + WithLifetime> core::fmt::Debug for ArenaBox<T>
where
    for<'a> T::With<'a>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.get(), f)
    }
}

impl<T: WithLifetime> PartialEq for ArenaBox<T>
where
    for<'a> T::With<'a>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data || self.get() == other.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Data<'a> {
        msg: &'a str,
    }
    make_arena_version!(Data, ArenaData);

    #[test]
    fn test_build() {
        let boxed = ArenaData::new(|arena| Data {
            msg: arena.alloc_str("Something"),
        });
        assert_eq!(boxed.get().msg, "Something");
    }

    #[test]
    fn test_update() {
        let mut boxed = ArenaData::new(|arena| Data {
            msg: arena.alloc_str("Something"),
        });
        assert_eq!(boxed.get().msg, "Something");

        {
            let mut handle = boxed.mutate();
            handle.msg = handle.arena().alloc_str("Something different"); // DerefMut
        }

        let message = boxed.get().msg;
        assert_eq!(message, "Something different");
    }

    #[test]
    fn test_update_twice() {
        let mut boxed = ArenaData::new(|arena| Data {
            msg: arena.alloc_str("Something"),
        });
        assert_eq!(boxed.get().msg, "Something");

        {
            let mut handle = boxed.mutate();
            handle.msg = handle.arena().alloc_str("Something different"); // DerefMut
        }

        assert_eq!(boxed.get().msg, "Something different");

        {
            let mut handle = boxed.mutate();
            handle.msg = handle.arena().alloc_str("Something else"); // DerefMut
        }
        assert_eq!(boxed.get().msg, "Something else");
    }

    fn do_something(boxed: ArenaData) {
        // ArenaBox moved here
        assert_eq!(boxed.get().msg, "Foo");
    }

    #[test]
    fn test_move() {
        let boxed = ArenaData::new(|arena| Data {
            msg: arena.alloc_str("Foo"),
        });
        let r = boxed.get();
        assert_eq!(r.msg, "Foo");
        do_something(boxed);
    }

    #[derive(Debug, PartialEq)]
    struct MyError<'arena> {
        message: &'arena str,
        details: &'arena str,
    }
    make_arena_version!(MyError, ArenaMyError);
    impl core::fmt::Display for MyError<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    #[test]
    fn test_as_an_error() {
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
        assert_eq!("an error happened", alloc::format!("{}", e));
        assert_eq!(
            r#"MyError { message: "an error happened", details: "while running this test case" }"#,
            alloc::format!("{:?}", e)
        );
    }

    #[test]
    fn test_equality() {
        let a = ArenaMyError::new(|arena| MyError {
            message: arena.alloc_str("an error happened"),
            details: arena.alloc_str("while running this test case"),
        });
        let mut b = ArenaMyError::new(|arena| MyError {
            message: arena.alloc_str("an error happened"),
            details: arena.alloc_str("while running this test case"),
        });
        assert_eq!(a, b);
        {
            let mut handle = b.mutate();
            handle.details = "";
        }
        assert_ne!(a, b);
    }
}
