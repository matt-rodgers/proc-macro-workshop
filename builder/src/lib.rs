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

    let builder_struct = quote! {
        pub struct #builder_ident {
            #(#struct_fields,)*
        }
    };

    // Initialise each struct field to None
    let struct_initialisers = data.fields.iter().map(|f| {
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

    // Create build method to turn the Builder into the original struct
    let set_fields = data.fields.iter().map(|f| {
        let name = &f.ident;
        let err_msg = format!("Field '{}' is not set", name.as_ref().unwrap());
        quote! { #name: self.#name.ok_or_else(|| #err_msg.to_string())? }
    });

    let build_method = quote! {
        pub fn build(mut self) -> Result<#struct_ident, Box<dyn std::error::Error>> {
            Ok(#struct_ident {
                #(#set_fields,)*
            })
        }
    };

    let builder_impl = quote! {
        impl #builder_ident {
            #(#builder_methods)*
            #build_method
        }
    };

    // Create a builder() method on the original struct
    let struct_impl = quote! {
        impl #struct_ident {
            pub fn builder() -> #builder_ident {
                #builder_ident {
                    #(#struct_initialisers,)*
                }
            }
        }
    };

    quote! {
        #struct_impl
        #builder_struct
        #builder_impl
    }
    .into()
}
