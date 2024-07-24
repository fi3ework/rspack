use rspack_core::{
  AsContextDependency, AsModuleDependency, Dependency, DependencyId, DependencyLocation,
  DependencyTemplate, ExportNameOrSpec, ExportSpec, ExportsOfExportsSpec, ExportsSpec, ModuleGraph,
  TemplateContext, TemplateReplaceSource,
};
#[derive(Debug, Clone)]
pub struct ExternalModuleDependency {
  pub id: DependencyId,
  request: String,
  target_request: String,
  range: DependencyLocation,
}

impl ExternalModuleDependency {
  pub fn new(request: String, target_request: String, range: DependencyLocation) -> Self {
    Self {
      id: DependencyId::new(),
      request,
      target_request,
      range,
    }
  }
}

impl Dependency for ExternalModuleDependency {
  fn id(&self) -> &rspack_core::DependencyId {
    &self.id
  }
}

impl AsModuleDependency for ExternalModuleDependency {}
impl AsContextDependency for ExternalModuleDependency {}

impl DependencyTemplate for ExternalModuleDependency {
  fn apply(
    &self,
    source: &mut TemplateReplaceSource,
    _code_generatable_context: &mut TemplateContext,
  ) {
    let content = self.target_request.to_string();
    source.replace(
      self.range.start,
      self.range.end,
      &format!("import(\"{}\")", content),
      None,
    );
  }

  fn dependency_id(&self) -> Option<DependencyId> {
    Some(self.id)
  }
}
