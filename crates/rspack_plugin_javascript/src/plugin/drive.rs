use std::{fmt, ops::Deref, sync::Arc};

use async_trait::async_trait;
use rspack_core::{
  rspack_sources::BoxSource, BoxModule, Chunk, ChunkInitFragments, ChunkUkey, Compilation,
  ModuleIdentifier,
};
use rspack_error::Result;
use rspack_hash::RspackHash;

#[async_trait]
pub trait JavascriptModulesPluginPlugin {
  async fn render_chunk(&self, _args: &RenderJsChunkArgs) -> PluginRenderJsChunkHookOutput {
    Ok(None)
  }

  fn render(&self, _args: &RenderJsArgs) -> PluginRenderJsStartupHookOutput {
    Ok(None)
  }

  fn render_startup(&self, _args: &RenderJsStartupArgs) -> PluginRenderJsStartupHookOutput {
    Ok(None)
  }

  fn render_module_content<'a>(
    &'a self,
    args: RenderJsModuleContentArgs<'a>,
  ) -> PluginRenderJsModuleContentOutput<'a> {
    Ok(args)
  }

  fn js_chunk_hash(&self, _args: &mut JsChunkHashArgs) -> PluginJsChunkHashHookOutput {
    Ok(())
  }

  fn inline_in_runtime_bailout(&self) -> Option<String> {
    None
  }

  fn embed_in_runtime_bailout(
    &self,
    _compilation: &Compilation,
    _module: &BoxModule,
    _chunk: &Chunk,
  ) -> Result<Option<String>> {
    Ok(None)
  }

  fn strict_runtime_bailout(
    &self,
    _compilation: &Compilation,
    _chunk_ukey: &ChunkUkey,
  ) -> Result<Option<String>> {
    Ok(None)
  }
}

#[derive(Debug)]
pub struct RenderJsChunkArgs<'a> {
  pub compilation: &'a Compilation,
  pub chunk_ukey: &'a ChunkUkey,
  pub module_source: BoxSource,
}

impl<'me> RenderJsChunkArgs<'me> {
  pub fn chunk(&self) -> &Chunk {
    self.compilation.chunk_by_ukey.expect_get(self.chunk_ukey)
  }
}

#[derive(Debug)]
pub struct RenderJsModuleContentArgs<'a> {
  pub module_source: BoxSource,
  pub chunk_init_fragments: ChunkInitFragments,
  pub compilation: &'a Compilation,
  pub module: &'a BoxModule,
}

#[derive(Debug)]
pub struct RenderJsStartupArgs<'a> {
  // pub module_source: &'a BoxSource,
  pub compilation: &'a Compilation,
  pub chunk: &'a ChunkUkey,
  pub module: ModuleIdentifier,
  pub source: BoxSource,
}

impl<'me> RenderJsStartupArgs<'me> {
  pub fn chunk(&self) -> &Chunk {
    self.compilation.chunk_by_ukey.expect_get(self.chunk)
  }
}

#[derive(Debug)]
pub struct RenderJsArgs<'a> {
  pub source: &'a BoxSource,
  pub chunk: &'a ChunkUkey,
  pub compilation: &'a Compilation,
}

impl<'me> RenderJsArgs<'me> {
  pub fn chunk(&self) -> &Chunk {
    self.compilation.chunk_by_ukey.expect_get(self.chunk)
  }
}

pub struct JsChunkHashArgs<'a> {
  pub chunk_ukey: &'a ChunkUkey,
  pub compilation: &'a Compilation,
  pub hasher: &'a mut RspackHash,
}

impl<'me> JsChunkHashArgs<'me> {
  pub fn chunk(&self) -> &Chunk {
    self.compilation.chunk_by_ukey.expect_get(self.chunk_ukey)
  }
}

pub type PluginRenderJsChunkHookOutput = Result<Option<BoxSource>>;
pub type PluginRenderJsModuleContentOutput<'a> = Result<RenderJsModuleContentArgs<'a>>;
pub type PluginRenderJsStartupHookOutput = Result<Option<BoxSource>>;
pub type PluginRenderJsHookOutput = Result<Option<BoxSource>>;
pub type PluginJsChunkHashHookOutput = Result<()>;

pub type BoxJavascriptModulePluginPlugin = Box<dyn JavascriptModulesPluginPlugin + Send + Sync>;

#[async_trait]
impl<T: JavascriptModulesPluginPlugin + Send + Sync> JavascriptModulesPluginPlugin for Arc<T> {
  async fn render_chunk(&self, args: &RenderJsChunkArgs) -> PluginRenderJsChunkHookOutput {
    self.deref().render_chunk(args).await
  }

  fn render(&self, args: &RenderJsArgs) -> PluginRenderJsStartupHookOutput {
    self.deref().render(args)
  }

  fn render_startup(&self, args: &RenderJsStartupArgs) -> PluginRenderJsStartupHookOutput {
    self.deref().render_startup(args)
  }

  fn render_module_content<'a>(
    &'a self,
    args: RenderJsModuleContentArgs<'a>,
  ) -> PluginRenderJsModuleContentOutput<'a> {
    self.deref().render_module_content(args)
  }

  fn js_chunk_hash(&self, args: &mut JsChunkHashArgs) -> PluginJsChunkHashHookOutput {
    self.deref().js_chunk_hash(args)
  }

  fn inline_in_runtime_bailout(&self) -> Option<String> {
    self.deref().inline_in_runtime_bailout()
  }

  fn embed_in_runtime_bailout(
    &self,
    compilation: &Compilation,
    module: &BoxModule,
    chunk: &Chunk,
  ) -> Result<Option<String>> {
    self
      .deref()
      .embed_in_runtime_bailout(compilation, module, chunk)
  }

  fn strict_runtime_bailout(
    &self,
    compilation: &Compilation,
    chunk_ukey: &ChunkUkey,
  ) -> Result<Option<String>> {
    self.deref().strict_runtime_bailout(compilation, chunk_ukey)
  }
}

#[derive(Default)]
pub struct JavascriptModulesPluginPluginDrive {
  plugins: Vec<BoxJavascriptModulePluginPlugin>,
}

impl fmt::Debug for JavascriptModulesPluginPluginDrive {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("JavascriptModulesPluginPluginDrive")
      .field("plugins", &"..")
      .finish()
  }
}

impl JavascriptModulesPluginPluginDrive {
  pub fn add_plugin(&mut self, plugin: impl JavascriptModulesPluginPlugin + Send + Sync + 'static) {
    self.plugins.push(Box::new(plugin));
  }

  pub async fn render_chunk(&self, args: RenderJsChunkArgs<'_>) -> PluginRenderJsChunkHookOutput {
    for plugin in &self.plugins {
      if let Some(source) = plugin.render_chunk(&args).await? {
        return Ok(Some(source));
      }
    }
    Ok(None)
  }

  pub fn render(&self, args: RenderJsArgs) -> PluginRenderJsHookOutput {
    for plugin in &self.plugins {
      if let Some(source) = plugin.render(&args)? {
        return Ok(Some(source));
      }
    }
    Ok(None)
  }

  pub fn render_startup(&self, args: RenderJsStartupArgs) -> PluginRenderJsStartupHookOutput {
    let mut source = args.source;
    for plugin in &self.plugins {
      if let Some(s) = plugin.render_startup(&RenderJsStartupArgs {
        source: source.clone(),
        ..args
      })? {
        source = s;
      }
    }
    Ok(Some(source))
  }

  pub fn js_chunk_hash(&self, mut args: JsChunkHashArgs) -> PluginJsChunkHashHookOutput {
    for plugin in &self.plugins {
      plugin.js_chunk_hash(&mut args)?
    }
    Ok(())
  }

  pub fn render_module_content<'a>(
    &'a self,
    mut args: RenderJsModuleContentArgs<'a>,
  ) -> PluginRenderJsModuleContentOutput<'a> {
    for plugin in &self.plugins {
      args = plugin.render_module_content(args)?;
    }
    Ok(args)
  }

  pub fn inline_in_runtime_bailout(&self) -> Option<String> {
    for plugin in &self.plugins {
      if let Some(reason) = plugin.inline_in_runtime_bailout() {
        return Some(reason);
      }
    }
    None
  }

  pub fn embed_in_runtime_bailout(
    &self,
    compilation: &Compilation,
    module: &BoxModule,
    chunk: &Chunk,
  ) -> Result<Option<String>> {
    for plugin in &self.plugins {
      if let Some(reason) = plugin.embed_in_runtime_bailout(compilation, module, chunk)? {
        return Ok(Some(reason));
      }
    }
    Ok(None)
  }

  pub fn strict_runtime_bailout(
    &self,
    compilation: &Compilation,
    chunk_ukey: &ChunkUkey,
  ) -> Result<Option<String>> {
    for plugin in &self.plugins {
      if let Some(reason) = plugin.strict_runtime_bailout(compilation, chunk_ukey)? {
        return Ok(Some(reason));
      }
    }
    Ok(None)
  }
}

// === ModuleConcatenationPluginPlugin ===

// pub type ExportsDefinitionArgs = HashMap<String, String>;
// pub type ExportsDefinitionOutput = Result<Option<bool>>;
// pub type BoxModuleConcatenationPluginPlugin =
//   Box<dyn ModuleConcatenationPluginPlugin + Send + Sync>;

// pub struct ModuleConcatenationPluginPluginDrive {
//   plugins: Vec<ModuleConcatenationPluginPlugin>,
// }

// #[async_trait]
// pub trait ModuleConcatenationPluginPlugin {
//   async fn exports_definitions(&self, _args: &ExportsDefinitionArgs) -> ExportsDefinitionOutput {
//     Ok(None)
//   }
// }

// impl ModuleConcatenationPluginPluginDrive {
//   pub async fn exports_definitions(&self, args: &ExportsDefinitionArgs) -> ExportsDefinitionOutput {
//     Ok(None)
//   }
// }
