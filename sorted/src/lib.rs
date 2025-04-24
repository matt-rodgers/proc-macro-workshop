use proc_macro::TokenStream;
use syn::{parse_macro_input, Result};

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut out = input.clone();

    let item = parse_macro_input!(input as syn::Item);

    if let Err(e) = sorted_inner(args, item) {
        let error_tokens: TokenStream = e.to_compile_error().into();
        out.extend(error_tokens);
    }

    out
}

fn sorted_inner(args: TokenStream, item: syn::Item) -> Result<()> {
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
                            variant.ident.clone(),
                            format!("{} should sort before {}", name, names[n]),
                        ));
                    }
                }
            }
        }

        names.push(name);
    }

    Ok(())
}
