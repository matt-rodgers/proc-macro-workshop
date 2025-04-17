use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // The identifier of the top level struct we're operating on
    let struct_ident = input.ident;
    let builder_ident = syn::Ident::new(&format!("{}Builder", struct_ident), struct_ident.span());

    let data = if let syn::Data::Struct(data) = input.data {
        data
    } else {
        panic!("Expected Data::Struct")
    };

    // Replace each struct field with the same type wrapped in an Option
    let builder_fields = data.fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! { #name: Option<#ty> }
    });

    // Initialise each struct field to None
    let builder_initialisers = data.fields.iter().map(|f| {
        let name = &f.ident;
        quote! { #name: None }
    });

    // Create a method to set each struct field
    let builder_methods = data.fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! {
            pub fn #name(&mut self, #name: #ty) -> &mut Self {
                self.#name = Some(#name);
                self
            }
        }
    });

    // For each field in the builder, set the appropriate field in the original struct
    let set_fields = data.fields.iter().map(|f| {
        let name = &f.ident;
        let err_msg = format!("Field '{}' is not set", name.as_ref().unwrap());
        quote! { #name: self.#name.clone().ok_or_else(|| #err_msg.to_string())? }
    });

    quote! {
        impl #struct_ident {
            pub fn builder() -> #builder_ident {
                #builder_ident {
                    #(#builder_initialisers,)*
                }
            }
        }

        pub struct #builder_ident {
            #(#builder_fields,)*
        }

        impl #builder_ident {
            #(#builder_methods)*

            pub fn build(&mut self) -> Result<#struct_ident, Box<dyn std::error::Error>> {
                Ok(#struct_ident {
                    #(#set_fields,)*
                })
            }
        }
    }
    .into()
}
