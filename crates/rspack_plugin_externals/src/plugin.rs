use std::{borrow::BorrowMut, fmt::Debug};

use once_cell::sync::Lazy;
use regex::Regex;
use rspack_core::{
  ApplyContext, AsyncDependenciesBlockIdentifier, BoxModule, Compilation, CompilationFinishModules,
  CompilerOptions, ContextInfo, DependenciesBlock, Dependency, DependencyLocation, ExternalItem,
  ExternalItemFnCtx, ExternalItemValue, ExternalModule, ExternalRequest, ExternalRequestValue,
  ExternalType, ModuleDependency, ModuleExt, ModuleFactoryCreateData, NormalModuleFactoryFactorize,
  Plugin, PluginContext,
};
use rspack_error::Result;
use rspack_hook::{plugin, plugin_hook};
use rspack_plugin_javascript::{
  dependency::ImportDependency, utils::mangle_exports::NUMBER_OF_IDENTIFIER_CONTINUATION_CHARS,
};

use crate::external_module_dependency::ExternalModuleDependency;

static UNSPECIFIED_EXTERNAL_TYPE_REGEXP: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^[a-z0-9-]+ ").expect("Invalid regex"));

#[plugin]
#[derive(Debug)]
pub struct ExternalsPlugin {
  externals: Vec<ExternalItem>,
  r#type: ExternalType,
}

impl ExternalsPlugin {
  pub fn new(r#type: ExternalType, externals: Vec<ExternalItem>) -> Self {
    Self::new_inner(externals, r#type)
  }

  fn handle_external(
    &self,
    config: &ExternalItemValue,
    r#type: Option<String>,
    dependency: &dyn ModuleDependency,
  ) -> Option<ExternalModule> {
    let (external_module_config, external_module_type) = match config {
      ExternalItemValue::String(config) => {
        let (external_type, config) =
          if let Some((external_type, new_config)) = parse_external_type_from_str(config) {
            (external_type, new_config)
          } else {
            (self.r#type.clone(), config.to_owned())
          };
        (
          ExternalRequest::Single(ExternalRequestValue::new(config, None)),
          external_type,
        )
      }
      ExternalItemValue::Array(arr) => {
        let mut iter = arr.iter().peekable();
        let primary = iter.next()?;
        let (external_type, primary) =
          if let Some((external_type, new_primary)) = parse_external_type_from_str(primary) {
            (external_type, new_primary)
          } else {
            (self.r#type.clone(), primary.to_owned())
          };
        let rest = iter.peek().is_some().then(|| iter.cloned().collect());
        (
          ExternalRequest::Single(ExternalRequestValue::new(primary, rest)),
          external_type,
        )
      }
      ExternalItemValue::Bool(config) => {
        if *config {
          (
            ExternalRequest::Single(ExternalRequestValue::new(
              dependency.request().to_string(),
              None,
            )),
            self.r#type.clone(),
          )
        } else {
          return None;
        }
      }
      ExternalItemValue::Object(map) => (
        ExternalRequest::Map(
          map
            .iter()
            .map(|(k, v)| {
              let mut iter = v.iter().peekable();
              let primary = iter.next().expect("should have at least one value");
              let rest = iter.peek().is_some().then(|| iter.cloned().collect());
              (
                k.clone(),
                ExternalRequestValue::new(primary.to_owned(), rest),
              )
            })
            .collect(),
        ),
        self.r#type.clone(),
      ),
    };

    fn parse_external_type_from_str(v: &str) -> Option<(ExternalType, String)> {
      if UNSPECIFIED_EXTERNAL_TYPE_REGEXP.is_match(v)
        && let Some((t, c)) = v.split_once(' ')
      {
        return Some((t.to_owned(), c.to_owned()));
      }
      None
    }

    Some(ExternalModule::new(
      external_module_config,
      r#type.unwrap_or(external_module_type),
      dependency.request().to_owned(),
    ))
  }
}

#[plugin_hook(NormalModuleFactoryFactorize for ExternalsPlugin)]
async fn factorize(&self, data: &mut ModuleFactoryCreateData) -> Result<Option<BoxModule>> {
  let mut dep = data.dependency.as_module_dependency_mut();
  dep.as_mut().unwrap().set_request(".".to_string());

  let dependency = data
    .dependency
    .as_module_dependency()
    .expect("should be module dependency");

  // let new_dep = ExternalModuleDependency::new(
  //   "test1".to_string(),
  //   "test2".to_string(),
  //   DependencyLocation {
  //     start: 1,
  //     end: 2,
  //     source: None,
  //   },
  // );

  let context = &data.context;
  for external_item in &self.externals {
    match external_item {
      ExternalItem::Object(eh) => {
        let request = dependency.request();

        if let Some(value) = eh.get(request) {
          let maybe_module = self.handle_external(value, None, dependency);
          return Ok(maybe_module.map(|i| i.boxed()));
        }
      }
      ExternalItem::RegExp(r) => {
        let request = dependency.request();
        if r.test(request) {
          let maybe_module = self.handle_external(
            &ExternalItemValue::String(request.to_string()),
            None,
            dependency,
          );
          return Ok(maybe_module.map(|i| i.boxed()));
        }
      }
      ExternalItem::String(s) => {
        let request = dependency.request();
        if s == request {
          let maybe_module = self.handle_external(
            &ExternalItemValue::String(request.to_string()),
            None,
            dependency,
          );
          return Ok(maybe_module.map(|i| i.boxed()));
        }
      }
      ExternalItem::Fn(f) => {
        let request = dependency.request();
        let result = f(ExternalItemFnCtx {
          context: context.to_string(),
          request: request.to_string(),
          dependency_type: dependency.category().to_string(),
          context_info: ContextInfo {
            issuer: data
              .issuer
              .clone()
              .map_or("".to_string(), |i| i.to_string()),
          },
        })
        .await?;
        if let Some(r) = result.result {
          let maybe_module = self.handle_external(&r, result.external_type, dependency);
          return Ok(maybe_module.map(|i| i.boxed()));
        }
      }
    }
  }
  Ok(None)
}

#[plugin_hook(CompilationFinishModules for ExternalsPlugin)]
async fn finish_modules(&self, compilation: &mut Compilation) -> Result<()> {
  // let mut module_graph = compilation.get_module_graph_mut();
  // let modules = module_graph.modules();
  // let module_ids = modules.keys().cloned().collect::<Vec<_>>();
  // let mut loop_infos = Vec::new();
  // let mut blocks_for_info = Vec::new();

  // for module_id in module_ids {
  //   let module = module_graph.module_by_identifier(&module_id).unwrap();
  //   if let Some(external_module) = module.as_any().downcast_ref::<ExternalModule>() {
  //     let request = external_module.request.clone();
  //     let connections = module_graph.get_incoming_connections(&module_id);
  //     for connection in connections {
  //       let original_module_identifier = connection.original_module_identifier.as_ref().unwrap();
  //       let original_module = module_graph
  //         .module_by_identifier(original_module_identifier)
  //         .unwrap();
  //       let block_ids: Vec<_> = original_module.get_blocks().iter().collect();
  //       let connection_dep = module_graph
  //         .dependency_by_id(&connection.dependency_id)
  //         .expect("should have connection dependency");

  //       if let Some(connection_dep) = connection_dep.as_any().downcast_ref::<ImportDependency>() {
  //         let user_request = connection_dep.request().to_string();
  //         for block_id in block_ids {
  //           let block = module_graph
  //             .block_by_id(block_id)
  //             .expect("should have block");
  //           let block_deps = block.get_dependencies();
  //           for block_dep_id in block_deps {
  //             let block_dep = module_graph.dependency_by_id(block_dep_id);
  //             if let Some(dep) = block_dep {
  //               if let Some(block_dep) = dep.as_any().downcast_ref::<ImportDependency>() {
  //                 if block_dep.request() == user_request {
  //                   loop_infos.push((
  //                     user_request.clone(),
  //                     request.clone(),
  //                     original_module_identifier,
  //                     connection.id.clone(),
  //                     block_id,
  //                     block_dep.start,
  //                     block_dep.end,
  //                     block_dep.id(),
  //                   ));
  //                 }
  //               }
  //             }
  //           }
  //         }
  //       }
  //     }
  //   }
  // }

  // for (
  //   user_request,
  //   request,
  //   original_module_identifier,
  //   connection_id,
  //   block_id,
  //   start,
  //   end,
  //   block_dep_id,
  // ) in loop_infos
  // {
  //   // if import_dependency.request() == user_request {
  //   if let ExternalRequest::Single(external_request_value) = request.clone() {
  //     let new_dep = ExternalModuleDependency::new(
  //       user_request.to_string(),
  //       external_request_value.primary,
  //       DependencyLocation {
  //         start,
  //         end,
  //         source: None,
  //       },
  //     );

  //     let info = (
  //       new_dep,
  //       original_module_identifier.to_owned(),
  //       block_id.clone(),
  //       connection_id.clone(),
  //       block_dep_id.clone(),
  //     );

  //     blocks_for_info.push(info);
  //   }
  // }

  // for (dep, ori_id, block_id, connection_id, block_dep_id) in blocks_for_info {
  //   module_graph.add_dependency(Box::new(dep.clone()) as Box<dyn rspack_core::Dependency>);
  //   module_graph.revoke_connection(&connection_id, true);

  //   let orig_module = module_graph.module_by_identifier_mut(&ori_id).unwrap();
  //   orig_module.add_dependency_id(dep.id);
  //   orig_module.clear_block(block_id);
  // }

  Ok(())
}

impl Plugin for ExternalsPlugin {
  fn name(&self) -> &'static str {
    "rspack.ExternalsPlugin"
  }

  fn apply(
    &self,
    ctx: PluginContext<&mut ApplyContext>,
    _options: &mut CompilerOptions,
  ) -> Result<()> {
    ctx
      .context
      .normal_module_factory_hooks
      .factorize
      .tap(factorize::new(self));

    ctx
      .context
      .compilation_hooks
      .finish_modules
      .tap(finish_modules::new(self));
    Ok(())
  }
}
