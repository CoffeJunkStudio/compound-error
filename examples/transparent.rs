use compound_error::CompoundError;

#[derive(Debug, CompoundError)]
pub struct Foo1;

#[derive(Debug, CompoundError)]
pub struct Foo2;

#[derive(Debug, CompoundError)]
#[compound_error(transparent)]
pub enum Foo {
	Foo1(Foo1),
	Foo2(Foo2),
}

#[derive(Debug, CompoundError)]
pub struct Bar;

#[derive(Debug, CompoundError)]
pub enum ExampleError {
	#[compound_error(transparent)]
	Foo(Foo),
	Bar(Bar),
}

pub fn throws_foo() -> Result<(), Foo> {
	Err(Foo1)?
}

pub fn throws_bar() -> Result<(), Bar> {
	Err(Bar)
}

pub fn throws_compound_err(which: u8) -> Result<(), ExampleError> {
	if which == 0 {
		Ok(())
	} else if which == 1 {
		Ok(throws_foo()?)
	} else {
		Ok(throws_bar()?)
	}
}

fn main() {
	if let Err(e) = throws_compound_err(1) {
		println!("Error: {}", e);
	}
	if let Err(e) = throws_compound_err(2) {
		println!("Error: {}", e);
	}
}
