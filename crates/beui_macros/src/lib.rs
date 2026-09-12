use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::braced;
use syn::parenthesized;
use syn::parse::{Parse, ParseStream};
use syn::{
    parse_macro_input, Attribute, Expr, ExprLit, FnArg, GenericArgument, Ident, ItemFn, Lit, Pat,
    PatType, PathArguments, Token, Type,
};

struct Prop {
    ident: Ident,
    ty: Type,
    inner_ty: Option<Type>,
    optional_reactive_inner_ty: Option<Type>,
    reactive_inner_ty: Option<Type>,
    render: Option<Render>,
    optional_render: Option<Render>,
    func_args: Option<Vec<Type>>,
    optional_func_args: Option<Vec<Type>>,
    is_children: bool,
    is_child: bool,
    is_optional_child: bool,
    callback_args: Option<Vec<Type>>,
    is_click_callback: bool,
    default: Option<Expr>,
}

struct Render {
    once: bool,
    handle: Option<Type>,
}

fn render_kind(ty: &Type) -> Option<Render> {
    if is_named_type(ty, "Render") {
        Some(Render {
            once: true,
            handle: generic_inner(ty, "Render"),
        })
    } else if is_named_type(ty, "RenderFn") {
        Some(Render {
            once: false,
            handle: generic_inner(ty, "RenderFn"),
        })
    } else {
        None
    }
}

fn func_args(ty: &Type) -> Option<Vec<Type>> {
    if !is_named_type(ty, "Func") {
        return None;
    }
    let args = generic_args(ty, "Func")?;
    (args.len() == 2).then_some(args)
}

fn func_setter(
    ident: &Ident,
    args: &[Type],
    wrap: impl Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let (value, result) = (&args[0], &args[1]);
    let stored = wrap(quote! { ::beui::reactive::IntoFunc::into_func(value) });
    quote! {
        pub fn #ident(mut self, value: impl ::beui::reactive::IntoFunc<#value, #result>) -> Self {
            self.#ident = Some(#stored);
            self
        }
    }
}

fn render_setter(
    ident: &Ident,
    render: &Render,
    wrap: impl Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let (signature, build) = match (&render.handle, render.once) {
        (Some(handle), true) => (
            quote! { ::beui::reactive::IntoRender<#handle> },
            quote! { ::beui::reactive::IntoRender::into_render(value) },
        ),
        (Some(handle), false) => (
            quote! { ::beui::reactive::IntoRenderFn<#handle> },
            quote! { ::beui::reactive::IntoRenderFn::into_render_fn(value) },
        ),
        (None, true) => (
            quote! { ::core::ops::FnOnce() -> ::beui::NodeId + 'static },
            quote! { ::beui::reactive::Render::new(move |()| value()) },
        ),
        (None, false) => (
            quote! { ::core::ops::Fn() -> ::beui::NodeId + 'static },
            quote! { ::beui::reactive::RenderFn::new(move |()| value()) },
        ),
    };
    let stored = wrap(build);
    quote! {
        pub fn #ident(mut self, value: impl #signature) -> Self {
            self.#ident = Some(#stored);
            self
        }
    }
}

fn take_prop_default(attrs: &mut Vec<Attribute>) -> Option<Expr> {
    let position = attrs.iter().position(|attr| attr.path().is_ident("prop"))?;
    let attr = attrs.remove(position);
    let mut default = None;
    attr.parse_nested_meta(|meta| {
        if !meta.path.is_ident("default") {
            return Err(meta.error("expected `default = <expr>`"));
        }
        default = Some(meta.value()?.parse::<Expr>()?);
        Ok(())
    })
    .expect("#[prop(...)] expects `default = <expr>`");
    Some(default.expect("#[prop(...)] expects `default = <expr>`"))
}

struct ComponentAttr {
    base: bool,
}

impl Parse for ComponentAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(ComponentAttr { base: false });
        }
        let ident: Ident = input.parse()?;
        if ident != "base" {
            return Err(syn::Error::new(
                ident.span(),
                "expected `base`, e.g. `#[component(base)]`",
            ));
        }
        Ok(ComponentAttr { base: true })
    }
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

fn generic_args(ty: &Type, name: &str) -> Option<Vec<Type>> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Some(Vec::new());
    };
    Some(
        args.args
            .iter()
            .filter_map(|arg| match arg {
                GenericArgument::Type(ty) => Some(ty.clone()),
                _ => None,
            })
            .collect(),
    )
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
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ComponentAttr { base } = parse_macro_input!(attr as ComponentAttr);
    let ItemFn {
        attrs,
        vis,
        mut sig,
        block,
    } = parse_macro_input!(item as ItemFn);
    let name = sig.ident.to_string();
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
            let default = take_prop_default(attrs);
            let inner_ty = generic_inner(ty, "Option");
            let optional_reactive_inner_ty = inner_ty
                .as_ref()
                .and_then(|inner| generic_inner(inner, "Prop"));
            let reactive_inner_ty = generic_inner(ty, "Prop");
            let optional_render = inner_ty.as_ref().and_then(render_kind);
            let optional_func_args = inner_ty.as_ref().and_then(func_args);
            let is_children = is_named_type(ty, "Children");
            let is_child = is_named_type(ty, "Child");
            let is_optional_child = inner_ty
                .as_ref()
                .is_some_and(|inner| is_named_type(inner, "Child"));
            Prop {
                ident,
                ty: (**ty).clone(),
                inner_ty,
                optional_reactive_inner_ty,
                reactive_inner_ty,
                render: render_kind(ty),
                optional_render,
                func_args: func_args(ty),
                optional_func_args,
                is_children,
                is_child,
                is_optional_child,
                callback_args: generic_args(ty, "Callback"),
                is_click_callback: is_named_type(ty, "ClickCallback"),
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
        if prop.is_children {
            quote! {
                pub fn #ident(mut self, children: impl Into<::beui::reactive::Children>) -> Self {
                    self.#ident = Some(children.into());
                    self
                }
            }
        } else if prop.is_child || prop.is_optional_child {
            let stored = if prop.is_optional_child {
                quote! { Some(children.only()) }
            } else {
                quote! { children.only() }
            };
            quote! {
                pub fn #ident(mut self, children: impl Into<::beui::reactive::Children>) -> Self {
                    let children: ::beui::reactive::Children = children.into();
                    self.#ident = #stored;
                    self
                }
            }
        } else if let Some(render) = &prop.optional_render {
            render_setter(ident, render, |build| quote! { Some(#build) })
        } else if let Some(render) = &prop.render {
            render_setter(ident, render, |build| build)
        } else if let Some(args) = &prop.optional_func_args {
            func_setter(ident, args, |build| quote! { Some(#build) })
        } else if let Some(args) = &prop.func_args {
            func_setter(ident, args, |build| build)
        } else if let Some(inner_ty) = &prop.optional_reactive_inner_ty {
            quote! {
                pub fn #ident(mut self, value: impl ::beui::reactive::IntoProp<#inner_ty>) -> Self {
                    self.#ident = Some(Some(::beui::reactive::IntoProp::into_prop(value)));
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
        } else if prop.is_click_callback {
            quote! {
                pub fn #ident(mut self, value: impl ::core::ops::FnMut() + 'static) -> Self {
                    self.#ident = Some(::beui::reactive::ClickCallback::new(value));
                    self
                }
            }
        } else if let Some(args) = &prop.callback_args {
            let signature = match args.as_slice() {
                [value] => quote! { ::core::ops::FnMut(#value) },
                [value, result] => quote! { ::core::ops::FnMut(#value) -> #result },
                _ => panic!("`Callback` props take one or two type arguments"),
            };
            quote! {
                pub fn #ident(mut self, value: impl #signature + 'static) -> Self {
                    self.#ident = Some(::beui::reactive::Callback::new(value));
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
        } else if prop.is_children {
            quote! { self.#ident.unwrap_or_default() }
        } else if prop.is_child {
            quote! {
                self.#ident.unwrap_or_else(|| {
                    panic!("component `{}` requires exactly one child", #name)
                })
            }
        } else if let Some(default) = &prop.default {
            if prop.reactive_inner_ty.is_some() {
                quote! {
                    self.#ident
                        .unwrap_or_else(|| ::beui::reactive::IntoProp::into_prop(#default))
                }
            } else {
                quote! { self.#ident.unwrap_or_else(|| #default) }
            }
        } else if prop.reactive_inner_ty.is_some() {
            quote! {
                self.#ident.unwrap_or_else(|| {
                    ::beui::reactive::Prop::Static(::core::default::Default::default())
                })
            }
        } else if prop.callback_args.is_some() || prop.is_click_callback {
            quote! { self.#ident.unwrap_or_default() }
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
    let finish = if base {
        quote! { (move || #block)() }
    } else {
        quote! { ::beui::reactive::component(#name, move || #block) }
    };

    let prop_idents: Vec<_> = props.iter().map(|prop| prop.ident.clone()).collect();

    quote! {
        #(#attrs)*
        #vis struct #builder_ident #generics #where_clause {
            #(#fields,)*
            test_id: Option<String>,
            node_ref: Option<::beui::reactive::NodeRef>,
        }

        impl #generics ::core::default::Default for #builder_ident #generics #where_clause {
            fn default() -> Self {
                Self {
                    #(#prop_idents: ::core::default::Default::default(),)*
                    test_id: None,
                    node_ref: None,
                }
            }
        }

        impl #generics #builder_ident #generics #where_clause {
            #(#setters)*

            pub fn test_id(mut self, value: impl Into<String>) -> Self {
                self.test_id = Some(value.into());
                self
            }

            pub fn node_ref(mut self, value: &::beui::reactive::NodeRef) -> Self {
                self.node_ref = Some(value.clone());
                self
            }

            pub fn build(self) #output {
                let test_id = self.test_id;
                let node_ref = self.node_ref;
                #(#field_lets)*
                let node = #finish;
                if let Some(test_id) = test_id {
                    ::beui::reactive::with_document(|document| document.set_test_id(node, test_id));
                }
                if let Some(node_ref) = node_ref {
                    node_ref.fill(node);
                }
                node
            }
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
    Size(Expr),
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
            "size" => {
                let args;
                parenthesized!(args in input);
                Ok(ViewSizing::Size(args.parse()?))
            }
            other => Err(syn::Error::new(
                ident.span(),
                format!("unknown sizing `@{other}`, expected `@intrinsic`, `@fixed(size)`, `@percent(weight)`, or `@size(item_size)`"),
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
    tag_path: Vec<Ident>,
    props: Vec<ViewAttr>,
    children: Option<Vec<ViewChild>>,
}

fn parse_tag_path(input: ParseStream) -> syn::Result<Vec<Ident>> {
    let mut segments = vec![input.parse::<Ident>()?];
    while input.peek(Token![::]) {
        input.parse::<Token![::]>()?;
        segments.push(input.parse::<Ident>()?);
    }
    Ok(segments)
}

fn tag_path_string(tag_path: &[Ident]) -> String {
    tag_path
        .iter()
        .map(|segment| segment.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

impl Parse for ViewNode {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![<]>()?;
        let tag_path = parse_tag_path(input)?;

        let mut props = Vec::new();
        while !input.peek(Token![>]) && !input.peek(Token![/]) {
            props.push(input.parse::<ViewAttr>()?);
        }

        if input.peek(Token![/]) {
            input.parse::<Token![/]>()?;
            input.parse::<Token![>]>()?;
            return Ok(ViewNode {
                tag_path,
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
        let close_tag_path = parse_tag_path(input)?;
        if tag_path_string(&close_tag_path) != tag_path_string(&tag_path) {
            let close_tag = tag_path_string(&close_tag_path);
            let tag = tag_path_string(&tag_path);
            return Err(syn::Error::new(
                close_tag_path.last().unwrap().span(),
                format!("mismatched closing tag `</{close_tag}>`, expected `</{tag}>`"),
            ));
        }
        input.parse::<Token![>]>()?;

        Ok(ViewNode {
            tag_path,
            props,
            children: Some(children),
        })
    }
}

fn expand_view_node(node: &ViewNode) -> proc_macro2::TokenStream {
    let mut segments = node.tag_path.clone();
    let last = segments.pop().expect("tag path always has one segment");
    let builder_name = format_ident!(
        "{}Builder",
        pascal_case(&last.to_string()),
        span = last.span()
    );
    let builder_path = if segments.is_empty() {
        quote! { #builder_name }
    } else {
        quote! { #(#segments::)* #builder_name }
    };
    let setters = node.props.iter().map(|prop| {
        let key = &prop.key;
        let value = &prop.value;
        quote! { .#key(#value) }
    });

    match &node.children {
        None => quote! { #builder_path::default() #(#setters)* .build() },
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
                    Some(ViewSizing::Size(size)) => {
                        quote! { ::beui::reactive::size(#node, #size) }
                    }
                }
            });
            quote! {
                #builder_path::default() #(#setters)* .children([#(#items),*]) .build()
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
