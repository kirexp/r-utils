use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(NamedStateMachine)]
pub fn type_name_derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    // Get the name of the struct
    let name = input.ident;

    // Generate the implementation of the TypeName trait
    let expanded = quote! {
        impl GetStateMachineName for #name {
            fn get_name(&self) -> &'static str {
                stringify!(#name)
            }
            fn get_name_st() -> &'static str {
                stringify!(#name)
            }
        }
    };

    // Convert into a TokenStream and return it
    TokenStream::from(expanded)
}
