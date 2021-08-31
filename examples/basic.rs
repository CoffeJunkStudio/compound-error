use compound_error::CompoundError;

#[derive(Debug, CompoundError)]
#[compound_error(
	//title = "Foo Error",
	//description = "some foo error"
)]
pub struct Foo;

#[derive(Debug, CompoundError)]
pub struct Goo;

#[derive(Debug, CompoundError)]
pub struct Bar;

#[derive(Debug, CompoundError)]
pub struct Baz;

#[derive(Debug, CompoundError)]
pub enum CompoundFoo<T: std::fmt::Debug> {
	Foo(Foo),
	Bar(Bar),

	#[compound_error(no_source)]
	Other(T),
}

#[derive(Debug, CompoundError)]
pub enum CompoundGoo {
	Foo(Foo),
	Goo(Goo),
}

#[derive(Debug, CompoundError)]
pub struct Wrap<T: 'static + std::fmt::Debug>(T);

#[derive(Debug, CompoundError)]
#[compound_error(title = "Compound Bar", description = "compound bar error")]
pub enum CompoundBar<T: 'static + std::fmt::Debug + std::error::Error> {
	#[compound_error(inline_from("CompoundFoo<T>", CompoundGoo))]
	Foo(crate::Foo),
	#[compound_error(inline_from("CompoundFoo<T>"))]
	Bar(Bar),
	#[compound_error(inline_from(CompoundGoo))]
	Goo(Goo),
	Baz(Baz),
	#[compound_error(inline_from("CompoundFoo<T>"))]
	Other(T),
	Wrapper(Wrap<T>),
}

pub fn throws_wrap<T: 'static + std::fmt::Debug>(err: T) -> Result<(), Wrap<T>> {
	Err(Wrap(err))
}

pub fn throws_foo() -> Result<(), Foo> {
	Err(Foo)
}

pub fn throws_goo() -> Result<(), Goo> {
	Err(Goo)
}

pub fn throws_bar() -> Result<(), Bar> {
	Err(Bar)
}

pub fn throws_baz() -> Result<(), Baz> {
	Err(Baz)
}

pub fn throws_compound_foo<T: std::fmt::Debug>(which: u8, _: T) -> Result<(), CompoundFoo<T>> {
	if which == 0 {
		Ok(())
	} else if which == 1 {
		Ok(throws_foo()?)
	} else {
		Ok(throws_bar()?)
	}
}

pub fn throws_compound_goo(which: u8) -> Result<(), CompoundGoo> {
	if which == 0 {
		Ok(())
	} else if which == 1 {
		Ok(throws_foo()?)
	} else {
		Ok(throws_goo()?)
	}
}

pub fn throws_compound_bar<T: std::fmt::Debug + std::error::Error>(
	which: u8,
	which2: u8,
	other: T,
) -> Result<(), CompoundBar<T>> {
	if which == 0 {
		Ok(())
	} else if which == 1 {
		Err(CompoundBar::Other(other))
	//Ok(throws_foo()?)
	} else if which == 2 {
		Ok(throws_bar()?)
	} else if which == 3 {
		Ok(throws_baz()?)
	} else if which == 4 {
		Ok(throws_goo()?)
	} else if which == 5 {
		Ok(throws_compound_foo(which2, other)?)
	} else if which == 6 {
		Ok(throws_wrap(other)?)
	} else {
		Ok(throws_compound_goo(which2)?)
	}
}

fn main() {
	if let Err(e) = throws_compound_bar(5, 1, Foo) {
		println!("Error: {}", e);
	}
}
