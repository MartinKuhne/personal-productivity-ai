//! `#[derive(ToolDescriptor)]` — generate the full
//! `impl Tool for XxxTool` block (descriptor + execute wiring)
//! from a `#[tool(...)]` attribute on a unit struct.
//!
//! ## Why a derive, and why a function pointer for `execute`?
//!
//! The `fastmd` tool registry has ten built-in families and a
//! handful of MCP-backed tools. Each one was a `pub struct
//! XxxTool;` unit struct plus a 7-line `static D: OnceLock<...>
//! = ...` block, plus a hand-rolled `impl Tool for XxxTool {
//! fn descriptor(...) { ... } fn execute(...) { ... } }`.
//!
//! The `#[derive(ToolDescriptor)]` derive collapses the
//! `OnceLock` boilerplate by generating the full
//! `impl Tool for XxxTool` block. The user supplies the
//! `execute()` body as a free function whose path is given in
//! the `execute_with` attribute. The generated `execute()`
//! method is a one-liner that calls the supplied function.
//!
//! The `execute_with` design is forced by Rust's coherence
//! rules: a trait can only be implemented once per type, so a
//! derive that generates `impl Tool for XxxTool { fn descriptor }
//! ...` and a user-supplied `impl Tool for XxxTool { fn execute
//! }` would be a "conflicting implementations" error. A free
//! function — one per tool — sidesteps the rule and keeps the
//! struct declaration short.
//!
//! ## Attribute keys
//!
//! | key           | required | kind                | meaning                          |
//! |---------------|----------|---------------------|----------------------------------|
//! | `name`        | yes      | string literal      | tool name as seen by the LLM     |
//! | `desc`        | yes      | expression          | description; coerces to `Cow<'static, str>` |
//! | `input`       | yes      | type                | input DTO; the macro emits `ToolDescriptor::new::<#input>(...)` |
//! | `safety`      | yes      | expression          | `Safety::ReadOnly` / `Safety::Mutating` |
//! | `group`       | yes      | identifier          | `InternalToolGroup` variant; the macro wraps it in `ToolGroupId::Internal(...)` |
//! | `execute_with`| yes      | path expression     | path to a free function with signature `fn(&XxxTool, &ToolContext, &str) -> Result<serde_json::Value, String>` |
//! | `config`      | no       | expression          | `ToolConfigSpec` value; defaults to `ToolConfigSpec::group_only(ToolGroupId::Internal(InternalToolGroup::<group>))` |
//! | `profile`     | no       | expression          | `ToolProfile` value; defaults to `ToolProfile::default()` and is applied via `with_profile` |
//!
//! Paths inside the generated code are `crate::`-rooted, so the
//! macro assumes the host crate is `fastmd` (the only consumer
//! today). Moving the derive to another crate would require
//! either renaming the absolute path or taking a `#[tool(crate =
//! "...")]` hint.
//!
//! ## Example
//!
//! ```ignore
//! use fastmd_tool_macros::ToolDescriptor;
//! use fastmd::agent::tools::Tool;
//!
//! #[derive(ToolDescriptor)]
//! #[tool(
//!     name = "search_notes",
//!     desc = "Search through Markdown notes",
//!     input = crate::agent::tools::dtos::SearchNotesInput,
//!     safety = crate::agent::tools::Safety::ReadOnly,
//!     group = Filesystem,
//!     execute_with = "execute_search_notes",
//! )]
//! pub(crate) struct SearchNotesTool;
//!
//! fn execute_search_notes(
//!     _self: &SearchNotesTool,
//!     ctx: &ToolContext,
//!     args: &str,
//! ) -> Result<serde_json::Value, String> {
//!     // body
//! }
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream, Result as ParseResult},
    DeriveInput, Expr, Ident, LitStr, Token, Type,
};

// ---------------------------------------------------------------------------
// ToolArgs — the `#[tool(name = "...", ...)]` payload.
// ---------------------------------------------------------------------------

/// Parsed payload of the `#[tool(...)]` helper attribute. Each
/// required key is mandatory; the optional keys fall back to
/// safe defaults at code-generation time.
struct ToolArgs {
    name: LitStr,
    desc: Expr,
    input: Type,
    safety: Expr,
    group: Ident,
    execute_with: Expr,
    config: Option<Expr>,
    profile: Option<Expr>,
}

impl Parse for ToolArgs {
    fn parse(input: ParseStream) -> ParseResult<Self> {
        // Parse the `key = value, key = value, ...` list.
        let mut name: Option<LitStr> = None;
        let mut desc: Option<Expr> = None;
        let mut input_ty: Option<Type> = None;
        let mut safety: Option<Expr> = None;
        let mut group: Option<Ident> = None;
        let mut execute_with: Option<Expr> = None;
        let mut config: Option<Expr> = None;
        let mut profile: Option<Expr> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            // Allow an optional `=` — `parse_meta` style.
            let _eq: Token![=] = input.parse()?;
            let key_str = key.to_string();
            match key_str.as_str() {
                "name" => {
                    if name.is_some() {
                        return Err(input.error("duplicate `name` key in #[tool(...)]"));
                    }
                    name = Some(input.parse::<LitStr>()?);
                }
                "desc" => {
                    if desc.is_some() {
                        return Err(input.error("duplicate `desc` key in #[tool(...)]"));
                    }
                    desc = Some(input.parse::<Expr>()?);
                }
                "input" => {
                    if input_ty.is_some() {
                        return Err(input.error("duplicate `input` key in #[tool(...)]"));
                    }
                    input_ty = Some(input.parse::<Type>()?);
                }
                "safety" => {
                    if safety.is_some() {
                        return Err(input.error("duplicate `safety` key in #[tool(...)]"));
                    }
                    safety = Some(input.parse::<Expr>()?);
                }
                "group" => {
                    if group.is_some() {
                        return Err(input.error("duplicate `group` key in #[tool(...)]"));
                    }
                    // `group` is a single Ident that names the
                    // `InternalToolGroup` variant — e.g. `Filesystem`.
                    // A path like `Email` would also be valid, but
                    // every existing caller is a single Ident.
                    group = Some(input.parse::<Ident>()?);
                }
                "execute_with" => {
                    if execute_with.is_some() {
                        return Err(input.error(
                            "duplicate `execute_with` key in #[tool(...)]",
                        ));
                    }
                    // `execute_with = "path::to::function"` is a
                    // path expression. We accept either a bare
                    // path (`my_fn`) or a string literal
                    // (`"path::to::fn"`) — the latter is friendlier
                    // for paths that need to thread through
                    // modules.
                    let lookahead = input.lookahead1();
                    let expr = if lookahead.peek(LitStr) {
                        // String-literal form: convert to a path
                        // expression so the generated code is
                        // uniform.
                        let lit: LitStr = input.parse()?;
                        let path: syn::Path = syn::parse_str(&lit.value())
                            .map_err(|_| input.error("`execute_with` string must be a valid Rust path"))?;
                        Expr::Path(syn::ExprPath {
                            attrs: Vec::new(),
                            qself: None,
                            path,
                        })
                    } else {
                        input.parse::<Expr>()?
                    };
                    execute_with = Some(expr);
                }
                "config" => {
                    if config.is_some() {
                        return Err(input.error("duplicate `config` key in #[tool(...)]"));
                    }
                    config = Some(input.parse::<Expr>()?);
                }
                "profile" => {
                    if profile.is_some() {
                        return Err(input.error("duplicate `profile` key in #[tool(...)]"));
                    }
                    profile = Some(input.parse::<Expr>()?);
                }
                other => {
                    return Err(input.error(format!(
                        "unknown key `{}` in #[tool(...)]; expected one of: \
                         name, desc, input, safety, group, execute_with, config, profile",
                        other
                    )));
                }
            }
            // Allow trailing comma between pairs.
            if input.peek(Token![,]) {
                let _comma: Token![,] = input.parse()?;
            } else {
                break;
            }
        }

        Ok(ToolArgs {
            name: name.ok_or_else(|| input.error("missing `name` in #[tool(...)]"))?,
            desc: desc.ok_or_else(|| input.error("missing `desc` in #[tool(...)]"))?,
            input: input_ty.ok_or_else(|| input.error("missing `input` in #[tool(...)]"))?,
            safety: safety.ok_or_else(|| input.error("missing `safety` in #[tool(...)]"))?,
            group: group.ok_or_else(|| input.error("missing `group` in #[tool(...)]"))?,
            execute_with: execute_with
                .ok_or_else(|| input.error("missing `execute_with` in #[tool(...)]"))?,
            config,
            profile,
        })
    }
}

// ---------------------------------------------------------------------------
// derive(ToolDescriptor)
// ---------------------------------------------------------------------------

/// Derive a `Tool::descriptor` method on a unit struct. See
/// the [module-level docs](self) for the attribute shape and a
/// worked example.
#[proc_macro_derive(ToolDescriptor, attributes(tool))]
pub fn derive_tool_descriptor(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).expect("derive(ToolDescriptor): parse input");
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    // Pull the `#[tool(...)]` helper attribute. `ast.attrs` has
    // every attribute; filter for one whose path is exactly
    // `tool` (we don't accept it through any prefix).
    let tool_attr = ast
        .attrs
        .iter()
        .find(|a| a.path().is_ident("tool"))
        .unwrap_or_else(|| {
            panic!(
                "derive(ToolDescriptor) on `{}` requires a `#[tool(...)]` attribute",
                name
            )
        });
    // `parse_args_with` walks the inside of the parentheses
    // (i.e. `name = "...", ...`) and feeds it to our
    // `ToolArgs::parse` impl. This is the standard
    // `syn`-recommended path for parsing `#[helper(...)]`
    // payloads and avoids the `Meta`-vs-`TokenStream`
    // mismatch that comes from calling
    // `attr.meta.to_token_stream()`.
    let args: ToolArgs = tool_attr
        .parse_args_with(ToolArgs::parse)
        .expect("derive(ToolDescriptor): parse #[tool(...)] attribute");

    let name_lit = &args.name;
    let desc_expr = &args.desc;
    let input_ty = &args.input;
    let safety_expr = &args.safety;
    let group_ident = &args.group;
    let execute_with = &args.execute_with;

    // `config` defaults to `group_only(ToolGroupId::Internal(InternalToolGroup::#group))`.
    let config_expr: TokenStream2 = match &args.config {
        Some(e) => quote! { #e },
        None => quote! {
            crate::tools::descriptor::ToolConfigSpec::group_only(
                crate::tools::registry::groups::ToolGroupId::Internal(
                    crate::tools::registry::groups::InternalToolGroup::#group_ident,
                ),
            )
        },
    };

    // `profile` defaults to `ToolProfile::default()`. The
    // generated code applies it via `with_profile` so the
    // caller's expression can be any `ToolProfile` value
    // (literal struct, `..ToolProfile::default()` shorthand,
    // or a helper function).
    let profile_expr: TokenStream2 = match &args.profile {
        Some(e) => quote! { #e },
        None => quote! { crate::tools::descriptor::ToolProfile::default() },
    };

    // Reuse the `group_id` value twice — once for the spec,
    // once for the descriptor. Computing it once via `let`
    // keeps the generated code small and avoids a subtle
    // ordering bug where the `spec`'s `group` field could
    // drift from the descriptor's `group` field.
    //
    // `execute()` is a one-liner that calls the user-supplied
    // function. The user-defined function takes
    // `(&self, &ToolContext, &str)` and returns the
    // tool's `Result`; passing `self` through preserves the
    // signature the trait promises even though unit-struct
    // tools never read it.
    let expanded = quote! {
        impl #impl_generics crate::tools::Tool for #name #ty_generics #where_clause {
            fn descriptor(&self) -> &'static crate::tools::ToolDescriptor {
                static D: ::std::sync::OnceLock<crate::tools::ToolDescriptor> =
                    ::std::sync::OnceLock::new();
                D.get_or_init(|| {
                    let group_id = crate::tools::registry::groups::ToolGroupId::Internal(
                        crate::tools::registry::groups::InternalToolGroup::#group_ident,
                    );
                    crate::tools::ToolDescriptor::new::<#input_ty>(
                        #name_lit,
                        #desc_expr,
                        #safety_expr,
                        #config_expr,
                        group_id,
                    )
                    .with_profile(#profile_expr)
                })
            }
            fn execute(
                &self,
                ctx: &crate::tools::context::ToolContext,
                args: &str,
            ) -> ::std::result::Result<serde_json::Value, String> {
                #execute_with(self, ctx, args)
            }
        }
    };

    expanded.into()
}
