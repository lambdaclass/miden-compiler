mod analysis;
mod instrumentation;
mod manager;
#[allow(clippy::module_inception)]
mod pass;
pub mod registry;
mod specialization;
pub mod statistics;

pub use self::{
    analysis::{Analysis, AnalysisManager, OperationAnalysis, PreservedAnalyses},
    instrumentation::{PassInstrumentation, PassInstrumentor, PipelineParentInfo},
    manager::{IRPrintingConfig, Nesting, OpPassManager, PassDisplayMode, PassManager},
    pass::{OperationPass, Pass, PassExecutionState, PassIdentifier, PostPassStatus},
    registry::{PassInfo, PassPipelineInfo},
    specialization::PassTarget,
    statistics::{PassStatistic, Statistic, StatisticValue},
};
use crate::{
    alloc::{string::String, vec::Vec},
    EntityRef, Operation, OperationName, OperationRef,
};

/// Struct that handles IR printing, based on provided configuration.
/// It is configured via the following CLI flags:
/// -Z print-ir-after-all: Enable IR printing for every pass.
/// -Z print-ir-after-pass: Enable IR printing only for some passes.
/// -Z print-ir-after-modified.: Only print the IR if it has been been modified.
#[derive(Default)]
pub struct Print {
    op_filter: Option<OpFilter>,
    selected_passes: Option<SelectedPasses>,
    target: Option<compact_str::CompactString>,
    only_when_modified: bool,
}

/// Select passes for IR printing.
#[derive(Debug)]
enum SelectedPasses {
    /// Select all passes for IR Printing.
    All,
    /// Just select a subset of passes for IR printing.
    Just(Vec<PassIdentifier>),
}

#[allow(dead_code)]
#[derive(Default, Debug)]
enum OpFilter {
    /// Print all operations
    #[default]
    All,
    /// Print any `Symbol` operation, optionally filtering by symbols whose name contains a given
    /// string.
    Symbol(Option<&'static str>),
    /// Print only operations of the given type
    Type {
        dialect: crate::interner::Symbol,
        op: crate::interner::Symbol,
    },
}

impl Print {
    pub fn new(config: &IRPrintingConfig) -> Option<Self> {
        let print = if config.print_ir_after_all
            || !config.print_ir_after_pass.is_empty()
            || config.print_ir_after_modified
        {
            Some(Self::default())
        } else {
            None
        };
        print.map(|p| p.with_pass_filter(config)).map(|p| p.with_symbol_filter(config))
    }

    pub fn with_type_filter<T: crate::OpRegistration>(mut self) -> Self {
        let dialect = <T as crate::OpRegistration>::dialect_name();
        let op = <T as crate::OpRegistration>::name();
        self.op_filter = Some(OpFilter::Type { dialect, op });
        self
    }

    #[allow(dead_code)]
    /// Create a printer that only prints `Symbol` operations containing `name`
    fn with_symbol_matching(mut self, name: &'static str) -> Self {
        self.op_filter = Some(OpFilter::Symbol(Some(name)));
        self
    }

    #[allow(unused_mut)]
    fn with_symbol_filter(mut self, _config: &IRPrintingConfig) -> Self {
        // NOTE: At the moment, symbol filtering is not processed by the CLI. However, were it to be
        // added, it could be done inside this function
        self.with_all_symbols()
    }

    fn with_all_symbols(mut self) -> Self {
        self.op_filter = Some(OpFilter::All);
        self
    }

    fn with_pass_filter(mut self, config: &IRPrintingConfig) -> Self {
        let is_ir_filter_set = if config.print_ir_after_all {
            self.selected_passes = Some(SelectedPasses::All);
            true
        } else if !config.print_ir_after_pass.is_empty() {
            self.selected_passes = Some(SelectedPasses::Just(config.print_ir_after_pass.clone()));
            true
        } else {
            false
        };

        if config.print_ir_after_modified {
            self.only_when_modified = true;
            // NOTE: If the user specified the "print after modification" flag, but didn't specify
            // any IR pass filter flag; then we assume that the desired behavior is to set the "all
            // pass" filter.
            if !is_ir_filter_set {
                self.selected_passes = Some(SelectedPasses::All);
            }
        };

        self
    }

    /// Specify the `log` target to write the IR output to.
    ///
    /// By default, the target is `printer`, unless the op is a `Symbol`, in which case it is the
    /// `Symbol` name.
    pub fn with_target(mut self, target: impl AsRef<str>) -> Self {
        let target = compact_str::CompactString::new(target.as_ref());
        self.target = Some(target);
        self
    }

    fn print_ir(&self, op: EntityRef<'_, Operation>) {
        match self.op_filter {
            Some(OpFilter::All) => {
                let target = self.target.as_deref().unwrap_or("printer");
                log::trace!(target: target, "{op}");
            }
            Some(OpFilter::Type {
                dialect,
                op: op_name,
            }) => {
                let name = op.name();
                if name.dialect() == dialect && name.name() == op_name {
                    let target = self.target.as_deref().unwrap_or("printer");
                    log::trace!(target: target, "{op}");
                }
            }
            Some(OpFilter::Symbol(None)) => {
                if let Some(sym) = op.as_symbol() {
                    let name = sym.name().as_str();
                    let target = self.target.as_deref().unwrap_or(name);
                    log::trace!(target: target, "{}", sym.as_symbol_operation());
                }
            }
            Some(OpFilter::Symbol(Some(filter))) => {
                if let Some(sym) = op.as_symbol().filter(|sym| sym.name().as_str().contains(filter))
                {
                    let target = self.target.as_deref().unwrap_or(filter);
                    log::trace!(target: target, "{}", sym.as_symbol_operation());
                }
            }
            None => (),
        }
    }

    fn pass_filter(&self, pass: &dyn OperationPass) -> bool {
        match &self.selected_passes {
            Some(SelectedPasses::All) => true,
            Some(SelectedPasses::Just(passes)) => passes.iter().any(|p| {
                if let Some(p_type) = pass.pass_id() {
                    *p == p_type
                } else {
                    false
                }
            }),
            None => false,
        }
    }

    fn should_print(&self, pass: &dyn OperationPass, ir_changed: PostPassStatus) -> bool {
        let pass_filter = self.pass_filter(pass);

        // Always print, unless "only_when_modified" has been set and there have not been changes.
        let modification_filter =
            !matches!((self.only_when_modified, ir_changed), (true, PostPassStatus::IRUnchanged));

        pass_filter && modification_filter
    }
}

impl PassInstrumentation for Print {
    fn run_before_pipeline(
        &mut self,
        _name: Option<&OperationName>,
        _parent_info: &PipelineParentInfo,
        op: OperationRef,
    ) {
        if !self.only_when_modified {
            return;
        }

        log::trace!("IR before the pass pipeline");
        let op = op.borrow();
        self.print_ir(op);
    }

    fn run_before_pass(&mut self, pass: &dyn OperationPass, op: &OperationRef) {
        if self.only_when_modified {
            return;
        }
        if self.pass_filter(pass) {
            log::trace!("Before the {} pass", pass.name());
            let op = op.borrow();
            self.print_ir(op);
        }
    }

    fn run_after_pass(
        &mut self,
        pass: &dyn OperationPass,
        op: &OperationRef,
        changed: PostPassStatus,
    ) {
        if self.should_print(pass, changed) {
            log::trace!("After the {} pass", pass.name());
            let op = op.borrow();
            self.print_ir(op);
        }
    }
}
