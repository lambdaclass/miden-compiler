use smallvec::smallvec;

use crate::{derive::operation, dialects::test::*, effects::*, traits::*, *};

/// Store `value` on the heap at `addr`
#[operation(
    dialect = TestDialect,
    implements(MemoryEffectOpInterface)
)]
pub struct Store {
    #[operand]
    addr: AnyPointer,
    #[operand]
    value: AnyType,
}

impl EffectOpInterface<MemoryEffect> for Store {
    fn effects(&self) -> EffectIterator<MemoryEffect> {
        EffectIterator::from_smallvec(smallvec![EffectInstance::new_for_value(
            MemoryEffect::Write,
            self.addr().as_value_ref()
        )])
    }
}

/// Load `result` from the heap at `addr`
///
/// The type of load is determined by the pointer operand type - cast the pointer to the type you
/// wish to load, so long as such a load is safe according to the semantics of your high-level
/// language.
#[operation(
    dialect = TestDialect,
    implements(InferTypeOpInterface, MemoryEffectOpInterface)
)]
pub struct Load {
    #[operand]
    addr: AnyPointer,
    #[result]
    result: AnyType,
}

impl EffectOpInterface<MemoryEffect> for Load {
    fn effects(&self) -> EffectIterator<MemoryEffect> {
        EffectIterator::from_smallvec(smallvec![EffectInstance::new_for_value(
            MemoryEffect::Read,
            self.addr().as_value_ref()
        )])
    }
}

impl InferTypeOpInterface for Load {
    fn infer_return_types(&mut self, _context: &Context) -> Result<(), Report> {
        let _span = self.span();
        let pointee = {
            let addr = self.addr();
            let addr_value = addr.value();
            addr_value.ty().pointee().cloned()
        };
        match pointee {
            Some(pointee) => {
                self.result_mut().set_type(pointee);
                Ok(())
            }
            None => {
                // let addr = self.addr();
                // let addr_value = addr.value();
                // let addr_ty = addr_value.ty();
                // Err(context
                //     .session
                //     .diagnostics
                //     .diagnostic(midenc_session::diagnostics::Severity::Error)
                //     .with_message("invalid operand for 'load'")
                //     .with_primary_label(
                //         span,
                //         format!("invalid 'addr' operand, expected pointer, got '{addr_ty}'"),
                //     )
                //     .into_report())
                Ok(())
            }
        }
    }
}

pub type SymbolTableHolderRef = UnsafeIntrusiveEntityRef<SymbolTableHolder>;

/// An operation that holds a symboltable
///
#[operation(
    dialect = TestDialect,
    traits(
        SingleRegion,
        SingleBlock,
        NoRegionArguments,
        NoTerminator,
        // HasOnlyGraphRegion,
        // GraphRegionNoTerminator,
        IsolatedFromAbove,
    ),
    implements(SymbolTable)
)]
pub struct SymbolTableHolder {
    #[region]
    body: RegionRef,
    #[default]
    symbols: SymbolMap,
    #[default]
    uses: SymbolUseList,
}

impl SymbolTable for SymbolTableHolder {
    #[inline(always)]
    fn as_symbol_table_operation(&self) -> &Operation {
        &self.op
    }

    #[inline(always)]
    fn as_symbol_table_ref(&self) -> SymbolTableRef {
        unsafe { SymbolTableRef::from_raw(self) }
    }

    #[inline(always)]
    fn as_symbol_table_operation_mut(&mut self) -> &mut Operation {
        &mut self.op
    }

    fn symbol_manager(&self) -> SymbolManager<'_> {
        SymbolManager::new(&self.op, crate::Symbols::Borrowed(&self.symbols))
    }

    fn symbol_manager_mut(&mut self) -> SymbolManagerMut<'_> {
        SymbolManagerMut::new(&mut self.op, crate::SymbolsMut::Borrowed(&mut self.symbols))
    }

    #[inline]
    fn get(&self, name: SymbolName) -> Option<SymbolRef> {
        self.symbols.get(name)
    }
}

pub struct PrimSymbolTableHolderBuilder {
    pub sym_table_holder: SymbolTableHolderRef,
    builder: OpBuilder,
}

impl PrimSymbolTableHolderBuilder {
    pub fn new(sym_table_ref: SymbolTableHolderRef) -> Self {
        let sym_table_holder = sym_table_ref.borrow();
        let context = sym_table_holder.as_operation().context_rc();
        let mut builder = OpBuilder::new(context);

        let body = sym_table_holder.body();
        if let Some(current_block) = body.entry_block_ref() {
            builder.set_insertion_point_to_end(current_block);
        } else {
            let body_ref = body.as_region_ref();
            drop(body);
            builder.create_block(body_ref, None, &[]);
        }

        Self {
            sym_table_holder: sym_table_ref,
            builder,
        }
    }
}
