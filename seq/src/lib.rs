use proc_macro::TokenStream;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input, Token,
};

#[derive(Debug)]
struct SeqMacroInput {
    repeat_ident: syn::Ident,
    start: usize,
    end: usize,
    content: proc_macro2::TokenStream,
}

fn parse_numeric_lit(tok: syn::Lit) -> syn::Result<usize> {
    match tok {
        syn::Lit::Int(n) => n.base10_parse(),
        _ => Err(syn::Error::new_spanned(
            tok,
            "Token must be parseable as `usize`",
        )),
    }
}

impl Parse for SeqMacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse each expected element out of the input stream in turn, throwing away the ones that
        // we won't need later
        let repeat_ident = input.parse()?;
        let _in: Token![in] = input.parse()?;
        let start_tok = input.parse()?;
        let _dotdot: Token![..] = input.parse()?;
        let end_tok = input.parse()?;
        let content;
        let _braces = braced!(content in input);
        let content = proc_macro2::TokenStream::parse(&content)?;

        let start = parse_numeric_lit(start_tok)?;
        let end = parse_numeric_lit(end_tok)?;

        Ok(SeqMacroInput {
            repeat_ident,
            start,
            end,
            content,
        })
    }
}

fn walk_token_stream(
    ts: proc_macro2::TokenStream,
    replace_ident: &syn::Ident,
    n: usize,
) -> proc_macro2::TokenStream {
    ts.into_iter()
        .map(|tt| match tt {
            proc_macro2::TokenTree::Group(g) => {
                let new_ts = walk_token_stream(g.stream(), replace_ident, n);
                let mut new_group = proc_macro2::Group::new(g.delimiter(), new_ts);
                new_group.set_span(g.span());
                proc_macro2::TokenTree::Group(new_group)
            }
            proc_macro2::TokenTree::Ident(ident) if ident == *replace_ident => {
                let lit = proc_macro2::Literal::usize_unsuffixed(n);
                proc_macro2::TokenTree::Literal(lit)
            }
            other => other,
        })
        .collect()
}

impl Into<TokenStream> for SeqMacroInput {
    fn into(self) -> TokenStream {
        let range = self.start..self.end;

        let out: proc_macro2::TokenStream = range
            .map(|n| walk_token_stream(self.content.clone(), &self.repeat_ident, n))
            .collect();

        out.into()
    }
}

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SeqMacroInput);
    input.into()
}
