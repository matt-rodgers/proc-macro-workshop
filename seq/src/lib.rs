use proc_macro::TokenStream;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input, Token,
};

#[derive(Debug)]
struct SeqMacroInput {
    repeat_ident: syn::Ident,
    start: syn::Lit,
    end: syn::Lit,
    braces: syn::token::Brace,
    content: proc_macro2::TokenStream,
}

impl Parse for SeqMacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse each expected element out of the input stream in turn, throwing away the ones that
        // we won't need later
        let repeat_ident = input.parse()?;
        let _in: Token![in] = input.parse()?;
        let start = input.parse()?;
        let _dotdot: Token![..] = input.parse()?;
        let end = input.parse()?;
        let content;
        let braces = braced!(content in input);
        let content = proc_macro2::TokenStream::parse(&content)?;

        Ok(SeqMacroInput {
            repeat_ident,
            start,
            end,
            braces,
            content,
        })
    }
}

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SeqMacroInput);
    eprintln!("{:#?}", input);

    TokenStream::new()
}
