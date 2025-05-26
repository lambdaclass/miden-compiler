use alloc::boxed::Box;

use crate::{derive::operation, dialects::test::TestDialect, effects::*, traits::*, *};

/// An operation for expressing constant immediate values.
///
/// This is used to materialize folded constants for the HIR dialect.
#[operation(
    dialect = TestDialect,
    traits(ConstantLike),
    implements(InferTypeOpInterface, Foldable, MemoryEffectOpInterface)
)]
pub struct Constant {
    #[attr(hidden)]
    value: Immediate,
    #[result]
    result: AnyInteger,
}

impl InferTypeOpInterface for Constant {
    fn infer_return_types(&mut self, _context: &Context) -> Result<(), Report> {
        let ty = self.value().ty();
        self.result_mut().set_type(ty);

        Ok(())
    }
}

impl Foldable for Constant {
    #[inline]
    fn fold(&self, results: &mut smallvec::SmallVec<[OpFoldResult; 1]>) -> FoldResult {
        results.push(OpFoldResult::Attribute(self.get_attribute("value").unwrap().clone_value()));
        FoldResult::Ok(())
    }

    #[inline(always)]
    fn fold_with(
        &self,
        _operands: &[Option<Box<dyn AttributeValue>>],
        results: &mut smallvec::SmallVec<[OpFoldResult; 1]>,
    ) -> FoldResult {
        self.fold(results)
    }
}

impl EffectOpInterface<MemoryEffect> for Constant {
    fn effects(&self) -> EffectIterator<MemoryEffect> {
        EffectIterator::from_smallvec(smallvec![])
    }
}

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
