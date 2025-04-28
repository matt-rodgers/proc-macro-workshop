use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, DeriveInput};

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = format!("{}", name);

    let fields = match input.data {
        syn::Data::Struct(ref s) => &s.fields,
        _ => {
            let err =
                syn::Error::new_spanned(name, "CustomDebug is only implemented for DataStructs");
            return err.to_compile_error().into();
        }
    };

    // Generate the .field(...) method calls for each field of the input struct
    let field_calls = fields.iter().map(|f| {
        if let Some(ref id) = f.ident {
            let mut custom_format = None;

            // Find a #[debug = "format"] attribute and store the format string
            for attr in f.attrs.iter() {
                if let syn::Meta::NameValue(ref nv) = attr.meta {
                    if nv.path.segments.len() == 1 && nv.path.segments[0].ident == "debug" {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(ref ls),
                            ..
                        }) = nv.value
                        {
                            custom_format = Some(ls.value());
                        }
                    }
                }
            }

            let id_str = id.to_string();

            if let Some(custom) = custom_format {
                quote! {
                    .field(#id_str, &format_args!(#custom, &self.#id))
                }
            } else {
                quote! {
                    .field(#id_str, &self.#id)
                }
            }
        } else {
            quote! {}
        }
    });

    let mut generics = input.generics.clone();
    let mut associated_types = Vec::new();

    // Check if a `#[debug(bound = "...")]` attribute is present on the top level struct
    let mut manual_bounds = None;
    for attr in input.attrs.iter() {
        if attr.path().is_ident("debug") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("bound") {
                    let val = meta.value()?;
                    let s: syn::LitStr = val.parse()?;
                    manual_bounds = Some(s);
                }
                Ok(())
            })
            .expect("Failed parsing `#[debug(bound = \"...\")]`");
        }
    }

    if manual_bounds.is_none() {
        // Ensure that the required type generics implement Debug.
        // A type need not implement Debug if it only appears:
        // - inside a PhantomData<T>, or
        // - as part of an associated type, in which case the associated type must implement Debug
        for param in &mut generics.params {
            if let syn::GenericParam::Type(ref mut type_param) = *param {
                if fields.iter().any(|f| {
                    match find_type_path_with_ident(&f.ty, &type_param.ident) {
                        None => {
                            // This field doesn't contain the ident we're currently considering
                            return false;
                        }
                        Some(p) => {
                            if p.path.segments.len() > 1
                                && p.path.segments[0].ident == type_param.ident
                            {
                                associated_types.push(syn::Type::Path(p.clone()));
                                // The ident is only present as part of an associated type
                                return false;
                            }
                        }
                    }

                    if is_phantom_data(&f.ty, &type_param.ident) {
                        return false;
                    }

                    // By default a type must implement Debug, if none of the other conditions were met
                    true
                }) {
                    type_param.bounds.push(parse_quote!(::std::fmt::Debug));
                }
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate or manipulate the where clause
    let where_clause = if let Some(manual) = manual_bounds {
        let tokens: proc_macro2::TokenStream = manual
            .value()
            .parse()
            .expect("Given bounds could not be parsed as TokenStream");

        parse_quote! { where #tokens }
    } else if associated_types.len() > 0 {
        let mut new_where_clause = where_clause
            .cloned()
            .unwrap_or_else(|| parse_quote! { where });

        new_where_clause
            .predicates
            .extend(associated_types.iter().map(|ty| -> syn::WherePredicate {
                parse_quote! { #ty: ::std::fmt::Debug }
            }));

        Some(new_where_clause)
    } else {
        where_clause.cloned()
    };

    quote! {
        impl #impl_generics ::std::fmt::Debug for #name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.debug_struct(#name_str)
                    #(#field_calls)*
                    .finish()
            }
        }
    }
    .into()
}

fn is_phantom_data(ty: &syn::Type, ident: &syn::Ident) -> bool {
    if let syn::Type::Path(ref p) = ty {
        for seg in p.path.segments.iter() {
            if seg.ident == "PhantomData" {
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                    ref args,
                    ..
                }) = seg.arguments
                {
                    assert!(args.len() == 1);
                    let arg = args.first().unwrap();
                    if let syn::GenericArgument::Type(syn::Type::Path(ref inner_ty)) = arg {
                        if inner_ty.path.segments.iter().any(|seg| &seg.ident == ident) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Search for an appearance of the given ident within a (potentially nested) type
/// If found, return the whole type path that contains it.
/// For example:
///   ty: Vec<T>, ident: T --> Some(T)
///   ty: Vec<T::Value>, ident: T --> Some(T::Value)
fn find_type_path_with_ident<'a>(
    ty: &'a syn::Type,
    ident: &syn::Ident,
) -> Option<&'a syn::TypePath> {
    if let syn::Type::Path(ref p) = ty {
        if let Some(ref seg) = p.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                ref args,
                ..
            }) = seg.arguments
            {
                for arg in args {
                    if let syn::GenericArgument::Type(ref inner_ty) = arg {
                        let inner_is_assoc = find_type_path_with_ident(inner_ty, ident);
                        if inner_is_assoc.is_some() {
                            return inner_is_assoc;
                        }
                    }
                }
            }
        }

        if p.path.segments.iter().any(|seg| &seg.ident == ident) {
            return Some(p);
        }
    }

    None
}
