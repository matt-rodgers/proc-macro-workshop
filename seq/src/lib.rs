use proc_macro::TokenStream;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input, Token,
};

#[derive(Debug)]
struct SeqMacroInput {
    repeat_ident: syn::Ident,
    range: std::ops::Range<usize>,
    content: proc_macro2::TokenStream,
}

#[derive(Debug, Copy, Clone)]
enum Mode {
    /// Scan for a `#(...)*` group without replacing anything
    FindGroup,

    /// Replace the replace_ident with the given number
    Replace(usize),
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

        // Parse the range, which may be inclusive `..=` or exclusive `..`
        let inclusive: bool = if input.peek(Token![..=]) {
            let _: Token![..=] = input.parse()?;
            true
        } else {
            let _: Token![..] = input.parse()?;
            false
        };

        let end_tok = input.parse()?;
        let content;
        let _braces = braced!(content in input);
        let content = proc_macro2::TokenStream::parse(&content)?;

        let start = parse_numeric_lit(start_tok)?;
        let mut end = parse_numeric_lit(end_tok)?;
        if inclusive {
            end += 1;
        }

        Ok(SeqMacroInput {
            repeat_ident,
            range: start..end,
            content,
        })
    }
}

impl SeqMacroInput {
    fn walk_token_stream(
        &self,
        ts: proc_macro2::TokenStream,
        mode: Mode,
        did_mutate: &mut bool,
    ) -> proc_macro2::TokenStream {
        // Create an output token stream that we can extend with new items
        let mut out = proc_macro2::TokenStream::new();

        // Put the TokenTrees into a Vec so that they can be indexed, to look ahead by an arbitrary
        // number of items
        let tts: Vec<proc_macro2::TokenTree> = ts.into_iter().collect();

        // Walk the TokenStream until we reach the end
        let mut i = 0;
        while i < tts.len() {
            let token_out = match (mode, tts.get(i)) {
                (Mode::FindGroup, Some(proc_macro2::TokenTree::Punct(p1)))
                    if p1.as_char() == '#' =>
                {
                    // If we are looking for a group and we get a '#' Punct, look ahead to see if
                    // we have a pattern like:
                    //   #(...)*
                    match (tts.get(i + 1), tts.get(i + 2)) {
                        (
                            Some(proc_macro2::TokenTree::Group(g)),
                            Some(proc_macro2::TokenTree::Punct(p2)),
                        ) if g.delimiter() == proc_macro2::Delimiter::Parenthesis
                            && p2.as_char() == '*' =>
                        {
                            // We found a group that should be expanded.
                            // Expand the group the specified number of times
                            let new_tokens: proc_macro2::TokenStream = self
                                .range
                                .clone()
                                .map(|n| {
                                    self.walk_token_stream(g.stream(), Mode::Replace(n), did_mutate)
                                })
                                .collect();

                            // Advance i past the consumed tokens, and record that group was found
                            i += 2;
                            *did_mutate = true;

                            // We need to return a TokenTree, but we have a TokenStream. Luckily, a
                            // Group can have a delimiter type of 'None', so make a TokenTree which
                            // is a group containing our TokenStream.
                            proc_macro2::TokenTree::Group(proc_macro2::Group::new(
                                proc_macro2::Delimiter::None,
                                new_tokens,
                            ))
                        }
                        _ => proc_macro2::TokenTree::Punct(p1.clone()),
                    }
                }
                (_, Some(proc_macro2::TokenTree::Group(g))) => {
                    // On a group, recursively walk the TokenStream inside the group
                    let new_ts = self.walk_token_stream(g.stream(), mode, did_mutate);
                    let mut new_group = proc_macro2::Group::new(g.delimiter(), new_ts);
                    new_group.set_span(g.span());
                    proc_macro2::TokenTree::Group(new_group)
                }
                (Mode::Replace(n), Some(proc_macro2::TokenTree::Ident(ident)))
                    if ident == &self.repeat_ident =>
                {
                    // On an ident that matches the one we want to replace, replace it
                    let lit = proc_macro2::Literal::usize_unsuffixed(n);
                    proc_macro2::TokenTree::Literal(lit)
                }
                (Mode::Replace(n), Some(proc_macro2::TokenTree::Ident(ident))) => {
                    // On some other ident, we need to check if we have a pattern like:
                    //   ident # self.repeat_ident , OR
                    //   ident # self.repeat_ident # suffix_ident
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
                        ) if p1.as_char() == '~'
                            && replace == &self.repeat_ident
                            && p2.as_char() == '~' =>
                        {
                            // We got the pattern:
                            //   ident # self.repeat_ident # suffix
                            // Combine the tokens and advance i past the consumed tokens
                            i += 4;
                            let name = format!("{}{}{}", ident, n, suffix);
                            proc_macro2::TokenTree::Ident(proc_macro2::Ident::new(
                                &name,
                                ident.span(),
                            ))
                        }
                        (
                            Some(proc_macro2::TokenTree::Punct(p1)),
                            Some(proc_macro2::TokenTree::Ident(replace)),
                            _,
                            _,
                        ) if p1.as_char() == '~' && replace == &self.repeat_ident => {
                            // We got the pattern:
                            //   ident # self.repeat_ident
                            // Combine the tokens and advance i past the consumed tokens
                            i += 2;
                            let name = format!("{}{}", ident, n);
                            proc_macro2::TokenTree::Ident(proc_macro2::Ident::new(
                                &name,
                                ident.span(),
                            ))
                        }
                        _ => {
                            // If we didn't match any of the above patterns, then this is just a
                            // normal ident that can be put into the output TokenStream unchanged.
                            proc_macro2::TokenTree::Ident(ident.clone())
                        }
                    }
                }
                (_, Some(other)) => other.clone(),
                (_, None) => {
                    break;
                }
            };

            i += 1;
            out.extend(std::iter::once(token_out));
        }

        out
    }
}

impl Into<TokenStream> for SeqMacroInput {
    fn into(self) -> TokenStream {
        let mut did_mutate = false;

        // On the first pass, don't initially replace anything unless we find a `#(...)*` group,
        // in which case perform the replacement inside the group only.
        let out: proc_macro2::TokenStream =
            self.walk_token_stream(self.content.clone(), Mode::FindGroup, &mut did_mutate);

        // If we found and replaced a group in the first pass, return the new TokenStream
        if did_mutate {
            return out.into();
        }

        // If we did *not* find and replace a group in the first pass, do a second pass where we
        // replace the entire content
        let out: proc_macro2::TokenStream = self
            .range
            .clone()
            .map(|n| {
                self.walk_token_stream(self.content.clone(), Mode::Replace(n), &mut did_mutate)
            })
            .collect();

        out.into()
    }
}

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SeqMacroInput);
    input.into()
}
