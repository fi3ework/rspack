use std::fmt::Debug;

use once_cell::sync::Lazy;
use regex::Regex;
use rspack_core::{
  external_module, module, ApplyContext, AsyncDependenciesBlockIdentifier, BoxModule, Compilation,
  CompilationFinishModules, CompilerOptions, ContextInfo, DependenciesBlock, Dependency,
  DependencyLocation, ExternalItem, ExternalItemFnCtx, ExternalItemValue, ExternalModule,
  ExternalRequest, ExternalRequestValue, ExternalType, ModuleDependency, ModuleExt,
  ModuleFactoryCreateData, NormalModuleFactoryFactorize, Plugin, PluginContext,
};
use rspack_error::Result;
use rspack_hook::{plugin, plugin_hook};
use rspack_plugin_javascript::dependency::ImportDependency;

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
  let dependency = data
    .dependency
    .as_module_dependency()
    .expect("should be module dependency");
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
  let mut module_graph = compilation.get_module_graph_mut();
  let modules = module_graph.modules();
  // map modules to module ids
  let module_ids = modules.keys().cloned().collect::<Vec<_>>();
  let mut id_to_blocks = Vec::new();

  for module_id in module_ids {
    let module = module_graph.module_by_identifier(&module_id).unwrap();
    if let Some(external_module) = module.as_any().downcast_ref::<ExternalModule>() {
      let user_request = external_module.user_request.clone();
      let request = external_module.request.clone();
      let connections = module_graph.get_incoming_connections(&module_id);
      // map to connection ids
      // let connection_ids = connections.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
      let mut ori_id_and_blocks = Vec::new();
      for connection in connections {
        let original_module_identifier = connection.original_module_identifier.as_ref().unwrap();
        let original_module = module_graph
          .module_by_identifier(original_module_identifier)
          .unwrap();
        // deep clone block ids
        let block_ids: Vec<_> = original_module
          .get_blocks()
          .into_iter()
          .map(|b| b.0.clone())
          .collect();
        ori_id_and_blocks.push((
          original_module_identifier.clone(),
          block_ids.clone(),
          connection.id,
        ));
      }
      id_to_blocks.push((module_id, user_request, request, ori_id_and_blocks));
    }
  }

  for (_module_id, user_request, request, ori_id_and_blocks) in id_to_blocks {
    let mut blocks_for_info = Vec::new();

    for (ori_id, block_ids, connection_id) in ori_id_and_blocks.into_iter() {
      for block_id in block_ids {
        let block = module_graph
          .block_by_id(&AsyncDependenciesBlockIdentifier(block_id.clone()))
          .expect("should have block");

        for dep_id in block.get_dependencies() {
          let dep = module_graph.dependency_by_id(dep_id);

          if let Some(dep) = dep {
            if let Some(import_dependency) = dep.as_any().downcast_ref::<ImportDependency>() {
              if import_dependency.request() == &user_request {
                println!("🐷 dep: {:?}", dep);

                if let ExternalRequest::Single(external_request_value) = request.clone() {
                  let new_dep = ExternalModuleDependency::new(
                    block.request().clone().unwrap().to_string(),
                    external_request_value.primary,
                    DependencyLocation {
                      start: import_dependency.start,
                      end: import_dependency.end,
                      source: None,
                    },
                  );

                  let info = (
                    new_dep,
                    ori_id.clone(),
                    block_id.clone(),
                    connection_id.clone(),
                  );

                  blocks_for_info.push(info);
                }
              }
            }
          }
        }
      }
    }

    blocks_for_info
      .iter()
      .for_each(|(dep, ori_id, block_id, connection_id)| {
        println!("🐷 dep: {:?}  id: {:?}", dep, ori_id);
        module_graph.add_dependency(Box::new(dep.clone()) as Box<dyn rspack_core::Dependency>);
        module_graph.revoke_connection(connection_id, true);
        let orig_module = module_graph.module_by_identifier_mut(ori_id).unwrap();
        orig_module.add_dependency_id(dep.id.clone());
        // clear orig_module blocks
        let blocks = orig_module.get_blocks().clone();
        // for block in blocks {
        // }
        println!("🐴 get_blocks: {:#?}", orig_module.get_blocks());
        orig_module.clear_blocks();
      });
  }

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
