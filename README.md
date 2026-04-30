# dynamic-wrapping

Proc macros for libraries that expose internal types through dynamic traits.

These macros allow library authors to keep internal implementations private while giving users the ability to:
- Choose their own container type (`Box`, `Rc`, `Arc`, custom handles)
- Add performance-critical operations that get monomorphized per concrete type
- Move dynamic dispatch to the outer boundary, keeping hot loops optimized

## The Problem

Many libraries select concrete implementations at runtime based on configuration or data characteristics, but only expose a trait to users. Clients want to add performance-critical operations, but doing so through `dyn Trait` means paying for dynamic dispatch on every call—even inside hot loops.

```rust
// Library exposes only the trait
pub trait ItemCollection {
    fn get_value(&self, key: u32) -> u32;
}

// User wants to add batch operations, but this is slow:
fn batch_lookup(collection: &dyn ItemCollection, keys: &[u32]) -> Vec<u32> {
    keys.iter().map(|k| collection.get_value(*k)).collect()
    // ^ vtable lookup on every iteration!
}
```

## The Solution

This crate provides a factory pattern where the library selects a concrete type at runtime and passes it to the user's wrapper. Users can then implement blanket traits that get monomorphized for each concrete type—moving dynamic dispatch to the outer boundary, not the hot loop.

## Library Usage

Mark your trait as wrappable and provide a default wrapper:

```rust
use dynamic_wrapping::{wrappable, wrapping};

#[wrappable]
pub trait ItemCollection {
    fn get_value(&self, key: u32) -> u32;
    fn get_message(&self) -> &str;
}

#[wrapping(
    ItemCollection => Box<dyn ItemCollection + 'a>, Box::new
)]
pub struct BoxDynWrapping;
```

Expose a factory method that lets clients choose the wrapper:

```rust
pub struct CollectionStorage<'a> {
    code: u8,  // Runtime selector
    name: &'a str,
}

impl<'a> CollectionStorage<'a> {
    // Default path: returns Box<dyn ItemCollection>
    pub fn open(&self) -> <BoxDynWrapping as ItemCollectionWrapper<'a>>::Wrapped {
        self.open_with::<BoxDynWrapping>()
    }

    // Generic path: client chooses the wrapper
    pub fn open_with<W>(self) -> W::Wrapped
    where
        W: ItemCollectionWrapper<'a>,
    {
        match self.code {
            0 => W::wrap(ConcreteCollection1 { message: self.name }),
            1 => W::wrap(ConcreteCollection2 { message: self.name }),
            _ => W::wrap(ConcreteCollection3 { message: self.name }),
        }
    }
}
```

## User Usage

Users can implement their own wrapper with performance-critical operations:

```rust
use std::rc::Rc;

// User adds a performance-critical operation
trait ItemCollectionExt: ItemCollection {
    fn batch_lookup(&self, keys: &[u32]) -> Vec<u32>;
}

// Blanket implementation: monomorphized per concrete type
impl<C: ItemCollection> ItemCollectionExt for C {
    fn batch_lookup(&self, keys: &[u32]) -> Vec<u32> {
        // Hot loop: self.get_value is resolved at compile time!
        keys.iter().map(|k| self.get_value(*k)).collect()
    }
}

// User's custom wrapper
struct MyWrapper;

impl<'a> ItemCollectionWrapper<'a> for MyWrapper {
    type Wrapped = Rc<dyn ItemCollectionExt + 'a>;
    fn wrap<C: ItemCollection + 'a>(c: C) -> Self::Wrapped {
        Rc::new(c)
    }
}

fn main() {
    let storage = CollectionStorage::new(0, "example");
    let collection = storage.open_with::<MyWrapper>();
    
    // batch_lookup is monomorphized for the concrete collection type
    let results = collection.batch_lookup(&[1, 2, 3, 4]);
}
```

## How It Works

1. Library marks trait with `#[wrappable]` → generates `ItemCollectionWrapper<'a>` trait
2. Library provides `#[wrapping(...)]` wrapper struct → implements the wrapper trait
3. Library exposes `open_with<W>()` factory method
4. Client implements their own wrapper and blanket traits
5. Blanket impls get monomorphized per concrete type, avoiding vtable lookups in hot loops

Dynamic dispatch happens once (when calling `batch_lookup`), not on every iteration inside it.

## License

MIT License - see [LICENSE](LICENSE) for details.
