use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// If the type matches 'expected', extract and return the inner type.
/// E.g. can be used to extract what's inside an Option<T> or a Vec<T>
fn extract_inner_type<'a, 'b>(
    ty: &'a syn::Type,
    expected: &'b str,
) -> std::option::Option<&'a syn::Type> {
    if let syn::Type::Path(ref p) = ty {
        // p is an &syn::TypePath
        if p.path.segments.len() != 1 {
            // The path segments contain colon separated parts of the path, for example:
            //   std::option::Option would have three segments separated by double colons.
            // Since it's impractical to exhaustively compare against every possible way to specify
            // a type, we choose to only work on types with a single segment.
            return None;
        }

        if p.path.segments[0].ident != expected {
            return None;
        }

        if let syn::PathArguments::AngleBracketed(ref inner) = p.path.segments[0].arguments {
            // inner gives us the T, if the outer is e.g. Option<T>
            // We only want to work for things that have a single generic argument, not something
            // like Vec<T, A>.
            if inner.args.len() != 1 {
                return None;
            }

            let inner_ty = inner.args.first().unwrap();
            if let syn::GenericArgument::Type(t) = inner_ty {
                return Some(&t);
            }
        }
    }

    None
}

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

        // If it's already wrapped in an option, we don't need to wrap again
        if let Some(_) = extract_inner_type(ty, "Option") {
            quote! { #name: #ty}
        } else {
            quote! { #name: std::option::Option<#ty> }
        }
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

        // If the field is an option, the function should set the inner value
        let arg_ty = if let Some(inner) = extract_inner_type(ty, "Option") {
            inner
        } else {
            ty
        };

        quote! {
            pub fn #name(&mut self, #name: #arg_ty) -> &mut Self {
                self.#name = Some(#name);
                self
            }
        }
    });

    // For each field in the builder, set the appropriate field in the original struct
    let set_fields = data.fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;

        if let Some(_) = extract_inner_type(ty, "Option") {
            quote! { #name: self.#name.clone() }
        } else {
            let err_msg = format!("Field '{}' is not set", name.as_ref().unwrap());
            quote! { #name: self.#name.clone().ok_or_else(|| #err_msg.to_string())? }
        }
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
