use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, visit_mut::VisitMut, Result};

/// The sorted macro checks if an Enum's variants are sorted, and returns an error if they are not,
/// pointing to the correct variant before which the incorrect variant should be placed.
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

/// The check macro checks if the arms of a match statement are sorted, and returns an error if they
/// are not, pointing to the correct arm before which the incorrect variant should be placed. The
/// match expression itself must be given a `#[sorted]` attribute, and the function containing the
/// match statement should have the `#[sorted::check]` attribute
#[proc_macro_attribute]
pub fn check(args: TokenStream, input: TokenStream) -> TokenStream {
    assert!(args.is_empty());
    let mut func = parse_macro_input!(input as syn::ItemFn);

    let res = check_inner(&mut func);
    let mut out_tokens: TokenStream = func.to_token_stream().into();

    if let Err(e) = res {
        let error_tokens: TokenStream = e.to_compile_error().into();
        out_tokens.extend(error_tokens);
    }

    out_tokens
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

struct CheckMatchSorted {
    pub error: Option<syn::Error>,
}

impl CheckMatchSorted {
    fn new() -> Self {
        Self { error: None }
    }
}

fn attr_matches_sorted(attr: &syn::Attribute) -> bool {
    if let syn::Meta::Path(ref p) = attr.meta {
        if p.is_ident("sorted") {
            return true;
        }
    }
    false
}

impl syn::visit_mut::VisitMut for CheckMatchSorted {
    fn visit_expr_match_mut(&mut self, m: &mut syn::ExprMatch) {
        // Look for a `#[sorted]` attribute
        if m.attrs.iter().any(attr_matches_sorted) {
            // Remove the attribute (an attribute on an expression is a compile error)
            m.attrs.retain(|a| !attr_matches_sorted(a));
        }
    }
}

fn check_inner(func: &mut syn::ItemFn) -> Result<()> {
    let mut cms = CheckMatchSorted::new();

    cms.visit_item_fn_mut(func);

    if let Some(err) = cms.error {
        return Err(err);
    }

    Ok(())
}
