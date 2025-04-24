use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Result};

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as syn::Item);

    sorted_inner(args, item)
        .unwrap_or_else(|err| err.to_compile_error().into())
        .into()
}

fn sorted_inner(args: TokenStream, item: syn::Item) -> Result<proc_macro2::TokenStream> {
    let _ = args;

    let enum_item = match item {
        syn::Item::Enum(en) => en,
        _ => {
            return Ok(quote! {
                compile_error!("expected enum or match expression");
            })
        }
    };

    eprintln!("{:#?}", enum_item);

    Ok(enum_item.to_token_stream())
}
