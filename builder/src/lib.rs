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

        if let Ok(Some(_)) = get_extend_ident(&f) {
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
            Ok(Some(_)) => quote! { #name: Vec::new() },
            _ => quote! { #name: None },
        }
    });

    // Create a method to set each struct field
    let builder_methods = data.fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;

        let extend = match get_extend_ident(&f) {
            Ok(ext) => ext,
            Err(e) => {
                return e.to_compile_error();
            }
        };

        if let Some((extend_ident, inner_ty)) = extend {
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

        if let Ok(Some(_)) = get_extend_ident(&f) {
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
fn get_extend_ident(f: &syn::Field) -> Result<Option<(syn::Ident, syn::Type)>, syn::Error> {
    for attr in f.attrs.iter() {
        if attr.path().is_ident("builder") {
            let mut each = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("each") {
                    let val = meta.value()?;
                    let s: syn::LitStr = val.parse()?;
                    each = Some(s);
                    Ok(())
                } else {
                    Err(syn::Error::new_spanned(
                        attr,
                        "expected `builder(each = \"...\")`",
                    ))
                }
            })?;

            let each = each.unwrap(); // Already returned an error if no correct attribute

            let inner = extract_inner_type(&f.ty, "Vec").unwrap_or_else(|| {
                panic!("Type must be Vec<T> for any field with an 'each' attribute");
            });

            let ident = syn::Ident::new(&each.value(), each.span());
            return Ok(Some((ident, inner.clone())));
        } else {
            return Err(syn::Error::new_spanned(
                attr,
                "expected `builder(each = \"...\")`",
            ));
        }
    }

    Ok(None)
}
