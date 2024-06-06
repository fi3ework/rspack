use swc_core::ecma::ast::Program;

use super::JavascriptParserPlugin;
use crate::visitors::JavascriptParser;

#[derive(Default)]
pub struct StrictPlugin;

impl JavascriptParserPlugin for StrictPlugin {
  fn program(&self, parser: &mut JavascriptParser, _: &Program) -> Option<bool> {
    if let Some(strict) = parser.javascript_options.strict {
      parser.build_info.strict = strict;
    }

    None
  }
}
