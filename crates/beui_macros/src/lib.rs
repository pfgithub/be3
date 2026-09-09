use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::braced;
use syn::parenthesized;
use syn::parse::{Parse, ParseStream};
use syn::{
    parse_macro_input, Expr, ExprLit, FnArg, GenericArgument, Ident, ItemFn, Lit, Pat, PatType,
    PathArguments, Token, Type,
};

struct Prop {
    ident: Ident,
    ty: Type,
    inner_ty: Option<Type>,
    reactive_inner_ty: Option<Type>,
    is_children: bool,
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

fn is_named_type(ty: &Type, name: &str) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
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
            let FnArg::Typed(PatType { pat, ty, .. }) = arg else {
                panic!("#[component] functions cannot take `self`");
            };
            let ident = match pat.as_ref() {
                Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => panic!("#[component] props must be simple identifiers"),
            };
            let inner_ty = generic_inner(ty, "Option");
            let reactive_inner_ty = generic_inner(ty, "Prop");
            let is_children = is_named_type(ty, "Children");
            Prop {
                ident,
                ty: (**ty).clone(),
                inner_ty,
                reactive_inner_ty,
                is_children,
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
        if prop.is_children {
            quote! {
                pub fn #ident(mut self, children: impl Into<::beui::reactive::Children>) -> Self {
                    self.#ident = Some(children.into());
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
        let value = if prop.inner_ty.is_some() {
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
    let finish = quote! { ::beui::reactive::component(#name, move || #block) };

    let prop_idents: Vec<_> = props.iter().map(|prop| prop.ident.clone()).collect();

    quote! {
        #(#attrs)*
        #vis struct #builder_ident #generics #where_clause {
            #(#fields,)*
        }

        impl #generics ::core::default::Default for #builder_ident #generics #where_clause {
            fn default() -> Self {
                Self {
                    #(#prop_idents: ::core::default::Default::default(),)*
                }
            }
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

struct ViewAttr {
    key: Ident,
    value: Expr,
}

impl Parse for ViewAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value = if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            content.parse::<Expr>()?
        } else {
            let lit: Lit = input.parse()?;
            Expr::Lit(ExprLit {
                attrs: Vec::new(),
                lit,
            })
        };
        Ok(ViewAttr { key, value })
    }
}

enum ViewSizing {
    Intrinsic,
    Fixed(Expr),
    Percent(Expr),
}

impl Parse for ViewSizing {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        match ident.to_string().as_str() {
            "intrinsic" => Ok(ViewSizing::Intrinsic),
            "fixed" => {
                let args;
                parenthesized!(args in input);
                Ok(ViewSizing::Fixed(args.parse()?))
            }
            "percent" => {
                let args;
                parenthesized!(args in input);
                Ok(ViewSizing::Percent(args.parse()?))
            }
            other => Err(syn::Error::new(
                ident.span(),
                format!("unknown sizing `@{other}`, expected `@intrinsic`, `@fixed(size)`, or `@percent(weight)`"),
            )),
        }
    }
}

enum ViewChildKind {
    Node(ViewNode),
    Expr(Expr),
}

struct ViewChild {
    sizing: Option<ViewSizing>,
    kind: ViewChildKind,
}

impl Parse for ViewChild {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let sizing = if input.peek(Token![@]) {
            input.parse::<Token![@]>()?;
            Some(input.parse()?)
        } else {
            None
        };

        let kind = if input.peek(Token![<]) {
            ViewChildKind::Node(input.parse()?)
        } else {
            let content;
            braced!(content in input);
            ViewChildKind::Expr(content.parse()?)
        };

        Ok(ViewChild { sizing, kind })
    }
}

struct ViewNode {
    tag: Ident,
    props: Vec<ViewAttr>,
    children: Option<Vec<ViewChild>>,
}

impl Parse for ViewNode {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![<]>()?;
        let tag: Ident = input.parse()?;

        let mut props = Vec::new();
        while !input.peek(Token![>]) && !input.peek(Token![/]) {
            props.push(input.parse::<ViewAttr>()?);
        }

        if input.peek(Token![/]) {
            input.parse::<Token![/]>()?;
            input.parse::<Token![>]>()?;
            return Ok(ViewNode {
                tag,
                props,
                children: None,
            });
        }
        input.parse::<Token![>]>()?;

        let mut children = Vec::new();
        while !(input.peek(Token![<]) && input.peek2(Token![/])) {
            children.push(input.parse::<ViewChild>()?);
        }

        input.parse::<Token![<]>()?;
        input.parse::<Token![/]>()?;
        let close_tag: Ident = input.parse()?;
        if close_tag != tag {
            return Err(syn::Error::new(
                close_tag.span(),
                format!("mismatched closing tag `</{close_tag}>`, expected `</{tag}>`"),
            ));
        }
        input.parse::<Token![>]>()?;

        Ok(ViewNode {
            tag,
            props,
            children: Some(children),
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
                let node = match &child.kind {
                    ViewChildKind::Node(node) => expand_view_node(node),
                    ViewChildKind::Expr(expr) => quote! { #expr },
                };
                match &child.sizing {
                    None | Some(ViewSizing::Intrinsic) => {
                        quote! { ::beui::reactive::intrinsic(#node) }
                    }
                    Some(ViewSizing::Fixed(size)) => {
                        quote! { ::beui::reactive::fixed(#node, #size) }
                    }
                    Some(ViewSizing::Percent(weight)) => {
                        quote! { ::beui::reactive::percent(#node, #weight) }
                    }
                }
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
