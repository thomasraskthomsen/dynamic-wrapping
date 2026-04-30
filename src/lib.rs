use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemTrait, Token, Type, Expr};
use syn::parse::{Parse, ParseStream};

/// Marks a trait as "wrappable" to generate a wrapper trait for it.
///
/// This attribute generates a wrapper trait that allows concrete implementations
/// to be wrapped in client-chosen container types (Box, Rc, Arc, or custom handles).
///
/// # Arguments
///
/// * `attr` - Optional custom name for the wrapper trait. If empty, defaults to `{TraitName}Wrapper`.
/// * `item` - The trait definition to make wrappable.
///
/// # Generated Code
///
/// The macro generates a wrapper trait with the following form:
///
/// ```rust,ignore
/// pub trait {TraitName}Wrapper<'a> {
///     type Wrapped;
///     fn wrap<C: {TraitName} + 'a>(c: C) -> Self::Wrapped;
/// }
/// ```
///
/// # Examples
///
/// Basic usage with default wrapper name:
///
/// ```
/// use dynamic_wrapping::wrappable;
///
/// #[wrappable]
/// pub trait ItemCollection {
///     fn get_value(&self, key: u32) -> u32;
/// }
///
/// // Generates: ItemCollectionWrapper<'a>
/// ```
///
/// Custom wrapper name:
///
/// ```
/// use dynamic_wrapping::wrappable;
///
/// #[wrappable(MyCustomWrapper)]
/// pub trait ItemSet {
///     fn contains(&self, value: u32) -> bool;
/// }
///
/// // Generates: MyCustomWrapper<'a>
/// ```
#[proc_macro_attribute]
pub fn wrappable(attr: TokenStream, item: TokenStream) -> TokenStream {
    let trait_def = parse_macro_input!(item as ItemTrait);
    let trait_name = &trait_def.ident;
    let wrapper_name = if attr.is_empty() {
        syn::Ident::new(&format!("{}Wrapper", trait_name), trait_name.span())
    } else {
        parse_macro_input!(attr as syn::Ident)
    };

    let output = quote! {
        #trait_def

        pub trait #wrapper_name<'a> {
            type Wrapped;
            fn wrap<C: #trait_name + 'a>(c: C) -> Self::Wrapped;
        }
    };

    output.into()
}

struct WrappingEntry {
    wrapper_trait: syn::Ident,
    trait_name: syn::Ident,
    wrapped_type: Type,
    constructor: Expr,
}

impl Parse for WrappingEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let first: syn::Ident = input.parse()?;

        let (wrapper_trait, trait_name) = if input.peek(Token![for]) {
            input.parse::<Token![for]>()?;
            let trait_name: syn::Ident = input.parse()?;
            (first, trait_name)
        } else {
            let wrapper = syn::Ident::new(&format!("{}Wrapper", first), first.span());
            (wrapper, first)
        };

        input.parse::<Token![=>]>()?;
        let wrapped_type: Type = input.parse()?;
        input.parse::<Token![,]>()?;
        let constructor: Expr = input.parse()?;
        Ok(WrappingEntry { wrapper_trait, trait_name, wrapped_type, constructor })
    }
}

struct WrappingArgs {
    entries: Vec<WrappingEntry>,
}

impl Parse for WrappingArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();
        entries.push(input.parse()?);
        while input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
            if input.is_empty() {
                break;
            }
            entries.push(input.parse()?);
        }
        Ok(WrappingArgs { entries })
    }
}

/// Implements wrapper traits for a struct with specific container types.
///
/// This attribute applies to a struct and generates implementations of one or more
/// wrapper traits (created by `#[wrappable]`) for that struct.
///
/// # Syntax
///
/// The attribute accepts a comma-separated list of wrapping entries:
///
/// ```ignore
/// #[wrapping(
///     TraitName => WrappedType, Constructor;
///     WrapperTrait for TraitName => WrappedType, Constructor
/// )]
/// pub struct MyWrapper;
/// ```
///
/// Each entry specifies:
/// - `TraitName` or `WrapperTrait for TraitName` - the trait to implement
/// - `=> WrappedType` - the output type (e.g., `Box<dyn Trait + 'a>`)
/// - `, Constructor` - expression to convert a concrete type to the wrapped type (e.g., `Box::new`)
///
/// If only a trait name is provided (without `for`), the wrapper trait name is assumed
/// to be `{TraitName}Wrapper`. If `WrapperTrait for TraitName` is used, you can specify
/// a custom wrapper trait name.
///
/// # Arguments
///
/// * `attr` - The wrapping entries specifying traits, output types, and constructors.
/// * `item` - The struct definition to generate implementations for.
///
/// # Examples
///
/// Basic usage with default wrapper names:
///
/// ```
/// use dynamic_wrapping::{wrappable, wrapping};
///
/// #[wrappable]
/// pub trait ItemCollection {
///     fn get_value(&self, key: u32) -> u32;
/// }
///
/// #[wrapping(
///     ItemCollection => Box<dyn ItemCollection + 'a>, Box::new
/// )]
/// pub struct BoxDynWrapping;
///
/// // Generates:
/// // impl<'a> ItemCollectionWrapper<'a> for BoxDynWrapping {
/// //     type Wrapped = Box<dyn ItemCollection + 'a>;
/// //     fn wrap<C: ItemCollection + 'a>(c: C) -> Self::Wrapped {
/// //         Box::new(c)
/// //     }
/// // }
/// ```
///
/// Multiple traits with custom wrapper names:
///
/// ```
/// use dynamic_wrapping::{wrappable, wrapping};
///
/// #[wrappable]
/// pub trait ItemCollection {
///     fn get_value(&self, key: u32) -> u32;
/// }
///
/// #[wrappable(ItemSetWrapperRenamed)]
/// pub trait ItemSet {
///     fn contains(&self, value: u32) -> bool;
/// }
///
/// #[wrapping(
///     ItemCollection => Box<dyn ItemCollection + 'a>, Box::new;
///     ItemSetWrapperRenamed for ItemSet => Box<dyn ItemSet + 'a>, Box::new
/// )]
/// pub struct BoxDynWrapping;
/// ```
///
/// Using different container types:
///
/// ```
/// use dynamic_wrapping::{wrappable, wrapping};
/// use std::rc::Rc;
///
/// #[wrappable]
/// pub trait ItemCollection {
///     fn get_value(&self, key: u32) -> u32;
/// }
///
/// #[wrapping(
///     ItemCollection => Rc<dyn ItemCollection + 'a>, Rc::new
/// )]
/// pub struct RcDynWrapping;
/// ```
#[proc_macro_attribute]
pub fn wrapping(attr: TokenStream, item: TokenStream) -> TokenStream {
    let WrappingArgs { entries } = parse_macro_input!(attr as WrappingArgs);
    let item_struct = parse_macro_input!(item as syn::ItemStruct);
    let name = &item_struct.ident;

    let impls = entries.iter().map(|entry| {
        let wrapper_trait = &entry.wrapper_trait;
        let trait_name = &entry.trait_name;
        let wrapped_type = &entry.wrapped_type;
        let constructor = &entry.constructor;

        quote! {
            impl<'a> #wrapper_trait<'a> for #name {
                type Wrapped = #wrapped_type;

                fn wrap<C: #trait_name + 'a>(c: C) -> Self::Wrapped {
                    (#constructor)(c)
                }
            }
        }
    });

    let output = quote! {
        #item_struct

        #(#impls)*
    };

    output.into()
}
