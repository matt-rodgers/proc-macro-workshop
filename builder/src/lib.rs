use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // The identifier of the top level struct we're operating on
    let struct_ident = input.ident;
    let builder_ident = Ident::new(&format!("{}Builder", struct_ident), struct_ident.span());

    let data = if let Data::Struct(data) = input.data {
        data
    } else {
        panic!("Expected Data::Struct")
    };

    // Replace each struct field with the same type wrapped in an Option
    let struct_fields = data.fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! { #name: Option<#ty> }
    });

    let builder_tokens = quote! {
        pub struct #builder_ident {
            #(#struct_fields,)*
        }
    };

    let struct_initialisers = data.fields.iter().map(|f| {
        let name = &f.ident;
        quote! { #name: None }
    });

    let builder_impl = quote! {
        impl #struct_ident {
            pub fn builder() -> #builder_ident {
                #builder_ident {
                    #(#struct_initialisers,)*
                }
            }
        }
    };

    quote! {
        #builder_tokens
        #builder_impl
    }
    .into()
}
