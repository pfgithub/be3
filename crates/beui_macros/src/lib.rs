use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = parse_macro_input!(item as ItemFn);
    let name = sig.ident.to_string();

    quote! {
        #(#attrs)*
        #vis #sig {
            ::beui::reactive::component(#name, move || #block)
        }
    }
    .into()
}
