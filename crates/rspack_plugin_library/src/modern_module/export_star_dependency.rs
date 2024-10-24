use rspack_core::{
  AsContextDependency, Dependency, InitFragmentExt, InitFragmentKey, InitFragmentStage,
  NormalInitFragment,
};
use rspack_core::{
  Compilation, DependencyType, ExternalRequest, ExternalType, ImportAttributes,
  RealDependencyLocation, RuntimeSpec,
};
use rspack_core::{DependencyCategory, DependencyId, DependencyTemplate};
use rspack_core::{ModuleDependency, TemplateContext, TemplateReplaceSource};
use rspack_plugin_javascript::dependency::create_resource_identifier_for_esm_dependency;
use swc_core::ecma::atoms::Atom;

#[derive(Debug, Clone)]
pub struct ModernModuleExportStarDependency {
  id: DependencyId,
  request: String,
  target_request: ExternalRequest,
  // external_type: ExternalType,
  // range: RealDependencyLocation,
  // attributes: Option<ImportAttributes>,
  resource_identifier: String,
}

impl ModernModuleExportStarDependency {
  pub fn new(
    request: String,
    target_request: ExternalRequest,
    // external_type: ExternalType,
    // range: RealDependencyLocation,
    // attributes: Option<ImportAttributes>,
  ) -> Self {
    let resource_identifier = create_resource_identifier_for_esm_dependency(request.as_str(), None);
    Self {
      request,
      target_request,
      // external_type,
      // range,
      id: DependencyId::new(),
      // attributes,
      resource_identifier,
    }
  }
}

impl Dependency for ModernModuleExportStarDependency {
  fn id(&self) -> &DependencyId {
    &self.id
  }

  fn resource_identifier(&self) -> Option<&str> {
    Some(&self.resource_identifier)
  }

  fn category(&self) -> &DependencyCategory {
    &DependencyCategory::Esm
  }

  fn dependency_type(&self) -> &DependencyType {
    &DependencyType::DynamicImport
  }

  // fn get_attributes(&self) -> Option<&ImportAttributes> {
  //   self.attributes.as_ref()
  // }

  // fn range(&self) -> Option<&RealDependencyLocation> {
  //   Some(&self.range)
  // }

  fn could_affect_referencing_module(&self) -> rspack_core::AffectType {
    rspack_core::AffectType::True
  }
}

impl ModuleDependency for ModernModuleExportStarDependency {
  fn request(&self) -> &str {
    &self.request
  }

  fn user_request(&self) -> &str {
    &self.request
  }

  fn set_request(&mut self, request: String) {
    self.request = request.into();
  }
}

impl DependencyTemplate for ModernModuleExportStarDependency {
  fn apply(
    &self,
    source: &mut TemplateReplaceSource,
    code_generatable_context: &mut TemplateContext,
  ) {
    let chunk_init_fragments = code_generatable_context.chunk_init_fragments();
    println!("🥣🥣🥣🥣🥣🥣🥣🥣🥣🥣🥣 {:?}", chunk_init_fragments);
    chunk_init_fragments.push(
      NormalInitFragment::new(
        format!("export * from \"{}\";\n", self.request.clone(),),
        InitFragmentStage::StageESMImports,
        0,
        InitFragmentKey::Const("gogogo".to_string()),
        None,
      )
      .boxed(),
    );
  }

  fn dependency_id(&self) -> Option<DependencyId> {
    Some(self.id)
  }

  fn update_hash(
    &self,
    _hasher: &mut dyn std::hash::Hasher,
    _compilation: &Compilation,
    _runtime: Option<&RuntimeSpec>,
  ) {
  }
}

impl AsContextDependency for ModernModuleExportStarDependency {}
