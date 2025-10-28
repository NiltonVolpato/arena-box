#![no_std]
#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::boxed::Box;
use bumpalo::Bump;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr::NonNull;

/// A trait for types that have a lifetime parameter.
pub trait WithLifetime {
    /// The type with a lifetime parameter.
    type With<'a>;
}

/// A macro to create a convenient alias for the smart pointer and also
/// implement the required traits.
///
/// You can control the visibility of the alias, so you can make your
/// struct public as part of your API, while keeping the arena implementation
/// private.
///
/// ```ignore
/// // Generated alias can have different visibility
/// make_arena_version!(Data, pub ArenaData);         // public
/// make_arena_version!(Data, pub(crate) ArenaData);  // crate-only
/// make_arena_version!(Data, ArenaData);             // private
/// ```
///
/// # Example
///
/// ```
/// # use arena_box::*;
///
/// pub struct Data<'a> {
///    msg: &'a str,
/// }
///
/// make_arena_version!(Data, pub ArenaData);
/// ```
#[macro_export]
macro_rules! make_arena_version {
    ($name:ident, $vis:vis $alias:ident) => {
        $vis type $alias = ArenaBox<$name<'static>>;

        impl WithLifetime for $name<'static> {
            type With<'a> = $name<'a>;
        }
    };
}

/// A handle for mutating the data in an `ArenaBox`.
///
/// This struct is created by the [`ArenaBox::mutate`] method.
pub struct MutHandle<'b, T: WithLifetime> {
    data: &'b mut <T as WithLifetime>::With<'b>,
    arena: &'b Bump,
}

impl<'b, T: WithLifetime> MutHandle<'b, T> {
    /// Returns a reference to the arena.
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

/// A smart pointer that holds a struct with arena allocated objects and the arena in the same struct.
///
/// This is useful for creating self-referential structs.
///
/// # Example
///
/// ```
/// # use arena_box::*;
///
/// pub struct Data<'a> {
///    msg: &'a str,
/// }
///
/// make_arena_version!(Data, pub ArenaData);
///
/// let boxed = ArenaData::new(|arena| Data {
///     msg: arena.alloc_str("Something"),
/// });
///
/// assert_eq!(boxed.get().msg, "Something");
/// ```
pub struct ArenaBox<T: WithLifetime> {
    arena: Pin<Box<Bump>>,
    data: NonNull<T>,
}

impl<T: WithLifetime> ArenaBox<T> {
    /// Creates a new `ArenaBox`.
    ///
    /// # Example
    ///
    /// ```
    /// # use arena_box::*;
    ///
    /// pub struct Data<'a> {
    ///    msg: &'a str,
    /// }
    ///
    /// make_arena_version!(Data, pub ArenaData);
    ///
    /// let boxed = ArenaData::new(|arena| Data {
    ///     msg: arena.alloc_str("Something"),
    /// });
    ///
    /// assert_eq!(boxed.get().msg, "Something");
    /// ```
    pub fn new<F>(build: F) -> Self
    where
        F: for<'a> FnOnce(&'a Bump) -> <T as WithLifetime>::With<'a>,
    {
        let arena = Box::pin(Bump::new());
        let arena_ref: &Bump = arena.as_ref().get_ref();
        let data_ref = arena_ref.alloc(build(arena_ref));
        let data = unsafe {
            // SAFETY: The arena is pinned, so the pointer to the data will be valid for the lifetime of the `ArenaBox`.
            NonNull::new_unchecked(data_ref as *mut <T as WithLifetime>::With<'_> as *mut T)
        };
        ArenaBox { arena, data }
    }

    /// Creates a new `ArenaBox` by transforming data from another `ArenaBox`, reusing its arena.
    ///
    /// This allows you to build up data structures incrementally, where new types can reference
    /// data from the original type. The source `ArenaBox` is consumed and its arena is moved
    /// into the new `ArenaBox`.
    ///
    /// # Example
    ///
    /// ```
    /// # use arena_box::*;
    ///
    /// #[derive(Debug, PartialEq)]
    /// pub struct Data<'a> {
    ///     msg: &'a str,
    /// }
    ///
    /// #[derive(Debug, PartialEq)]
    /// pub struct AugmentedData<'a> {
    ///     original: &'a Data<'a>,
    ///     extra: &'a str,
    /// }
    ///
    /// make_arena_version!(Data, pub ArenaData);
    /// make_arena_version!(AugmentedData, pub ArenaAugmentedData);
    ///
    /// let data = ArenaData::new(|arena| Data {
    ///     msg: arena.alloc_str("hello"),
    /// });
    ///
    /// // Transform Data into AugmentedData, reusing the arena
    /// let augmented = ArenaAugmentedData::new_from(data, |arena, original| {
    ///     AugmentedData {
    ///         original, // Reference to the original Data!
    ///         extra: arena.alloc_str("extra info"),
    ///     }
    /// });
    ///
    /// assert_eq!(augmented.get().original.msg, "hello");
    /// assert_eq!(augmented.get().extra, "extra info");
    /// ```
    pub fn new_from<U: WithLifetime, F>(source: ArenaBox<U>, build: F) -> Self
    where
        F: for<'a> FnOnce(
            &'a Bump,
            &'a <U as WithLifetime>::With<'a>,
        ) -> <T as WithLifetime>::With<'a>,
    {
        let ArenaBox { arena, data } = source;
        let arena_ref = arena.as_ref().get_ref();

        let source_data = unsafe { &*(data.as_ptr() as *const <U as WithLifetime>::With<'_>) };

        let new_data_ref = arena_ref.alloc(build(arena_ref, source_data));
        let new_data = unsafe {
            NonNull::new_unchecked(new_data_ref as *mut <T as WithLifetime>::With<'_> as *mut T)
        };

        ArenaBox {
            arena,
            data: new_data,
        }
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
        // SAFETY: The data is guaranteed to be valid for the lifetime of the `ArenaBox`.
        unsafe { &*(self.data.as_ptr() as *const <T as WithLifetime>::With<'b>) }
    }

    /// Mutates the data in the `ArenaBox`.
    ///
    /// This method returns a [`MutHandle`] that can be used to mutate the data.
    ///
    /// # Example
    ///
    /// ```
    /// # use arena_box::*;
    ///
    /// pub struct Data<'a> {
    ///   msg: &'a str,
    /// }
    ///
    /// make_arena_version!(Data, pub ArenaData);
    ///
    /// let mut boxed = ArenaData::new(|arena| Data {
    ///    msg: arena.alloc_str("Something"),
    /// });
    ///
    /// assert_eq!(boxed.get().msg, "Something");
    ///
    /// {
    ///    let mut handle = boxed.mutate();
    ///    handle.msg = handle.arena().alloc_str("Something different");
    /// }
    ///
    /// assert_eq!(boxed.get().msg, "Something different");
    /// ```
    pub fn mutate<'b>(&'b mut self) -> MutHandle<'b, T> {
        // SAFETY: The data is guaranteed to be valid for the lifetime of the `ArenaBox`.
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

    #[derive(Debug, PartialEq)]
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
    struct AugmentedData<'arena> {
        data: &'arena Data<'arena>,
        extra: &'arena str,
    }

    make_arena_version!(AugmentedData, ArenaAugmentedData);

    #[test]
    fn test_new_from() {
        let a = ArenaData::new(|arena| Data {
            msg: arena.alloc_str("hello"),
        });

        let b = ArenaAugmentedData::new_from(a, |arena, data| AugmentedData {
            data, // Reference to the original!
            extra: arena.alloc_str("extra info"),
        });

        assert_eq!(
            *b.get(),
            AugmentedData {
                data: &Data { msg: "hello" },
                extra: "extra info",
            }
        );
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
