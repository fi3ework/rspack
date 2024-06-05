use std::borrow::BorrowMut;
use std::hash::Hash;
use std::sync::Arc;

use rspack_core::rspack_sources::{ConcatSource, RawSource, SourceExt};
use rspack_core::{
  property_access, to_identifier, ApplyContext, ChunkUkey, CodeGenerationExportsFinalNames,
  Compilation, CompilationParams, CompilerCompilation, CompilerOptions, ConcatenatedModule,
  ConcatenatedModuleExportsDefinitions, DependencyType, ExportInfoProvided, FindTargetRetEnum,
  FindTargetRetValue, LibraryOptions, ModuleIdentifier, MutableModuleGraph, Plugin, PluginContext,
  ProvidedExports,
};
use rspack_error::{error_bail, Result};
use rspack_hook::{plugin, plugin_hook};
use rspack_plugin_javascript::{
  JavascriptModulesPluginPlugin, JsChunkHashArgs, JsPlugin, PluginJsChunkHashHookOutput,
  PluginRenderJsStartupHookOutput, RenderJsStartupArgs,
};
use rspack_util::ext::AsAny;

use crate::utils::{get_options_for_chunk, COMMON_LIBRARY_NAME_MESSAGE};

const PLUGIN_NAME: &str = "rspack.ModernModuleLibraryPlugin";

#[plugin]
#[derive(Debug, Default)]
pub struct ModernModuleLibraryPlugin {
  js_plugin: Arc<ModernModuleLibraryJavascriptModulesPluginPlugin>,
}

#[derive(Debug, Default)]
struct ModernModuleLibraryJavascriptModulesPluginPlugin;

impl ModernModuleLibraryJavascriptModulesPluginPlugin {
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
}

impl JavascriptModulesPluginPlugin for ModernModuleLibraryJavascriptModulesPluginPlugin {
  fn render_startup(&self, args: &RenderJsStartupArgs) -> PluginRenderJsStartupHookOutput {
    let chunk = args.compilation.chunk_by_ukey.expect_get(args.chunk);

    let codegen = args
      .compilation
      .code_generation_results
      .get(&args.module, Some(&chunk.runtime));

    let Some(_) = self.get_options_for_chunk(args.compilation, args.chunk)? else {
      return Ok(None);
    };

    let mut source = ConcatSource::default();
    let exports_final_names = codegen
      .data
      .get::<CodeGenerationExportsFinalNames>()
      .map(|d| d.inner())
      .expect("should have exports final names");
    let module_graph = args.compilation.get_module_graph();
    source.add(args.source.clone());
    let mut exports = vec![];

    let exports_info = module_graph.get_exports_info(&args.module);
    let exports_info_ids = module_graph.get_exports_info(&args.module).id;

    // get atoms from module
    let module = module_graph.module_by_identifier(&args.module).unwrap();
    println!("啊啊 {:?}", module);

    // if module is ConcatenatedModule, get the first module
    if let Some(concrete) = module.downcast_ref::<ConcatenatedModule>() {
      println!("啊啊 111111啊啊啊啊 ");
      // let inner_module = &concrete.root_module_ctxt.id;
      // for inner_module in inner_modules {
      let inner_module = module_graph
        .module_by_identifier(&concrete.root_module_ctxt.id)
        .unwrap();
      let inner_deps = inner_module.get_dependencies();
      for dep in inner_deps {
        let dep = module_graph.dependency_by_id(&dep).unwrap();
        println!("info! {:?}", *dep.dependency_type());
        if ((*dep.dependency_type() == DependencyType::EsmImportSpecifier)
          | (*dep.dependency_type() == DependencyType::EsmExportImportedSpecifier))
        {
          let dep_ids = dep.get_ids(&module_graph);
          println!("info!!! {:?} {:?}", dep.dependency_type(), dep_ids);
          for id in dep_ids {
            let export_info = exports_info_ids.get_read_only_export_info(&id, &module_graph);
            let ppp = export_info.provided;
            println!("🔥: {:?} --- {:?}", id, ppp);
          }
        }
      }
      // }
    }

    // let deps = module.get_dependencies();
    // for dep in deps {
    //   let dep = module_graph.dependency_by_id(&dep).unwrap();
    //   println!("info! {:?}", *dep.dependency_type());

    //   if (*dep.dependency_type() == DependencyType::EsmExportSpecifier) {
    //     // let dep_ids = dep.get_ids(&module_graph);
    //     // println!("info!!! {:?} {:?}", dep.dependency_type(), dep_ids);
    //     // for id in dep_ids {
    //     //   let export_info = exports_info_ids.get_read_only_export_info(&id, &module_graph);
    //     //   let ppp = export_info.provided;
    //     //   println!("info: {:?} --- {:?}", id, ppp);
    //     // }
    //   }
    // }

    // let atoms = &args.module.

    // let atom_id = exports_info_ids();

    for id in exports_info.get_ordered_exports() {
      // let info = exports_info_ids.get_read_only_export_info("", &module_graph);

      // println!("啊？{:?}", info.provided);
      // if matches!(info.provided, Some(ExportInfoProvided::False)) {
      //   println!("exports_info.provided is false, {}", id.to_string());
      //   //
      //   // return Ok(Some(source.boxed()));
      // }

      // let target = id.
      // let x = module_graph.mut
      let mut should_continue = false;
      // let mut mga = MutableModuleGraph::new(&mut module_graph);
      // let reexport = id.get_target(&mut mga, None);
      let reexport = id.find_target(&module_graph, Arc::new(|x: &ModuleIdentifier| true));

      if let FindTargetRetEnum::Value(v) = reexport {
        let exp = module_graph.get_exports_info(&v.module);
        for id in exp.get_ordered_exports() {
          let info = id.get_export_info(&module_graph);
          if (info.name.is_none()) {
            continue;
          }

          if (info.provided.is_none()
            && v.export.clone().unwrap()[0] == info.name.clone().unwrap().to_string())
          {
            should_continue = true;
          }
        }
      }

      if should_continue {
        continue;
      }

      let info = id.get_export_info(&module_graph);
      let chunk = args.compilation.chunk_by_ukey.expect_get(args.chunk);
      let info_name = info.name.as_ref().expect("should have name");
      let used_name = info
        .get_used_name(info.name.as_ref(), Some(&chunk.runtime))
        .expect("name can't be empty");

      let final_name = exports_final_names.get(used_name.as_str());

      if info_name == final_name.unwrap() {
        exports.push(info_name.to_string());
      } else {
        exports.push(format!("{} as {}", final_name.unwrap(), info_name));
      }
    }

    if !exports.is_empty() {
      source.add(RawSource::from(format!(
        "export {{ {} }};\n",
        exports.join(", ")
      )));
    }
    Ok(Some(source.boxed()))
  }

  fn js_chunk_hash(&self, args: &mut JsChunkHashArgs) -> PluginJsChunkHashHookOutput {
    let Some(_) = self.get_options_for_chunk(args.compilation, args.chunk_ukey)? else {
      return Ok(());
    };
    PLUGIN_NAME.hash(&mut args.hasher);
    Ok(())
  }
}

#[plugin_hook(CompilerCompilation for ModernModuleLibraryPlugin)]
async fn compilation(
  &self,
  compilation: &mut Compilation,
  _params: &mut CompilationParams,
) -> Result<()> {
  let mut drive = JsPlugin::get_compilation_drives_mut(compilation);
  drive.add_plugin(self.js_plugin.clone());
  Ok(())
}

#[plugin_hook(ConcatenatedModuleExportsDefinitions for ModernModuleLibraryPlugin)]
fn exports_definitions(
  &self,
  exports_definitions: &mut Vec<(String, String)>,
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

    Ok(())
  }
}
