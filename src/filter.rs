use crate::ForeignInterfacePath;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;

pub trait ImportFilter {
    fn filter_rule(&self, import_path: &ForeignInterfacePath) -> ImportRule;
}

impl Default for Box<dyn ImportFilter> {
    fn default() -> Self {
        Box::new(ImportRule::default())
    }
}

#[derive(Clone, Default, Debug)]
pub enum ImportRule {
    /// Skip the import and do not include it in the graph.
    Skip,

    /// Include the import.
    #[default]
    Include,

    /// Import even if the interface functions are not used.
    Force,
}

impl<F: ImportFilter> ImportFilter for &F {
    fn filter_rule(&self, path: &ForeignInterfacePath) -> ImportRule {
        (**self).filter_rule(path)
    }
}

impl<F: ImportFilter> ImportFilter for &mut F {
    fn filter_rule(&self, path: &ForeignInterfacePath) -> ImportRule {
        (**self).filter_rule(path)
    }
}

impl<F: ImportFilter> ImportFilter for Box<F> {
    fn filter_rule(&self, path: &ForeignInterfacePath) -> ImportRule {
        (**self).filter_rule(path)
    }
}

impl<F: ImportFilter> ImportFilter for Rc<F> {
    fn filter_rule(&self, path: &ForeignInterfacePath) -> ImportRule {
        (**self).filter_rule(path)
    }
}

impl<F: ImportFilter> ImportFilter for Arc<F> {
    fn filter_rule(&self, path: &ForeignInterfacePath) -> ImportRule {
        (**self).filter_rule(path)
    }
}

impl ImportFilter for dyn Fn(&ForeignInterfacePath) -> ImportRule {
    fn filter_rule(&self, import_path: &ForeignInterfacePath) -> ImportRule {
        self(import_path)
    }
}

impl ImportFilter for ImportRule {
    fn filter_rule(&self, _path: &ForeignInterfacePath) -> ImportRule {
        self.clone()
    }
}

impl<F: ImportFilter> ImportFilter for Vec<F> {
    fn filter_rule(&self, path: &ForeignInterfacePath) -> ImportRule {
        for filter in self {
            match filter.filter_rule(path) {
                ImportRule::Skip => return ImportRule::Skip,
                ImportRule::Force => return ImportRule::Force,
                ImportRule::Include => continue,
            }
        }
        ImportRule::Include
    }
}

#[derive(Clone, Debug)]
pub struct RegexMatchFilter<F: ImportFilter, D: ImportFilter = ImportRule> {
    regex: regex::Regex,
    match_rule: F,
    default_rule: D,
}

impl<F: ImportFilter> RegexMatchFilter<F, ImportRule> {
    pub fn new(regex: regex::Regex, match_rule: F) -> Self {
        Self::with_default(regex, match_rule, ImportRule::Include)
    }
}

impl<F: ImportFilter, D: ImportFilter> RegexMatchFilter<F, D> {
    pub fn with_default(regex: regex::Regex, match_rule: F, default_rule: D) -> Self {
        Self {
            regex,
            match_rule,
            default_rule,
        }
    }
}

impl<F: ImportFilter, D: ImportFilter> ImportFilter for RegexMatchFilter<F, D> {
    fn filter_rule(&self, import_path: &ForeignInterfacePath) -> ImportRule {
        if self.regex.is_match(&import_path.to_string()) {
            self.match_rule.filter_rule(import_path)
        } else {
            self.default_rule.filter_rule(import_path)
        }
    }
}
