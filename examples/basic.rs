use composite_error::CompositeError;

#[derive(Debug)]
pub struct Foo;

#[derive(Debug)]
pub struct Goo;

#[derive(Debug)]
pub struct Bar;

#[derive(Debug)]
pub struct Baz;


#[derive(Debug, CompositeError)]
pub enum CompositeFoo {
	Foo(Foo),
	Bar(Bar)
}

#[derive(Debug, CompositeError)]
pub enum CompositeGoo {
	Foo(Foo),
	Goo(Goo)
}

#[derive(Debug, CompositeError)]
pub enum CompositeBar {
	#[compound_error( inline_from(CompositeFoo, CompositeGoo) )]
	Foo(crate::Foo),
	#[compound_error( inline_from(CompositeFoo) )]
	Bar(Bar),
	#[compound_error( inline_from(CompositeGoo) )]
	Goo(Goo),
	Baz(Baz)
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

pub fn throws_composite_foo(which: u8) -> Result<(), CompositeFoo> {
	if which == 0 {
		Ok(())
	} else if which == 1 {
		Ok(throws_foo()?)
	} else {
		Ok(throws_bar()?)
	}
}

pub fn throws_composite_goo(which: u8) -> Result<(), CompositeGoo> {
	if which == 0 {
		Ok(())
	} else if which == 1 {
		Ok(throws_foo()?)
	} else {
		Ok(throws_goo()?)
	}
}

pub fn throws_composite_bar(which: u8, which2: u8) -> Result<(), CompositeBar> {
	if which == 0 {
		Ok(())
	} else if which == 1 {
		Ok(throws_foo()?)
	} else if which == 2 {
		Ok(throws_bar()?)
	} else if which == 3 {
		Ok(throws_baz()?)
	} else if which == 4 {
		Ok(throws_goo()?)
	} else if which == 5 {
		Ok(throws_composite_foo(which2)?)
	} else {
		Ok(throws_composite_goo(which2)?)
	}
}


fn main() {
	throws_composite_bar(5,1).unwrap();
}


