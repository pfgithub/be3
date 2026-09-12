mod computation;
mod memo;
mod runtime;
mod scope;
mod selector;
mod signal;

pub use computation::{create_effect, Effect};
pub use memo::{create_memo, Memo};
pub use runtime::{batch, settle, untrack};
pub use scope::{on_cleanup, owner_scope, provide_context, use_context, Scope, ScopeContext};
pub use selector::{create_selector, Selector};
pub use signal::{create_signal, ReadSignal, WriteSignal};

#[macro_export]
macro_rules! clone {
    ($($name:ident)* -> $body:expr) => {{
        $(let $name = $name.clone();)*
        $body
    }};
}

#[cfg(test)]
mod tests;
