use itertools::Itertools;
use rspack_core::SpanExt;
use swc_core::{atoms::Atom, common::Spanned};

use super::JavascriptParserPlugin;
use crate::{
  dependency::ProvideDependency,
  visitors::{CallHooksName, JavascriptParser},
};

const SOURCE_DOT: &str = r#"."#;
const MODULE_DOT: &str = r#"_dot_"#;

fn dep(parser: &JavascriptParser, name: &str, start: u32, end: u32) -> Option<ProvideDependency> {
  if let Some(requests) = parser.compiler_options.builtins.provide.get(name) {
    let name_identifier = if name.contains(SOURCE_DOT) {
      format!("__webpack_provide_{}", name.replace(SOURCE_DOT, MODULE_DOT))
    } else {
      name.to_string()
    };
    return Some(ProvideDependency::new(
      start,
      end,
      Atom::from(requests[0].as_str()),
      name_identifier,
      requests[1..]
        .iter()
        .map(|s| Atom::from(s.as_str()))
        .collect_vec(),
    ));
  }
  None
}

pub struct ProviderPlugin;

impl JavascriptParserPlugin for ProviderPlugin {
  fn call(
    &self,
    parser: &mut crate::visitors::JavascriptParser,
    expr: &swc_core::ecma::ast::CallExpr,
    for_name: &str,
  ) -> Option<bool> {
    dep(
      parser,
      for_name,
      expr.callee.span().real_lo(),
      expr.callee.span().real_hi(),
    )
    .map(|dep| {
      parser.dependencies.push(Box::new(dep));
      // FIXME: webpack use `walk_expression` here
      parser.walk_expr_or_spread(&expr.args);
      true
    })
  }

  fn member(
    &self,
    parser: &mut crate::visitors::JavascriptParser,
    expr: &swc_core::ecma::ast::MemberExpr,
    for_name: &str,
  ) -> Option<bool> {
    dep(
      parser,
      for_name,
      expr.span().real_lo(),
      expr.span().real_hi(),
    )
    .map(|dep| {
      parser.dependencies.push(Box::new(dep));
      true
    })
  }

  fn identifier(
    &self,
    parser: &mut crate::visitors::JavascriptParser,
    ident: &swc_core::ecma::ast::Ident,
    _for_name: &str,
  ) -> Option<bool> {
    let Some(name) = ident.sym.call_hooks_name(parser) else {
      return None;
    };
    dep(parser, &name, ident.span.real_lo(), ident.span.real_hi()).map(|dep| {
      parser.dependencies.push(Box::new(dep));
      true
    })
  }
}
