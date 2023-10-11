use proc_macro::TokenStream;
use quote::{format_ident, quote};
// use syn::parse::Parse;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_error::*;
use syn::parse_macro_input;
use syn::Item;

const HAX_COMPILATION: &str = "hax_compilation";
const HAX_TOOL: &str = "_hax";

macro_rules! no_argument {
    () => {
        |at: TokenStream2| {
            if !at.is_empty() {
                abort!(at, "this attribute doesn't take any argument")
            }
            at
        }
    };
}

/// `passthrough_attribute!(NAME)` generates a proc-macro that expands
/// into the tool attribute `HAX_TOOL::NAME` when the cfg flag
/// `HAX_COMPILATION` is set.
macro_rules! passthrough_attribute {
    ($(#$a:tt)*$name:ident) => {
        passthrough_attribute!($(#$a)*$name, no_argument!());
    };
    ($(#$a:tt)*$name:ident, |$x:pat_param| $e:expr) => {
        passthrough_attribute!($(#$a)*$name, |$x: TokenStream2| $e);
    };
    ($(#$a:tt)*$name:ident, $validator:expr) => {
        #[proc_macro_error]
        #[proc_macro_attribute]
        $(#$a)*
        pub fn $name(attr: TokenStream, item: TokenStream) -> TokenStream {
            let attr: TokenStream2 = attr.into();
            let item: TokenStream2 = item.into();
            let hax_compilation = format_ident!("{}", HAX_COMPILATION);
            let hax_tool = format_ident!("{}", HAX_TOOL);
            let attr: TokenStream2 = $validator(attr);
            quote! {
                #[cfg_attr(#hax_compilation, #hax_tool::$name(#attr))]
                #item
            }
            .into()
        }
    };
}

passthrough_attribute!(
    /// Makes Hax ignore a function, a type, a trait or any other
    /// item. Hax's engine won't look at the item at all.
    skip
);

passthrough_attribute!(
    /// Makes Hax backends ignore a function, a type, a trait or any
    /// other item. Hax still processes the items up to the backends,
    /// and then drop them. This is useful for generating helper
    /// functions aimed at being inlined, for example, refinements.
    late_skip
);

#[proc_macro_error]
#[proc_macro_attribute]
/// Replace a Rust item with an item in the backend.
pub fn replace_with(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(item as syn::Path);
    quote! {#path}.into()
}

#[proc_macro_error]
#[proc_macro_attribute]
/// Enable the following attrubutes in the annotated item:
///  - (in a struct) `refine`: refine a type with a logical formula
pub fn hax_attributes(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: Item = parse_macro_input!(item as Item);

    struct AttrVisitor {}

    use syn::visit_mut;
    use syn::{Attribute, Expr};
    use visit_mut::VisitMut;
    impl VisitMut for AttrVisitor {
        fn visit_item_mut(&mut self, item: &mut Item) {
            visit_mut::visit_item_mut(self, item);
            use syn::spanned::Spanned;
            let span = item.span();
            let mut extra: Vec<Item> = vec![];
            match item {
                Item::Struct(s) => {
                    let only_one_field = s.fields.len() == 1;
                    let idents: Vec<_> = s
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(i, field)| {
                            let ident = field.ident.clone().unwrap_or(if only_one_field {
                                format_ident!("x")
                            } else {
                                format_ident!("x{}", i)
                            });
                            (ident, field.ty.clone())
                        })
                        .collect();
                    for (i, field) in s.fields.iter_mut().enumerate() {
                        let prev = &idents[0..=i];
                        let refine: Option<(&mut Attribute, Expr)> =
                            field.attrs.iter_mut().find_map(|attr| {
                                if attr.path().is_ident("refine") {
                                    let payload = attr.parse_args().ok()?;
                                    Some((attr, payload))
                                } else {
                                    None
                                }
                            });
                        if let Some((attr, refine)) = refine {
                            let binders: TokenStream2 = prev
                                .iter()
                                .map(|(name, ty)| quote! {#name: #ty, })
                                .collect();
                            let hax_tool = format_ident!("{}", HAX_TOOL);
                            use uuid::Uuid;
                            let uuid = format!("{}", Uuid::new_v4().simple());
                            let hax_compilation = format_ident!("{}", HAX_COMPILATION);
                            *attr = syn::parse_quote! { #[cfg_attr(#hax_compilation, #hax_tool::uuid(#uuid))] };
                            extra.push(syn::parse_quote! {
                                #[cfg(#hax_compilation)]const _: () = {
                                    #[#hax_tool::associated_with(#uuid, refinement)]
                                    #[#hax_tool::late_skip]
                                    fn refinement(#binders) -> bool { #refine }
                                };
                            })
                        }
                    }
                }
                _ => (),
            }
            let extra: TokenStream2 = extra.iter().map(|extra| quote! {#extra}).collect();
            *item = Item::Verbatim(quote! {#extra #item});
        }
    }

    let mut v = AttrVisitor {};
    let mut item = item;
    v.visit_item_mut(&mut item);

    quote! { #item }.into()
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn hax_no_unfold_attr(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr: TokenStream2 = attr.into();
    let item: TokenStream2 = item.into();
    quote! {
        #[hax::#attr]
        #[cfg_attr(not(feature = "hax_compilation"), #attr )]
        #item
    }.into()
}
