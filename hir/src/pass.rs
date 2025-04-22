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
    pass::{OperationPass, Pass, PassExecutionState},
    registry::{PassInfo, PassPipelineInfo},
    specialization::PassTarget,
    statistics::{PassStatistic, Statistic, StatisticValue},
};
use crate::{
    alloc::{string::String, vec::Vec},
    EntityRef, Operation, OperationRef,
};

/// A `Pass` which prints IR it is run on, based on provided configuration.
#[derive(Default)]
pub struct Print {
    filter: OpFilter,
    pass_filter: PassFilter,
    target: Option<compact_str::CompactString>,
}

/// Filter for the different passes.
#[derive(Default, Debug)]
enum PassFilter {
    /// Print IR regardless of which pass is executed.
    #[default]
    All,
    /// Only print IR if the pass's name is present in the vector.
    Certain(Vec<String>),
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
    /// Create a printer that prints any operation
    pub fn any() -> Self {
        Self {
            filter: OpFilter::All,
            pass_filter: PassFilter::All,
            target: None,
        }
    }

    /// Create a printer that only prints operations of type `T`
    pub fn only<T: crate::OpRegistration>() -> Self {
        let dialect = <T as crate::OpRegistration>::dialect_name();
        let op = <T as crate::OpRegistration>::name();
        Self {
            filter: OpFilter::Type { dialect, op },
            pass_filter: PassFilter::All,
            target: None,
        }
    }

    /// Adds a PassFilter to Print. IR will only be printed before and after those passes are
    /// executed.
    pub fn with_pass_filter(mut self, passes: Vec<String>) -> Self {
        self.pass_filter = PassFilter::Certain(passes);
        self
    }

    /// Create a printer that only prints `Symbol` operations containing `name`
    pub fn symbol_matching(name: &'static str) -> Self {
        Self {
            filter: OpFilter::Symbol(Some(name)),
            pass_filter: PassFilter::All,
            target: None,
        }
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
            OpFilter::All => {
                let target = self.target.as_deref().unwrap_or("printer");
                log::trace!(target: target, "{op}");
            }
            OpFilter::Type {
                dialect,
                op: op_name,
            } => {
                let name = op.name();
                if name.dialect() == dialect && name.name() == op_name {
                    let target = self.target.as_deref().unwrap_or("printer");
                    log::trace!(target: target, "{op}");
                }
            }
            OpFilter::Symbol(None) => {
                if let Some(sym) = op.as_symbol() {
                    let name = sym.name().as_str();
                    let target = self.target.as_deref().unwrap_or(name);
                    log::trace!(target: target, "{}", sym.as_symbol_operation());
                }
            }
            OpFilter::Symbol(Some(filter)) => {
                if let Some(sym) = op.as_symbol().filter(|sym| sym.name().as_str().contains(filter))
                {
                    let target = self.target.as_deref().unwrap_or(filter);
                    log::trace!(target: target, "{}", sym.as_symbol_operation());
                }
            }
        }
    }

    fn should_print(&self, pass: &dyn OperationPass) -> bool {
        match &self.pass_filter {
            PassFilter::All => true,
            PassFilter::Certain(passes) => passes.iter().any(|p| p == pass.name()),
        }
    }
}

impl Pass for Print {
    type Target = crate::Operation;

    fn name(&self) -> &'static str {
        "print"
    }

    fn can_schedule_on(&self, _name: &crate::OperationName) -> bool {
        true
    }

    fn run_on_operation(
        &mut self,
        op: crate::EntityMut<'_, Self::Target>,
        _state: &mut PassExecutionState,
    ) -> Result<(), crate::Report> {
        let op = op.into_entity_ref();
        self.print_ir(op);
        Ok(())
    }
}

impl PassInstrumentation for Print {
    fn run_after_pass(&mut self, pass: &dyn OperationPass, op: &OperationRef) {
        if self.should_print(pass) {
            let op = op.borrow();
            self.print_ir(op);
        }
    }

    fn run_before_pass(&mut self, pass: &dyn OperationPass, op: &OperationRef) {
        if self.should_print(pass) {
            let op = op.borrow();
            self.print_ir(op);
        }
    }
}
