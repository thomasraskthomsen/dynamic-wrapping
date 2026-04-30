# dynamic-wrapping

Rust macros for monomorphized blanket implementations of dynamic traits.

Create object-safe dynamic entry points that transfer control into concrete implementations. Your custom blanket implementations get monomorphized per concrete type—dynamic dispatch where you want it, static dispatch where you need it.

## Overview

This crate lets library authors expose internal trait implementations while giving clients control over:
- Container type (`Box`, `Rc`, `Arc`, custom handles)
- Performance-critical code paths through monomorphized blanket implementations
- Where dynamic dispatch happens (boundary) vs. where it doesn't (hot loops)

## Usage

```rust
use dynamic_wrapping::{wrappable, wrapping};

// 1. Mark your trait as wrappable
#[wrappable]
pub trait ItemCollection {
    fn len(&self) -> u32;
    fn get_value(&self, key: u32) -> u32;
}

// 2. Define a wrapper that produces your preferred container type
#[wrapping(
    ItemCollection => Box<dyn ItemCollection + 'a>, Box::new
)]
pub struct BoxDynWrapping;
```

The macros generate:
- `ItemCollectionWrapper<'a>` trait with `type Wrapped` and `fn wrap<C: ItemCollection + 'a>(c: C) -> Self::Wrapped`
- Implementation of `ItemCollectionWrapper<'a>` for `BoxDynWrapping` that wraps in `Box<dyn ItemCollection + 'a>`

Your library can then expose:

```rust
pub struct CollectionStorage<'a> { /* ... */ }

impl<'a> CollectionStorage<'a> {
    // Default: returns Box<dyn ItemCollection>
    pub fn open_collection(&self) -> <BoxDynWrapping as ItemCollectionWrapper<'a>>::Wrapped {
        self.open_collection_with::<BoxDynWrapping>()
    }

    // Generic: client chooses the wrapper
    pub fn open_collection_with<W>(&self) -> W::Wrapped
    where
        W: ItemCollectionWrapper<'a>,
    {
        // Match on runtime value to select concrete type
        match self.code {
            0 => W::wrap(DummyCollection1 { /* ... */ }),
            1 => W::wrap(DummyCollection2 { /* ... */ }),
            _ => W::wrap(DummyCollection3 { /* ... */ }),
        }
    }
}
```

## Why Monomorphization Matters

In Rust, calling methods through `dyn Trait` (dynamic dispatch) has overhead:
- Virtual table lookup on every call
- Prevents inlining across the trait boundary
- Blocks many compiler optimizations

This pattern moves the dynamic dispatch to the **outer boundary** (where you call `batch_lookup`), while the **inner critical path** (the loop inside `batch_lookup`) uses static dispatch with full optimization:

```rust
// This blanket impl gets compiled separately for EACH concrete type:
// - DummyCollection1::batch_lookup  (fully optimized for DummyCollection1)
// - DummyCollection2::batch_lookup  (fully optimized for DummyCollection2)
// - DummyCollection3::batch_lookup  (fully optimized for DummyCollection3)
impl<C: ItemCollection> MySpecialization for C {
    fn batch_lookup(&self, keys: &[u32], values: &mut [u32]) {
        // Static dispatch: self.get_value is resolved at compile time
        // for the specific concrete type C, not through a vtable
        for (key, value) in keys.iter().zip(values.iter_mut()) {
            *value = self.get_value(*key);  // inlined, optimized per type
        }
    }
}
```

The result: you pay for dynamic dispatch once (the outer call to `batch_lookup`), not on every iteration of your hot loop.

## Client Usage

Clients can then use their own wrapper with blanket implementations:

```rust
use std::rc::Rc;

// Client's specialization trait
trait MySpecialization: ItemCollection {
    fn batch_lookup(&self, keys: &[u32], values: &mut [u32]);
}

// BLANKET IMPLEMENTATION - gets monomorphized per concrete type!
impl<C: ItemCollection> MySpecialization for C {
    fn batch_lookup(&self, keys: &[u32], values: &mut [u32]) {
        // Hot loop is monomorphized!
        for (key, value) in keys.iter().zip(values.iter_mut()) {
            *value = self.get_value(*key);
        }
    }
}

// Client's custom wrapper
struct MySpecializationWrapper;

impl<'a> ItemCollectionWrapper<'a> for MySpecializationWrapper {
    type Wrapped = Rc<dyn MySpecialization + 'a>;
    fn wrap<C: ItemCollection + 'a>(c: C) -> Self::Wrapped {
        Rc::new(c)
    }
}

fn main() {
    let storage = CollectionStorage::new(...);
    let specialization = storage.open_collection_with::<MySpecializationWrapper>();
    
    // batch_lookup is monomorphized for the concrete collection type!
    specialization.batch_lookup(&keys, &mut values);
}
```

## License

MIT License - see [LICENSE](LICENSE) for details.
