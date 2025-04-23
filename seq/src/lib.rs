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
    // Create an output token stream that we can extend with new items
    let mut out = proc_macro2::TokenStream::new();

    // Put the TokenTrees into a Vec so that they can be indexed, to look ahead by an arbitrary
    // number of items
    let tts: Vec<proc_macro2::TokenTree> = ts.into_iter().collect();

    // Walk the TokenStream until we reach the end
    let mut i = 0;
    while i < tts.len() {
        let token_out = match tts.get(i) {
            Some(proc_macro2::TokenTree::Group(g)) => {
                // On a group, recursively walk the TokenStream inside the group
                let new_ts = walk_token_stream(g.stream(), replace_ident, n);
                let mut new_group = proc_macro2::Group::new(g.delimiter(), new_ts);
                new_group.set_span(g.span());
                proc_macro2::TokenTree::Group(new_group)
            }
            Some(proc_macro2::TokenTree::Ident(ident)) if ident == replace_ident => {
                // On an ident that matches the one we want to replace, replace it
                let lit = proc_macro2::Literal::usize_unsuffixed(n);
                proc_macro2::TokenTree::Literal(lit)
            }
            Some(proc_macro2::TokenTree::Ident(ident)) => {
                // On some other ident, we need to check if we have a pattern like:
                //   ident # replace_ident , OR
                //   ident # replace_ident # suffix_ident
                // To do this, we must look ahead by a few tokens
                match (
                    tts.get(i + 1),
                    tts.get(i + 2),
                    tts.get(i + 3),
                    tts.get(i + 4),
                ) {
                    (
                        Some(proc_macro2::TokenTree::Punct(p1)),
                        Some(proc_macro2::TokenTree::Ident(replace)),
                        Some(proc_macro2::TokenTree::Punct(p2)),
                        Some(proc_macro2::TokenTree::Ident(suffix)),
                    ) if p1.as_char() == '~' && replace == replace_ident && p2.as_char() == '~' => {
                        // We got the pattern:
                        //   ident # replace_ident # suffix
                        // Combine the tokens and advance i past the consumed tokens
                        i += 4;
                        let name = format!("{}{}{}", ident, n, suffix);
                        proc_macro2::TokenTree::Ident(proc_macro2::Ident::new(&name, ident.span()))
                    }
                    (
                        Some(proc_macro2::TokenTree::Punct(p1)),
                        Some(proc_macro2::TokenTree::Ident(replace)),
                        _,
                        _,
                    ) if p1.as_char() == '~' && replace == replace_ident => {
                        // We got the pattern:
                        //   ident # replace_ident
                        // Combine the tokens and advance i past the consumed tokens
                        i += 2;
                        let name = format!("{}{}", ident, n);
                        proc_macro2::TokenTree::Ident(proc_macro2::Ident::new(&name, ident.span()))
                    }
                    _ => {
                        // If we didn't match any of the above patterns, then this is just a normal
                        // ident that can be put into the output TokenStream unchanged.
                        proc_macro2::TokenTree::Ident(ident.clone())
                    }
                }
            }
            Some(other) => other.clone(),
            None => {
                break;
            }
        };

        i += 1;
        out.extend(std::iter::once(token_out));
    }

    out
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
