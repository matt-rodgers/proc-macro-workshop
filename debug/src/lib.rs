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

    // Find all of the field types which are contained in a PhantomData<T>
    let phantom: Vec<_> = fields
        .iter()
        .filter_map(|f| {
            if let syn::Type::Path(ref p) = f.ty {
                for seg in p.path.segments.iter() {
                    if seg.ident == "PhantomData" {
                        if let syn::PathArguments::AngleBracketed(
                            syn::AngleBracketedGenericArguments { ref args, .. },
                        ) = seg.arguments
                        {
                            for arg in args.iter() {
                                if let syn::GenericArgument::Type(syn::Type::Path(ref ty)) = arg {
                                    assert!(ty.path.segments.len() == 1);
                                    return Some(&ty.path.segments[0].ident);
                                }
                            }
                        }
                    }
                }
            }
            None
        })
        .collect();

    // Ensure that every type generic implements Debug
    let mut generics = input.generics.clone();
    for param in &mut generics.params {
        if let syn::GenericParam::Type(ref mut type_param) = *param {
            // The exception is if the type in inside a PhantomData, in which case it need not
            // implement debug
            if !phantom.iter().any(|ty| **ty == type_param.ident) {
                type_param.bounds.push(parse_quote!(::std::fmt::Debug));
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

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
