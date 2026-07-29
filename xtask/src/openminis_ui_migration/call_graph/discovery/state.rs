use super::*;

pub(super) struct DiscoveryState<'a> {
    pub(super) modules: &'a mut BTreeMap<String, RustModuleScope>,
    pub(super) definitions: &'a mut BTreeMap<String, String>,
    pub(super) functions: &'a mut Vec<RustFunctionCalls>,
    pub(super) method_dispatch: &'a mut BTreeMap<(String, String), BTreeSet<String>>,
    pub(super) local_types: &'a mut BTreeSet<String>,
    pub(super) local_traits: &'a mut BTreeSet<String>,
    pub(super) function_return_types: &'a mut BTreeMap<String, Vec<String>>,
    pub(super) function_try_return_types: &'a mut BTreeMap<String, Vec<String>>,
    pub(super) struct_fields: &'a mut BTreeMap<(String, String), Option<Vec<String>>>,
    pub(super) trait_impls: &'a mut Vec<(Vec<String>, Vec<String>)>,
    pub(super) deref_targets: &'a mut BTreeMap<String, Vec<String>>,
    pub(super) trait_default_methods: &'a mut BTreeMap<(String, String), String>,
    pub(super) declared_external_modules: &'a mut BTreeSet<String>,
    pub(super) test_modules: &'a BTreeSet<String>,
    pub(super) active_cfg: &'a BTreeSet<String>,
}
