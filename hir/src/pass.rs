mod analysis;
mod instrumentation;
mod manager;
/// Made public momentarily.
#[allow(clippy::module_inception)]
pub mod pass;
pub mod registry;
mod specialization;
pub mod statistics;

pub use self::{
    analysis::{Analysis, AnalysisManager, OperationAnalysis, PreservedAnalyses},
    instrumentation::{PassInstrumentation, PassInstrumentor, PipelineParentInfo},
    manager::{IRPrintingConfig, Nesting, OpPassManager, PassDisplayMode, PassManager},
    pass::{IRAfterPass, OperationPass, Pass, PassExecutionState, PassType},
    registry::{PassInfo, PassPipelineInfo},
    specialization::PassTarget,
    statistics::{PassStatistic, Statistic, StatisticValue},
};
use crate::{
    alloc::{string::String, vec::Vec},
    EntityRef, Operation, OperationName, OperationRef,
};

/// A `Pass` which prints IR it is run on, based on provided configuration.
#[derive(Default)]
pub struct Print {
    filter: Option<OpFilter>,
    pass_filter: Option<PassFilter>,
    target: Option<compact_str::CompactString>,
    only_when_modified: bool,
}

/// Filter for the different passes.
#[derive(Default, Debug)]
enum PassFilter {
    /// Print IR regardless of which pass is executed.
    #[default]
    All,
    /// Only print IR if the pass's name is present in the vector.
    Certain(Vec<PassType>),
}

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
    // /// Create a printer that prints any operation
    // pub fn any() -> Self {
    //     Self {
    //         filter: OpFilter::All,
    //         pass_filter: PassFilter::All,
    //         target: None,
    //         only_when_modified: false,
    //     }
    // }
    pub fn with_type_filter<T: crate::OpRegistration>(mut self) -> Self {
        let dialect = <T as crate::OpRegistration>::dialect_name();
        let op = <T as crate::OpRegistration>::name();
        self.filter = Some(OpFilter::Type { dialect, op });
        self
    }

    /// Create a printer that only prints `Symbol` operations containing `name`
    pub fn with_symbol_matching(mut self, name: &'static str) -> Self {
        self.filter = Some(OpFilter::Symbol(Some(name)));
        self
    }

    pub fn with_all_symbols(mut self) -> Self {
        self.filter = Some(OpFilter::All);
        self
    }

    pub fn with_no_pass_filter(mut self) -> Self {
        self.pass_filter = Some(PassFilter::All);
        self
    }

    pub fn with_pass_filter(mut self, config: IRPrintingConfig) -> Self {
        if config.print_ir_after_all {
            self.pass_filter = Some(PassFilter::All);
        } else if !config.print_ir_after_pass.is_empty() {
            self.pass_filter = Some(PassFilter::Certain(config.print_ir_after_pass));
        }

        if config.print_ir_after_modified {
            self.only_when_modified = true;
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
        match self.filter {
            Some(OpFilter::All) => {
                let target = self.target.as_deref().unwrap_or("printer");
                log::error!(target: target, "{op}");
            }
            Some(OpFilter::Type {
                dialect,
                op: op_name,
            }) => {
                let name = op.name();
                if name.dialect() == dialect && name.name() == op_name {
                    let target = self.target.as_deref().unwrap_or("printer");
                    log::error!(target: target, "{op}");
                }
            }
            Some(OpFilter::Symbol(None)) => {
                if let Some(sym) = op.as_symbol() {
                    let name = sym.name().as_str();
                    let target = self.target.as_deref().unwrap_or(name);
                    log::error!(target: target, "{}", sym.as_symbol_operation());
                }
            }
            Some(OpFilter::Symbol(Some(filter))) => {
                if let Some(sym) = op.as_symbol().filter(|sym| sym.name().as_str().contains(filter))
                {
                    let target = self.target.as_deref().unwrap_or(filter);
                    log::error!(target: target, "{}", sym.as_symbol_operation());
                }
            }
            None => (),
        }
    }

    fn pass_filter(&self, pass: &dyn OperationPass) -> bool {
        match &self.pass_filter {
            Some(PassFilter::All) => true,
            Some(PassFilter::Certain(passes)) => passes.iter().any(|p| {
                if let Some(p_type) = pass.pass_type() {
                    *p == p_type
                } else {
                    false
                }
            }),
            None => true,
        }
    }

    fn should_print(&self, pass: &dyn OperationPass, ir_changed: IRAfterPass) -> bool {
        let pass_filter = self.pass_filter(pass);

        // Always print, unless "only_when_modified" has been set and there have not been changes.
        let modification_filter =
            !matches!((self.only_when_modified, ir_changed), (true, IRAfterPass::Unchanged));

        pass_filter && modification_filter
    }
}

impl PassInstrumentation for Print {
    fn run_before_pass(&mut self, pass: &dyn OperationPass, op: &OperationRef) {
        if self.only_when_modified {
            return;
        }
        if self.pass_filter(pass) {
            log::error!("Before the {} pass", pass.name());
            let op = op.borrow();
            self.print_ir(op);
        }
    }

    fn run_after_pass(
        &mut self,
        pass: &dyn OperationPass,
        op: &OperationRef,
        changed: IRAfterPass,
    ) {
        std::dbg!(changed);
        if self.should_print(pass, changed) {
            log::error!("After the {} pass", pass.name());
            let op = op.borrow();
            self.print_ir(op);
        }
    }

    fn run_before_pipeline(
        &mut self,
        _name: Option<&OperationName>,
        _parent_info: &PipelineParentInfo,
        op: OperationRef,
    ) {
        if !self.only_when_modified {
            return;
        }

        log::error!("IR before the pass pipeline");
        let op = op.borrow();
        self.print_ir(op);
    }
}
