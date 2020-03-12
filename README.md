# Composite Error

This crates allows you to define errors that are convenient to write, convenient to use, flexible and extensible. To make this work, a compositing approach was followed to be able to define complex errors on the base of primitive ones.

## Motivation

Let's look at the error handling approach implemented in `std::io`. There is one error type for the whole module: `std::io::Error` which has a `kind()` method that returns a `std::io::ErrorKind`. Everywhere where an error can occur in `std::io`, a `Result<T, std::io::Error>` is returned. This makes it easy to find out if an error occurred. `kind()` can then be used to determine which error that was. However `std::io::ErrorKind` defines all possible errors, even those that may not be possible to occur in a specific case. For example, it defines the `UnexpectedEof` variant, which can occur during `std::fs::File::read()` but definitely not during `std::fs::File::create()` even though the error kind (`std::io::ErrorKind`) could technically be `ErrorKind::UnexpectedEof`. Therefor, it has to be stated in the doc comment which of the error variants in `std::io::ErrorKind` can actually occur in any specific case and, because a match must be complete in Rust, error kinds that cannot occur even have to be accounted for during error resolution. While this *is* a solution to error handling, it definitely is not the best one.

An alternative would have been to create one error type for each function, which can be enums defining only variants for those errors that can actually occur. However, this leads to problems with indirect errors. For example, one can think of the following scenario:

```rust
struct PermissionDenied; // permission denied error
struct FileNotFound; // file not found error

enum OpenError {
	FileNotFound(FileNotFound),
	PermissionDenied(PermissionDenied)
}

enum CreateError {
	PermissionDenied(PermissionDenied)
}

enum OpenOrCreateError { ? }

fn open(...) -> Result<..., OpenError> { ... }
fn create(...) -> Result<..., CreateError> { ... }
fn read_or_create(...) -> Result<..., OpenOrCreateError> { ... }
```

Let's say the `read_or_create(...)` method calls the `create()` method for creating a file if it doesn't exist and the `open()` method to open the file. It then reads the contents from the given file and returns them if no error during opening, creating or reading occurred. How should the `OpenOrCreateError` be defined? You could define one variant for `OpenError` and one for `CreateError` and on further one to encode an error during reading, like this:

```rust
struct Read; // error during reading

enum OpenOrCreateError {
	OpenError(OpenError),
	CreateError(CreateError),
	Read(Read)
}

impl From<OpenError> for OpenOrCreateError {
	fn from(err: OpenError) -> Self {
		Self::OpenError(err)
	}
}

impl From<CreateError> for OpenOrCreateError {
	fn from(err: CreateError) -> Self {
		Self::CreateError(err)
	}
}

impl From<ReadError> for OpenOrCreateError {
	fn from(err: Read) -> Self {
		Self::Read(err)
	}
}
```

The problem with this is approach is that a `PermissionDenied` error is now encode in two distinct ways despite being the exact same error: it can either be represented by `OpenOrCreateError::OpenError::PermissionDenied(...)` or by `OpenOrCreateError::CreateError::PermissionDenied(...)`. A caller of `read_or_create()` now has to account for two possibilities one error can occur. Another approach would be to flatten out the error hierarchy, like this:

```rust
struct Read; // error during reading

enum OpenOrCreateError {
	FileNotFound(FileNotFound),
	PermissionDenied(PermissionDenied),
	Read(Read)
}

impl From<OpenError> for OpenOrCreateError {
	fn from(err: OpenError) -> Self {
		match err {
			OpenError::FileNotFound(fnf) => Self::FileNotFound(fnf),
			OpenError::PermissionDenied(pd) => Self::PermissionDenied(pd)
		}
	}
}

impl From<CreateError> for OpenOrCreateError {
	fn from(err: CreateError) -> Self {
		match err {
			CreateError::PermissionDenied(pd) => Self::PermissionDenied(pd)
		}
	}
}

impl From<ReadError> for OpenOrCreateError {
	fn from(err: Read) -> Self {
		Self::Read(err)
	}
}
```

This is a better approach as the `PermissionDenied` error is now only encoded in one way. However, when errors get more complex, this becomes cumbersome to write as all the from implementations now contain matches over all variants. This is where this crate comes in. With `composite-error`, the above errors can be specified like this:

```rust
use composite_error::CompositeError;

struct PermissionDenied; // permission denied error
struct FileNotFound; // file not found error
struct Read; // error during reading

#[derive(CompositeError)]
enum OpenError {
	FileNotFound(FileNotFound),
	PermissionDenied(PermissionDenied)
}

#[derive(CompositeError)]
enum CreateError {
	PermissionDenied(PermissionDenied)
}

#[derive(CompositeError)]
enum OpenOrCreateError {
	#[from(OpenError)]
	FileNotFound(FileNotFound),
	
	#[from(OpenError, CreateError)]
	PermissionDenied(PermissionDenied),
	
	Read(Read)
}
```

## Why oh why yet another error utility for Rust?

There is a reason! Most other error handling crates manage building a hierarchy of errors while `composite-error` tries to keep the error hierarchy flat, which makes error types simpler. Also, `composite-error` is kept as slim as possible to allow an easy start. That's the reason `composite-error` even exists; making things easier!

### What alternatives are there and why is `composite_error` better?

- [error-chain] - While being powerful, it is kind of hard to use when first getting started with it. Also, its syntax does not embrace Rust. It introduces some function-like macros defining their own syntax. `composite-error` limits itself to derive macros and derive macro helper attributes, the minimum needed to generate the impls for the error types.
- [quick-error] - Error specification is easy but the specification of errors also happens using function-like macros introducing own syntax.
- [failure] - It is very complex. `composite-error` tries to be as simple as possible.

Also, non of these crates has the declared goal to keep the error hierarchy flat. This is the main contribution of the `composite-error` crate.

[error-chain]: https://crates.io/crates/error-chain
[quick-error]: https://crates.io/crates/quick-error
[failure]: https://crates.io/crates/failure


