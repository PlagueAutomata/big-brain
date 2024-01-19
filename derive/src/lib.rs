//! Big Brain Derive
//! Procedural macros to simplify the implementation of Big Brain traits

use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derives ActionSpawn for a struct that implements Component + Clone
#[proc_macro_derive(ActionSpawn)]
pub fn action_builder_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let component_name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let turbofish = ty_generics.as_turbofish();

    let gen = quote! {
        impl #impl_generics ::big_brain::ActionSpawn for #component_name #ty_generics #where_clause {
            fn spawn(&self, mut cmd: ::big_brain::ActionCommands) -> ::big_brain::Action {
                cmd.spawn(#component_name #turbofish ::clone(self))
            }
        }
    };

    proc_macro::TokenStream::from(gen)
}

/// Derives ScorerSpawn for a struct that implements Component + Clone
#[proc_macro_derive(ScorerSpawn)]
pub fn scorer_builder_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let component_name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let turbofish = ty_generics.as_turbofish();

    let gen = quote! {
        impl #impl_generics ::big_brain::ScorerSpawn for #component_name #ty_generics #where_clause {
            fn spawn(&self, mut cmd: ::big_brain::ScorerCommands) -> ::big_brain::Scorer {
                cmd.spawn(#component_name #turbofish ::clone(self))
            }
        }
    };

    proc_macro::TokenStream::from(gen)
}
