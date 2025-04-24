use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, Result};

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as syn::Item);

    sorted_inner(args, item)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn sorted_inner(args: TokenStream, item: syn::Item) -> Result<proc_macro2::TokenStream> {
    // We don't expect any args for now
    assert!(args.is_empty());

    let enum_item = match item {
        syn::Item::Enum(en) => en,
        _ => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "expected enum or match expression",
            ));
        }
    };

    let mut names: Vec<String> = Vec::new();
    for variant in enum_item.variants.iter() {
        let name = variant.ident.to_string();
        if let Some(last) = names.last() {
            if &name < last {
                // We are out of order, find where the variant *should* be placed
                match names.binary_search(&name) {
                    Ok(_) => {
                        return Err(syn::Error::new_spanned(variant, "duplicate enum variant"));
                    }
                    Err(n) => {
                        return Err(syn::Error::new_spanned(
                            variant,
                            format!("{} should sort before {}", name, names[n]),
                        ));
                    }
                }
            }
        }

        names.push(name);
    }

    Ok(enum_item.to_token_stream())
}
