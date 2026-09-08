use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::braced;
use syn::bracketed;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, Attribute, Expr, ExprClosure, FnArg, GenericArgument, Ident, ItemFn, Pat,
    PatType, PathArguments, Token, Type,
};

struct Prop {
    ident: Ident,
    ty: Type,
    inner_ty: Option<Type>,
    reactive_inner_ty: Option<Type>,
    with: Option<ExprClosure>,
    default: Option<Expr>,
}

fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn generic_inner(ty: &Type, name: &str) -> Option<Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty.clone()),
        _ => None,
    })
}

fn take_prop_attr(attrs: &mut Vec<Attribute>) -> (Option<ExprClosure>, Option<Expr>) {
    let mut with = None;
    let mut default = None;
    attrs.retain(|attr| {
        if !attr.path().is_ident("prop") {
            return true;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("with") {
                let expr: Expr = meta.value()?.parse()?;
                let Expr::Closure(closure) = expr else {
                    return Err(meta.error("#[prop(with = ...)] must be a closure"));
                };
                with = Some(closure);
            } else if meta.path.is_ident("default") {
                default = Some(meta.value()?.parse()?);
            }
            Ok(())
        })
        .expect("invalid #[prop(...)] attribute");
        false
    });
    (with, default)
}

#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(item, true)
}

#[proc_macro_attribute]
pub fn builder(_attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(item, false)
}

fn expand(item: TokenStream, shadowed: bool) -> TokenStream {
    let ItemFn {
        attrs,
        vis,
        mut sig,
        block,
    } = parse_macro_input!(item as ItemFn);
    let name = sig.ident.to_string();
    let fn_ident = sig.ident.clone();
    let builder_ident = format_ident!("{}Builder", pascal_case(&name));
    let output = sig.output.clone();

    let props: Vec<Prop> = sig
        .inputs
        .iter_mut()
        .map(|arg| {
            let FnArg::Typed(PatType { attrs, pat, ty, .. }) = arg else {
                panic!("#[component] functions cannot take `self`");
            };
            let ident = match pat.as_ref() {
                Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => panic!("#[component] props must be simple identifiers"),
            };
            let (with, default) = take_prop_attr(attrs);
            let inner_ty = generic_inner(ty, "Option");
            let reactive_inner_ty = generic_inner(ty, "Prop");
            Prop {
                ident,
                ty: (**ty).clone(),
                inner_ty,
                reactive_inner_ty,
                with,
                default,
            }
        })
        .collect();

    let fields = props.iter().map(|prop| {
        let ident = &prop.ident;
        let ty = &prop.ty;
        quote! { #ident: Option<#ty> }
    });

    let setters = props.iter().map(|prop| {
        let ident = &prop.ident;
        if let Some(with) = &prop.with {
            let input = with
                .inputs
                .first()
                .expect("#[prop(with = ...)] closure must take one argument");
            let Pat::Type(PatType { pat, ty, .. }) = input else {
                panic!("#[prop(with = ...)] closure argument must be typed");
            };
            let body = &with.body;
            let assign = if prop.inner_ty.is_some() {
                quote! { self.#ident = Some(Some(#body)); }
            } else {
                quote! { self.#ident = Some(#body); }
            };
            quote! {
                pub fn #ident(mut self, #pat: #ty) -> Self {
                    #assign
                    self
                }
            }
        } else if let Some(inner_ty) = &prop.inner_ty {
            quote! {
                pub fn #ident(mut self, value: #inner_ty) -> Self {
                    self.#ident = Some(Some(value));
                    self
                }
            }
        } else if let Some(inner_ty) = &prop.reactive_inner_ty {
            quote! {
                pub fn #ident(mut self, value: impl ::beui::reactive::IntoProp<#inner_ty>) -> Self {
                    self.#ident = Some(::beui::reactive::IntoProp::into_prop(value));
                    self
                }
            }
        } else {
            let ty = &prop.ty;
            quote! {
                pub fn #ident(mut self, value: #ty) -> Self {
                    self.#ident = Some(value);
                    self
                }
            }
        }
    });

    let field_lets = props.iter().map(|prop| {
        let ident = &prop.ident;
        let ident_str = ident.to_string();
        let value = if let Some(default) = &prop.default {
            quote! { self.#ident.unwrap_or_else(|| #default) }
        } else if prop.inner_ty.is_some() {
            quote! { self.#ident.unwrap_or(None) }
        } else if prop.reactive_inner_ty.is_some() {
            quote! {
                self.#ident.unwrap_or_else(|| {
                    ::beui::reactive::Prop::Static(::core::default::Default::default())
                })
            }
        } else {
            quote! {
                self.#ident.unwrap_or_else(|| {
                    panic!(
                        "missing required prop `{}` for component `{}`",
                        #ident_str, #name,
                    )
                })
            }
        };
        quote! { let #ident = #value; }
    });

    let generics = &sig.generics;
    let where_clause = &sig.generics.where_clause;
    let finish = if shadowed {
        quote! { ::beui::reactive::component(#name, move || #block) }
    } else {
        quote! { #block }
    };

    quote! {
        #(#attrs)*
        #[derive(Default)]
        #vis struct #builder_ident #generics #where_clause {
            #(#fields,)*
        }

        impl #generics #builder_ident #generics #where_clause {
            #(#setters)*

            pub fn build(self) #output {
                #(#field_lets)*
                #finish
            }
        }

        #vis fn #fn_ident #generics () -> #builder_ident #generics #where_clause {
            #builder_ident::default()
        }
    }
    .into()
}

struct ViewProp {
    key: Ident,
    value: Expr,
}

impl Parse for ViewProp {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let value: Expr = input.parse()?;
        Ok(ViewProp { key, value })
    }
}

enum ViewChild {
    Node(ViewNode),
    Expr(Expr),
}

impl Parse for ViewChild {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) && input.peek2(syn::token::Brace) {
            Ok(ViewChild::Node(input.parse()?))
        } else {
            Ok(ViewChild::Expr(input.parse()?))
        }
    }
}

struct ViewNode {
    tag: Ident,
    props: Vec<ViewProp>,
    children: Option<Vec<ViewChild>>,
}

impl Parse for ViewNode {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let tag: Ident = input.parse()?;

        let props_input;
        braced!(props_input in input);
        let props = Punctuated::<ViewProp, Token![,]>::parse_terminated(&props_input)?
            .into_iter()
            .collect();

        let children = if input.peek(syn::token::Bracket) {
            let children_input;
            bracketed!(children_input in input);
            let children = Punctuated::<ViewChild, Token![,]>::parse_terminated(&children_input)?
                .into_iter()
                .collect();
            Some(children)
        } else {
            None
        };

        Ok(ViewNode {
            tag,
            props,
            children,
        })
    }
}

fn expand_view_node(node: &ViewNode) -> proc_macro2::TokenStream {
    let tag = &node.tag;
    let setters = node.props.iter().map(|prop| {
        let key = &prop.key;
        let value = &prop.value;
        quote! { .#key(#value) }
    });

    match &node.children {
        None => quote! { #tag() #(#setters)* .build() },
        Some(children) => {
            let items = children.iter().map(|child| {
                let child = match child {
                    ViewChild::Node(node) => expand_view_node(node),
                    ViewChild::Expr(expr) => quote! { #expr },
                };
                quote! { ::beui::reactive::intrinsic(#child) }
            });
            quote! {
                #tag() #(#setters)* .children([#(#items),*]) .build()
            }
        }
    }
}

#[proc_macro]
pub fn view(item: TokenStream) -> TokenStream {
    let node = parse_macro_input!(item as ViewNode);
    let expanded = expand_view_node(&node);
    quote! { { #expanded } }.into()
}
