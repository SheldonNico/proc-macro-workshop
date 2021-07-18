use derive_debug::CustomDebug;
use std::fmt::Debug;
use std::marker::PhantomData;

pub trait Trait {
    type Value;
}

#[derive(CustomDebug)]
pub struct Field<T: Trait, S>
where
    T::Value : Debug
{
    values: Vec<PhantomData<T::Value>>,
    values2: Vec<T::Value>,
    values3: Vec<S>
}

fn assert_debug<F: Debug>() {}

fn main() {
    // Does not implement Debug, but its associated type does.
    struct Id;

    impl Trait for Id {
        type Value = u8;
    }

    assert_debug::<Field<Id, String>>();
}
