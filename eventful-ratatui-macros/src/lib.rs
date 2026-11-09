//! Procedural macros for eventful-ratatui. Use them through the
//! `eventful-ratatui` crate, which re-exports them.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod chord;

use chord::Chord;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemTrait, LitStr, Token, TraitItem, punctuated::Punctuated};

/// Bind key chords to events: `#[key_events]` on a trait whose methods carry
/// `#[key("j", "down")]`.
///
/// The trait becomes an `#[eventful_rs::events]` interface. A `{Trait}Bridge`
/// source is generated next to it, with:
///
/// - `new()`, also available as `Default`;
/// - `dispatch(&InputEvent) -> Handled`, which matches key presses and
///   repeats synchronously and queues the bound event on a hit;
/// - `connect_to(&receiver)`, which connects every event, plus the usual
///   signal accessors and `HasEvents`;
/// - `BINDINGS`, each chord with the name of its event, in declaration order,
///   for help lines.
///
/// Chords are checked at compile time and use the syntax of `KeyChord`.
/// Methods take only `&self` and return nothing. A chord can be bound only
/// once per trait.
#[proc_macro_attribute]
pub fn key_events(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        let attr = proc_macro2::TokenStream::from(attr);
        return syn::Error::new_spanned(attr, "#[key_events] takes no arguments")
            .to_compile_error()
            .into();
    }
    let interface = syn::parse_macro_input!(item as ItemTrait);
    match expand(interface) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Names the bridge defines itself; events with these names would be
/// shadowed or clash.
const RESERVED: [&str; 4] = ["new", "dispatch", "connect_to", "events"];

/// One bound event.
struct Binding {
    method: syn::Ident,
    chords: Vec<(Chord, LitStr)>,
}

fn expand(mut interface: ItemTrait) -> syn::Result<proc_macro2::TokenStream> {
    let runtime = runtime_path();
    let tui = tui_path();
    let private = quote!(#tui::__private);
    let mut bindings = Vec::new();
    let mut errors: Option<syn::Error> = None;
    let mut fail = |error: syn::Error| match &mut errors {
        Some(errors) => errors.combine(error),
        None => errors = Some(error),
    };

    for item in &mut interface.items {
        let TraitItem::Fn(method) = item else {
            continue;
        };
        let mut chords = Vec::new();
        let mut has_key = false;
        let mut keep = Vec::new();
        for attr in method.attrs.drain(..) {
            if attr.path().is_ident("key") {
                has_key = true;
                match attr.parse_args_with(Punctuated::<LitStr, Token![,]>::parse_terminated) {
                    Ok(list) if list.is_empty() => fail(syn::Error::new_spanned(
                        &attr,
                        "#[key] needs at least one chord, like #[key(\"j\")]",
                    )),
                    Ok(list) => {
                        for lit in list {
                            match Chord::parse(&lit.value()) {
                                Ok(chord) => chords.push((chord, lit)),
                                Err(reason) => fail(syn::Error::new_spanned(
                                    &lit,
                                    format!("invalid key chord `{}`: {reason}", lit.value()),
                                )),
                            }
                        }
                    }
                    Err(error) => fail(error),
                }
            } else {
                if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
                    fail(syn::Error::new_spanned(
                        &attr,
                        "key events do not support #[cfg]; put the whole trait behind it",
                    ));
                }
                keep.push(attr);
            }
        }
        method.attrs = keep;
        let sig = &method.sig;
        if !has_key {
            fail(syn::Error::new_spanned(
                &sig.ident,
                "key event without a chord; add #[key(\"...\")]",
            ));
        }
        for input in sig.inputs.iter().skip(1) {
            fail(syn::Error::new_spanned(
                input,
                "key events take no arguments",
            ));
        }
        if !matches!(sig.inputs.first(), Some(FnArg::Receiver(_))) {
            fail(syn::Error::new_spanned(sig, "key events take `&self`"));
        }
        if !matches!(sig.output, syn::ReturnType::Default) {
            fail(syn::Error::new_spanned(
                &sig.output,
                "key events return nothing",
            ));
        }
        if RESERVED.iter().any(|name| sig.ident == name) {
            fail(syn::Error::new_spanned(
                &sig.ident,
                format!(
                    "`{}` is reserved by the generated bridge; rename the event",
                    sig.ident
                ),
            ));
        }
        bindings.push(Binding {
            method: sig.ident.clone(),
            chords,
        });
    }

    // Each chord may trigger one event only.
    let mut seen: Vec<(Chord, &syn::Ident)> = Vec::new();
    for binding in &bindings {
        for (chord, lit) in &binding.chords {
            if let Some((_, other)) = seen.iter().find(|(c, _)| c == chord) {
                fail(syn::Error::new_spanned(
                    lit,
                    format!("`{}` is already bound to `{other}`", lit.value()),
                ));
            } else {
                seen.push((*chord, &binding.method));
            }
        }
    }
    if let Some(errors) = errors {
        return Err(errors);
    }

    let name = &interface.ident;
    let vis = &interface.vis;
    let bridge = format_ident!("{}Bridge", name);
    let set = format_ident!("{}EventSet", name);
    let emissions = format_ident!("{}Emissions", name);
    let signals_ext = format_ident!("{}SignalsExt", name);
    let entries = bindings.iter().flat_map(|binding| {
        let method = binding.method.to_string();
        let method = method.strip_prefix("r#").unwrap_or(&method).to_owned();
        let private = &private;
        binding.chords.iter().map(move |(chord, _)| {
            let chord = chord.to_tokens(private);
            quote!((#chord, #method))
        })
    });
    let mut index = 0usize;
    let arms = bindings.iter().map(|binding| {
        let method = &binding.method;
        let indices = (index..index + binding.chords.len()).collect::<Vec<_>>();
        index += binding.chords.len();
        quote! {
            if #(chord == Self::BINDINGS[#indices].0)||* {
                self.events.#method().emit();
                return #tui::Handled::Yes;
            }
        }
    });
    let doc =
        format!("Key input source for [`{name}`]: matches chords and emits the bound events.");

    Ok(quote! {
        #[#runtime::events]
        #interface

        #[doc = #doc]
        ///
        /// Store it in the component, call `dispatch` from `on_input`, and
        /// connect it to the receiver with `connect_to`.
        #[derive(::core::fmt::Debug, ::core::default::Default)]
        #vis struct #bridge {
            events: #emissions,
        }

        impl #bridge {
            /// Every chord with the name of the event it emits, in
            /// declaration order.
            #vis const BINDINGS: &'static [(#tui::KeyChord, &'static str)] = &[#(#entries),*];

            /// A bridge with no receivers connected.
            #vis fn new() -> Self {
                ::core::default::Default::default()
            }

            /// Emit the event bound to `event`'s chord, if any. Matches key
            /// presses and repeats, not releases. Returns synchronously whether
            /// a chord matched; delivery to receivers is queued.
            #vis fn dispatch(&self, event: &#private::Event) -> #tui::Handled {
                let ::core::option::Option::Some(chord) = #private::pressed(event) else {
                    return #tui::Handled::No;
                };
                #(#arms)*
                #tui::Handled::No
            }

            /// Connect every event to `receiver`, which is held weakly.
            /// Dropping the returned group does not disconnect; use `scoped()`.
            #vis fn connect_to<T, S>(&self, receiver: &S) -> #runtime::ConnectionGroup
            where
                T: #runtime::Eventful + #runtime::HasEvents<T::EventSetType> + 'static,
                S: #runtime::Sharded<T>,
                #set: #runtime::ConnectEvents<T>,
            {
                #runtime::ConnectEvents::connect_events(&**self.events.signals(), receiver)
            }
        }

        impl #signals_ext for #bridge {}

        impl #runtime::HasEvents<#set> for #bridge {
            fn events(&self) -> &::std::sync::Arc<#set> {
                self.events.signals()
            }
        }
    })
}

/// The eventful-rs crate as the user named it.
fn runtime_path() -> proc_macro2::TokenStream {
    match proc_macro_crate::crate_name("eventful-rs") {
        Ok(proc_macro_crate::FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name);
            quote!(::#ident)
        }
        _ => quote!(::eventful_rs),
    }
}

/// The eventful-ratatui crate as the user named it. Inside eventful-ratatui
/// itself, `::eventful_ratatui` resolves through `extern crate self`.
fn tui_path() -> proc_macro2::TokenStream {
    match proc_macro_crate::crate_name("eventful-ratatui") {
        Ok(proc_macro_crate::FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name);
            quote!(::#ident)
        }
        _ => quote!(::eventful_ratatui),
    }
}
