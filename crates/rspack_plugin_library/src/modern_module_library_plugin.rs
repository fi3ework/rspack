use std::hash::Hash;

use rspack_core::rspack_sources::{ConcatSource, RawSource, SourceExt};
use rspack_core::{
  merge_runtime, to_identifier, ApplyContext, AsyncDependenciesBlockIdentifier, ChunkUkey,
  CodeGenerationExportsFinalNames, Compilation, CompilationFinishModules,
  CompilationOptimizeChunkModules, CompilationParams, CompilerCompilation, CompilerOptions,
  ConcatenatedModule, ConcatenatedModuleExportsDefinitions, DependenciesBlock, Dependency,
  DependencyId, ExternalModule, ExternalRequest, LibraryOptions, ModuleIdentifier, Plugin,
  PluginContext,
};
use rspack_error::{error_bail, Result};
use rspack_hash::RspackHash;
use rspack_hook::{plugin, plugin_hook};
use rspack_plugin_javascript::dependency::ImportDependency;
use rspack_plugin_javascript::ModuleConcatenationPlugin;
use rspack_plugin_javascript::{
  ConcatConfiguration, JavascriptModulesChunkHash, JavascriptModulesRenderStartup, JsPlugin,
  RenderSource,
};
use rustc_hash::FxHashSet as HashSet;

use super::modern_module::ModernModuleImportDependency;
use crate::utils::{get_options_for_chunk, COMMON_LIBRARY_NAME_MESSAGE};

const PLUGIN_NAME: &str = "rspack.ModernModuleLibraryPlugin";

#[plugin]
#[derive(Debug, Default)]
pub struct ModernModuleLibraryPlugin;

impl ModernModuleLibraryPlugin {
  fn parse_options(&self, library: &LibraryOptions) -> Result<()> {
    if library.name.is_some() {
      error_bail!("Library name must be unset. {COMMON_LIBRARY_NAME_MESSAGE}")
    }
    Ok(())
  }

  fn get_options_for_chunk(
    &self,
    compilation: &Compilation,
    chunk_ukey: &ChunkUkey,
  ) -> Result<Option<()>> {
    get_options_for_chunk(compilation, chunk_ukey)
      .filter(|library| library.library_type == "modern-module")
      .map(|library| self.parse_options(library))
      .transpose()
  }

  // The `optimize_chunk_modules_impl` here will be invoked after the one in `ModuleConcatenationPlugin`.
  // Force trigger concatenation for single modules what bails from `ModuleConcatenationPlugin.is_empty`,
  // to keep all chunks can benefit from runtime optimization.
  async fn optimize_chunk_modules_impl(&self, compilation: &mut Compilation) -> Result<()> {
    let module_graph: rspack_core::ModuleGraph = compilation.get_module_graph();

    let module_ids: Vec<_> = module_graph
      .module_graph_modules()
      .keys()
      .copied()
      .collect();

    let mut concatenated_module_ids = HashSet::default();

    for module_id in &module_ids {
      let module = module_graph
        .module_by_identifier(module_id)
        .expect("should have module");

      if let Some(module) = module.as_ref().downcast_ref::<ConcatenatedModule>() {
        concatenated_module_ids.insert(*module_id);
        for inner_module in module.get_modules() {
          concatenated_module_ids.insert(inner_module.id);
        }
      }
    }

    let unconcatenated_module_ids = module_ids
      .iter()
      .filter(|id| !concatenated_module_ids.contains(id))
      .filter(|id| {
        let mgm = module_graph
          .module_graph_module_by_identifier(id)
          .expect("should have module");
        let reasons = &mgm.optimization_bailout;
        reasons
          .iter()
          // We did want force concatenate entry point here.
          // TODO: use constant variable to identify the reason.
          .filter(|r| !r.contains("Module is an entry point"))
          .collect::<Vec<_>>()
          .is_empty()
      })
      .collect::<HashSet<_>>();

    for module_id in unconcatenated_module_ids.into_iter() {
      let chunk_runtime = compilation
        .chunk_graph
        .get_module_runtimes(*module_id, &compilation.chunk_by_ukey)
        .into_values()
        .fold(Default::default(), |acc, r| merge_runtime(&acc, &r));

      let current_configuration: ConcatConfiguration =
        ConcatConfiguration::new(*module_id, Some(chunk_runtime.clone()));

      let mut used_modules = HashSet::default();

      ModuleConcatenationPlugin::process_concatenated_configuration(
        compilation,
        current_configuration,
        &mut used_modules,
      )
      .await?;
    }

    Ok(())
  }
}

#[plugin_hook(JavascriptModulesRenderStartup for ModernModuleLibraryPlugin)]
fn render_startup(
  &self,
  compilation: &Compilation,
  chunk_ukey: &ChunkUkey,
  module_id: &ModuleIdentifier,
  render_source: &mut RenderSource,
) -> Result<()> {
  let chunk = compilation.chunk_by_ukey.expect_get(chunk_ukey);
  let codegen = compilation
    .code_generation_results
    .get(module_id, Some(&chunk.runtime));

  let mut exports = vec![];
  let mut exports_with_property_access = vec![];

  let Some(_) = self.get_options_for_chunk(compilation, chunk_ukey)? else {
    return Ok(());
  };

  let mut source = ConcatSource::default();
  let module_graph = compilation.get_module_graph();
  source.add(render_source.source.clone());

  if let Some(exports_final_names) = codegen
    .data
    .get::<CodeGenerationExportsFinalNames>()
    .map(|d: &CodeGenerationExportsFinalNames| d.inner())
  {
    let exports_info = module_graph.get_exports_info(module_id);
    for export_info in exports_info.ordered_exports(&module_graph) {
      let info_name = export_info.name(&module_graph).expect("should have name");
      let used_name = export_info
        .get_used_name(&module_graph, Some(info_name), Some(&chunk.runtime))
        .expect("name can't be empty");

      let final_name = exports_final_names.get(used_name.as_str());

      let contains_char =
        |string: &str, chars: &str| -> bool { string.chars().any(|c| chars.contains(c)) };

      if let Some(final_name) = final_name {
        // Currently, there's not way to determine if a final_name contains a property access.
        if contains_char(final_name, "[]().") {
          exports_with_property_access.push((final_name, info_name));
        } else if info_name == final_name {
          exports.push(info_name.to_string());
        } else {
          exports.push(format!("{} as {}", final_name, info_name));
        }
      }
    }

    for (final_name, info_name) in exports_with_property_access.iter() {
      let var_name = format!("__webpack_exports__{}", to_identifier(info_name));

      source.add(RawSource::from(format!(
        "var {var_name} = {};\n",
        final_name
      )));

      exports.push(format!("{} as {}", var_name, info_name));
    }
  }

  if !exports.is_empty() {
    source.add(RawSource::from(format!(
      "export {{ {} }};\n",
      exports.join(", ")
    )));
  }

  render_source.source = source.boxed();
  Ok(())
}

#[plugin_hook(CompilationFinishModules for ModernModuleLibraryPlugin)]
async fn finish_modules(&self, compilation: &mut Compilation) -> Result<()> {
  let mut module_graph = compilation.get_module_graph_mut();
  let modules = module_graph.modules();
  let module_ids = modules.keys().cloned().collect::<Vec<_>>();

  for module_id in module_ids {
    let mut deps_to_replace = Vec::new();
    let module = module_graph.module_by_identifier(&module_id).unwrap();
    let connections = module_graph.get_outgoing_connections(&module_id);
    let block_ids = module.get_blocks();

    for block_id in block_ids {
      let block = module_graph.block_by_id(block_id).unwrap();
      for dep_id in block.get_dependencies() {
        let dep = module_graph.dependency_by_id(dep_id);
        if let Some(dep) = dep {
          if let Some(import_dependency) = dep.as_any().downcast_ref::<ImportDependency>() {
            let target_connection = connections.iter().find(|c| {
              let module_id = c.module_identifier();
              let module = module_graph.module_by_identifier(module_id).unwrap();
              if let Some(external_module) = module.as_any().downcast_ref::<ExternalModule>() {
                return external_module.user_request == import_dependency.request.to_string();
              } else {
                false
              }
            });

            if let Some(target_connection) = target_connection {
              // Rust version:
              let target_module_id = target_connection.module_identifier();
              let target_module = module_graph.module_by_identifier(target_module_id).unwrap();

              if let Some(target_module) = target_module.as_external_module() {
                if let ExternalRequest::Single(external_request_value) =
                  target_module.request.clone()
                {
                  let new_dep = ModernModuleImportDependency::new(
                    import_dependency.request.as_str().into(),
                    external_request_value.primary.as_str().into(),
                    import_dependency.range.clone(),
                    None,
                    None,
                  );

                  deps_to_replace.push((
                    block_id.clone(),
                    dep.clone(),
                    new_dep.clone(),
                    target_connection.id,
                  ));

                  // let b = module_graph
                  //   .block_by_id_mut(block_id)
                  //   .expect("should have block");

                  // block.take_dependencies();
                  // module_graph.add_block(block)
                  // module.get_blocks()
                  // block.add_dependency_id(*new_dep.id());
                }
              }
            }
          }
        }
      }
    }

    // let mut module_graph = compilation.get_module_graph_mut();
    for (block_id, dep, new_dep, connection_id) in deps_to_replace.iter() {
      let block = module_graph.block_by_id_mut(&block_id).unwrap();
      let dep_id = dep.id();
      block.remove_dependency_id(dep_id.clone());
      let boxed = Box::new(new_dep.clone()) as Box<dyn rspack_core::Dependency>;
      block.add_dependency_id(new_dep.id().clone());
      module_graph.add_dependency(boxed);

      // JS version:
      // Remove original external module connection
      // const originalModule = moduleGraph.getModule(dep);
      // if (originalModule) {
      //   moduleGraph.removeConnection(dep);
      // }

      // Rust:
      // let original_module = module_graph.module_by_identifier_mut(&module_id).unwrap();
      module_graph.revoke_connection(connection_id, true);
    }

    // let blocks =
    // map to connection ids
    // let connection_ids = connections.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
    // let mut ori_id_and_blocks = Vec::new();
    // for connection in connections {
    //   let original_module_identifier = connection.original_module_identifier.as_ref().unwrap();

    //   let original_module = module_graph
    //     .module_by_identifier(original_module_identifier)
    //     .unwrap();
    //   // deep clone block ids
    //   let block_ids: Vec<_> = original_module
    //     .get_blocks()
    //     .into_iter()
    //     // .map(|b| b.0.clone())
    //     .collect();
    //   ori_id_and_blocks.push((
    //     original_module_identifier.clone(),
    //     block_ids.clone(),
    //     connection.id,
    //   ));
    // }
    // id_to_blocks.push((module_id, user_request, request, ori_id_and_blocks));
    // }
  }

  // for (_module_id, user_request, request, ori_id_and_blocks) in id_to_blocks {
  //   // let mut blocks_for_info = Vec::new();

  //   for (ori_id, block_ids, connection_id) in ori_id_and_blocks.into_iter() {
  //     for block_id in block_ids {
  //       let block = module_graph
  //         .block_by_id(block_id)
  //         .expect("should have block");

  //       for dep_id in block.get_dependencies() {
  //         let dep = module_graph.dependency_by_id(dep_id);

  //         if let Some(dep) = dep {
  //           if let Some(import_dependency) = dep.as_any().downcast_ref::<ImportDependency>() {
  //             println!("🐷 dep: {:?}", import_dependency);
  // if import_dependency.request() == &user_request {
  //   println!("🐷 dep: {:?}", dep);

  //   if let ExternalRequest::Single(external_request_value) = request.clone() {
  //     let new_dep = ExternalModuleDependency::new(
  //       block.request().clone().unwrap().to_string(),
  //       external_request_value.primary,
  //       DependencyLocation {
  //         start: import_dependency.start,
  //         end: import_dependency.end,
  //         source: None,
  //       },
  //     );

  //     let info = (
  //       new_dep,
  //       ori_id.clone(),
  //       block_id.clone(),
  //       connection_id.clone(),
  //     );

  //     blocks_for_info.push(info);
  //   }
  // }
  //           }
  //         }
  //       }
  //     }
  //   }

  // blocks_for_info
  //   .iter()
  //   .for_each(|(dep, ori_id, block_id, connection_id)| {
  //     println!("🐷 dep: {:?}  id: {:?}", dep, ori_id);
  //     module_graph.add_dependency(Box::new(dep.clone()) as Box<dyn rspack_core::Dependency>);
  //     module_graph.revoke_connection(connection_id, true);
  //     let orig_module = module_graph.module_by_identifier_mut(ori_id).unwrap();
  //     orig_module.add_dependency_id(dep.id.clone());
  //     // clear orig_module blocks
  //     let blocks = orig_module.get_blocks().clone();
  //     // for block in blocks {
  //     // }
  //     println!("🐴 get_blocks: {:#?}", orig_module.get_blocks());
  //     orig_module.clear_blocks();
  //   });
  // }

  Ok(())
}

#[plugin_hook(JavascriptModulesChunkHash for ModernModuleLibraryPlugin)]
async fn js_chunk_hash(
  &self,
  compilation: &Compilation,
  chunk_ukey: &ChunkUkey,
  hasher: &mut RspackHash,
) -> Result<()> {
  let Some(_) = self.get_options_for_chunk(compilation, chunk_ukey)? else {
    return Ok(());
  };
  PLUGIN_NAME.hash(hasher);
  Ok(())
}

#[plugin_hook(CompilationOptimizeChunkModules for ModernModuleLibraryPlugin)]
async fn optimize_chunk_modules(&self, compilation: &mut Compilation) -> Result<Option<bool>> {
  self.optimize_chunk_modules_impl(compilation).await?;
  Ok(None)
}

#[plugin_hook(CompilerCompilation for ModernModuleLibraryPlugin)]
async fn compilation(
  &self,
  compilation: &mut Compilation,
  _params: &mut CompilationParams,
) -> Result<()> {
  let mut hooks = JsPlugin::get_compilation_hooks_mut(compilation);
  hooks.render_startup.tap(render_startup::new(self));
  hooks.chunk_hash.tap(js_chunk_hash::new(self));
  Ok(())
}

#[plugin_hook(ConcatenatedModuleExportsDefinitions for ModernModuleLibraryPlugin)]
fn exports_definitions(
  &self,
  _exports_definitions: &mut Vec<(String, String)>,
) -> Result<Option<bool>> {
  Ok(Some(true))
}

impl Plugin for ModernModuleLibraryPlugin {
  fn name(&self) -> &'static str {
    PLUGIN_NAME
  }

  fn apply(
    &self,
    ctx: PluginContext<&mut ApplyContext>,
    _options: &mut CompilerOptions,
  ) -> Result<()> {
    ctx
      .context
      .compiler_hooks
      .compilation
      .tap(compilation::new(self));

    ctx
      .context
      .concatenated_module_hooks
      .exports_definitions
      .tap(exports_definitions::new(self));

    ctx
      .context
      .compilation_hooks
      .optimize_chunk_modules
      .tap(optimize_chunk_modules::new(self));
    ctx
      .context
      .compilation_hooks
      .finish_modules
      .tap(finish_modules::new(self));

    Ok(())
  }
}
