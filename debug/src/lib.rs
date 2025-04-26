use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

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

    let field_calls = fields.iter().map(|f| {
        if let Some(ref id) = f.ident {
            let mut custom_format = None;

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

    quote! {
        impl ::std::fmt::Debug for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.debug_struct(#name_str)
                    #(#field_calls)*
                    .finish()
            }
        }
    }
    .into()
}
