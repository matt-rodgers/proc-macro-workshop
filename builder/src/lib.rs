use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder, attributes(builder))]
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

    // Create the fields of the builder struct
    let builder_fields = data.fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;

        if let Some(_) = get_extend_ident(&f) {
            quote! { #name: #ty }
        } else if let Some(_) = extract_inner_type(ty, "Option") {
            quote! { #name: #ty}
        } else {
            quote! { #name: std::option::Option<#ty> }
        }
    });

    // Initialise each struct field
    let builder_initialisers = data.fields.iter().map(|f| {
        let name = &f.ident;

        match get_extend_ident(&f) {
            Some(_) => quote! { #name: Vec::new() },
            _ => quote! { #name: None },
        }
    });

    // Create a method to set each struct field
    let builder_methods = data.fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;

        if let Some((extend_ident, inner_ty)) = get_extend_ident(&f) {
            // If the field is extendable, the function should push values to the Vec<T>
            quote! {
                pub fn #extend_ident(&mut self, #extend_ident: #inner_ty) -> &mut Self {
                    self.#name.push(#extend_ident);
                    self
                }
            }
        } else if let Some(inner) = extract_inner_type(ty, "Option") {
            // If the field was originally an Option, the function should accept the inner type and
            // store in an Option
            quote! {
                pub fn #name(&mut self, #name: #inner) -> &mut Self {
                    self.#name = Some(#name);
                    self
                }
            }
        } else {
            // Otherwise, the function should accept the original type, and store in an Option
            quote! {
                pub fn #name(&mut self, #name: #ty) -> &mut Self {
                    self.#name = Some(#name);
                    self
                }
            }
        }
    });

    // For each field in the builder, set the appropriate field in the original struct
    let set_fields = data.fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;

        if let Some(_) = get_extend_ident(&f) {
            quote! { #name: self.#name.clone() }
        } else if let Some(_) = extract_inner_type(ty, "Option") {
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

/// If a field has an attribute matching the pattern:
///   #[builder(each = "name")]
/// Then confirm that the type is a Vec<T>, and return Some(name, inner_ty).
/// Otherwise return None.
/// panics on unrecognised attributes.
fn get_extend_ident(f: &syn::Field) -> std::option::Option<(syn::Ident, syn::Type)> {
    for attr in f.attrs.iter() {
        match attr.meta {
            syn::Meta::List(ref l) => {
                if l.path.segments[0].ident != "builder" {
                    panic!("Expected 'builder' attribute");
                }

                let mut ti = l.tokens.clone().into_iter();

                match ti.next() {
                    Some(proc_macro2::TokenTree::Ident(i)) => {
                        assert_eq!(i, "each");
                    }
                    _ => {
                        panic!("Expected Ident(each)");
                    }
                }

                match ti.next() {
                    Some(proc_macro2::TokenTree::Punct(p)) => {
                        assert_eq!(p.as_char(), '=');
                    }
                    _ => {
                        panic!("Expected Punct('=')")
                    }
                }

                match ti.next() {
                    Some(proc_macro2::TokenTree::Literal(lit)) => match syn::Lit::new(lit) {
                        syn::Lit::Str(s) => match extract_inner_type(&f.ty, "Vec") {
                            Some(inner) => {
                                let ident = syn::Ident::new(&s.value(), s.span());
                                return Some((ident, inner.clone()));
                            }
                            _ => {
                                panic!("It is not valid to have a #[builder(each = ...)] attribute on a field which is not a Vec");
                            }
                        },
                        _ => {
                            panic!("Expected string literal");
                        }
                    },
                    _ => {
                        panic!("Expected literal");
                    }
                }
            }
            _ => {
                panic!("Expected Meta::List attr");
            }
        }
    }

    None
}
