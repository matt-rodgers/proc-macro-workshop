use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(CustomDebug)]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = format!("{}", name);

    let field_idents = match input.data {
        syn::Data::Struct(ref s) => s.fields.iter().filter_map(|f| f.ident.as_ref()),
        _ => todo!(),
    };

    let field_calls = field_idents.map(|id| {
        let id_str = format!("{}", id);
        quote! {
            .field(#id_str, &self.#id)
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
