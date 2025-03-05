//! Copies of the relevant `MIR` types. MIR represents a rust (function) body as a CFG. It's a
//! semantically rich representation that contains no high-level control-flow operations like loops
//! or patterns; instead the control flow is entirely described by gotos and switches on integer
//! values.
use crate::prelude::*;
#[cfg(feature = "rustc")]
use rustc_middle::{mir, ty};
#[cfg(feature = "rustc")]
use tracing::trace;

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx>>, from: rustc_middle::mir::SourceInfo, state: S as s)]
pub struct SourceInfo {
    pub span: Span,
    pub scope: SourceScope,
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx>>, from: rustc_middle::mir::LocalDecl<'tcx>, state: S as s)]
pub struct LocalDecl {
    pub mutability: Mutability,
    pub ty: Ty,
    pub source_info: SourceInfo,
    #[value(None)]
    pub name: Option<String>, // This information is contextual, thus the SInto instance initializes it to None, and then we fill it while `SInto`ing MirBody
}

pub type BasicBlocks = IndexVec<BasicBlock, BasicBlockData>;

#[cfg(feature = "rustc")]
fn name_of_local(
    local: rustc_middle::mir::Local,
    var_debug_info: &Vec<mir::VarDebugInfo>,
) -> Option<String> {
    var_debug_info
        .iter()
        .find(|info| {
            if let mir::VarDebugInfoContents::Place(place) = info.value {
                place.projection.is_empty() && place.local == local
            } else {
                false
            }
        })
        .map(|dbg| dbg.name.to_ident_string())
}

/// Enumerates the kinds of Mir bodies. TODO: use const generics
/// instead of an open list of types.
pub mod mir_kinds {
    use crate::prelude::{derive_group, JsonSchema};

    #[derive_group(Serializers)]
    #[derive(Clone, Copy, Debug, JsonSchema)]
    pub struct Built;

    #[derive_group(Serializers)]
    #[derive(Clone, Copy, Debug, JsonSchema)]
    pub struct Promoted;

    #[derive_group(Serializers)]
    #[derive(Clone, Copy, Debug, JsonSchema)]
    pub struct Elaborated;

    #[derive_group(Serializers)]
    #[derive(Clone, Copy, Debug, JsonSchema)]
    pub struct Optimized;

    #[derive_group(Serializers)]
    #[derive(Clone, Copy, Debug, JsonSchema)]
    pub struct CTFE;

    #[cfg(feature = "rustc")]
    pub use rustc::*;
    #[cfg(feature = "rustc")]
    mod rustc {
        use super::*;
        use rustc_middle::mir::Body;
        use rustc_middle::ty::TyCtxt;
        use rustc_span::def_id::LocalDefId;

        pub trait IsMirKind: Clone + std::fmt::Debug {
            // CPS to deal with stealable bodies cleanly.
            fn get_mir<'tcx, T>(
                tcx: TyCtxt<'tcx>,
                id: LocalDefId,
                f: impl FnOnce(&Body<'tcx>) -> T,
            ) -> Option<T>;
        }

        impl IsMirKind for Built {
            fn get_mir<'tcx, T>(
                tcx: TyCtxt<'tcx>,
                id: LocalDefId,
                f: impl FnOnce(&Body<'tcx>) -> T,
            ) -> Option<T> {
                let steal = tcx.mir_built(id);
                if steal.is_stolen() {
                    None
                } else {
                    Some(f(&steal.borrow()))
                }
            }
        }

        impl IsMirKind for Promoted {
            fn get_mir<'tcx, T>(
                tcx: TyCtxt<'tcx>,
                id: LocalDefId,
                f: impl FnOnce(&Body<'tcx>) -> T,
            ) -> Option<T> {
                let (steal, _) = tcx.mir_promoted(id);
                if steal.is_stolen() {
                    None
                } else {
                    Some(f(&steal.borrow()))
                }
            }
        }

        impl IsMirKind for Elaborated {
            fn get_mir<'tcx, T>(
                tcx: TyCtxt<'tcx>,
                id: LocalDefId,
                f: impl FnOnce(&Body<'tcx>) -> T,
            ) -> Option<T> {
                let steal = tcx.mir_drops_elaborated_and_const_checked(id);
                if steal.is_stolen() {
                    None
                } else {
                    Some(f(&steal.borrow()))
                }
            }
        }

        impl IsMirKind for Optimized {
            fn get_mir<'tcx, T>(
                tcx: TyCtxt<'tcx>,
                id: LocalDefId,
                f: impl FnOnce(&Body<'tcx>) -> T,
            ) -> Option<T> {
                Some(f(tcx.optimized_mir(id)))
            }
        }

        impl IsMirKind for CTFE {
            fn get_mir<'tcx, T>(
                tcx: TyCtxt<'tcx>,
                id: LocalDefId,
                f: impl FnOnce(&Body<'tcx>) -> T,
            ) -> Option<T> {
                Some(f(tcx.mir_for_ctfe(id)))
            }
        }
    }
}

#[cfg(feature = "rustc")]
pub use mir_kinds::IsMirKind;

#[derive_group(Serializers)]
#[derive(Clone, Debug, JsonSchema)]
pub struct ConstOperand {
    pub span: Span,
    pub ty: Ty,
    pub const_: ConstantExpr,
}

#[cfg(feature = "rustc")]
impl<'tcx, S: UnderOwnerState<'tcx>> SInto<S, ConstOperand>
    for rustc_middle::mir::ConstOperand<'tcx>
{
    fn sinto(&self, s: &S) -> ConstOperand {
        ConstOperand {
            span: self.span.sinto(s),
            ty: self.const_.ty().sinto(s),
            const_: self.const_.sinto(s),
        }
    }
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::Body<'tcx>, state: S as s)]
pub struct MirBody<KIND> {
    pub span: Span,
    #[map({
        let mut local_decls: rustc_index::IndexVec<rustc_middle::mir::Local, LocalDecl> = x.iter().map(|local_decl| {
            local_decl.sinto(s)
        }).collect();
        local_decls.iter_enumerated_mut().for_each(|(local, local_decl)| {
            local_decl.name = name_of_local(local, &self.var_debug_info);
        });
        let local_decls: rustc_index::IndexVec<Local, LocalDecl> = local_decls.into_iter().collect();
        local_decls.into()
    })]
    pub local_decls: IndexVec<Local, LocalDecl>,
    #[map(x.clone().as_mut().sinto(s))]
    pub basic_blocks: BasicBlocks,
    pub source_scopes: IndexVec<SourceScope, SourceScopeData>,
    pub tainted_by_errors: Option<ErrorGuaranteed>,
    #[value(std::marker::PhantomData)]
    pub _kind: std::marker::PhantomData<KIND>,
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx>>, from: rustc_middle::mir::SourceScopeData<'tcx>, state: S as s)]
pub struct SourceScopeData {
    pub span: Span,
    pub parent_scope: Option<SourceScope>,
    pub inlined_parent_scope: Option<SourceScope>,
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::Operand<'tcx>, state: S as s)]
pub enum Operand {
    Copy(Place),
    Move(Place),
    Constant(ConstOperand),
}

#[cfg(feature = "rustc")]
impl Operand {
    pub(crate) fn ty(&self) -> &Ty {
        match self {
            Operand::Copy(p) | Operand::Move(p) => &p.ty,
            Operand::Constant(c) => &c.ty,
        }
    }
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::Terminator<'tcx>, state: S as s)]
pub struct Terminator {
    pub source_info: SourceInfo,
    pub kind: TerminatorKind,
}

#[cfg(feature = "rustc")]
pub(crate) fn get_function_from_def_id_and_generics<'tcx, S: BaseState<'tcx> + HasOwnerId>(
    s: &S,
    def_id: rustc_hir::def_id::DefId,
    generics: rustc_middle::ty::GenericArgsRef<'tcx>,
) -> (DefId, Vec<GenericArg>, Vec<ImplExpr>, Option<ImplExpr>) {
    let tcx = s.base().tcx;

    // Retrieve the trait requirements for the **method**.
    // For instance, if we write:
    // ```
    // fn foo<T : Bar>(...)
    //            ^^^
    // ```
    let mut trait_refs = solve_item_required_traits(s, def_id, generics);

    // Check if this is a trait method call: retrieve the trait source if
    // it is the case (i.e., where does the method come from? Does it refer
    // to a top-level implementation? Or the method of a parameter? etc.).
    // At the same time, retrieve the trait obligations for this **trait**.
    // Remark: the trait obligations for the method are not the same as
    // the trait obligations for the trait. More precisely:
    //
    // ```
    // trait Foo<T : Bar> {
    //              ^^^^^
    //      trait level trait obligation
    //   fn baz(...) where T : ... {
    //      ...                ^^^
    //             method level trait obligation
    //   }
    // }
    // ```
    //
    // Also, a function doesn't need to belong to a trait to have trait
    // obligations:
    // ```
    // fn foo<T : Bar>(...)
    //            ^^^
    //     method level trait obligation
    // ```
    let (generics, source) = if let Some(assoc) = tcx.opt_associated_item(def_id) {
        // There is an associated item.
        use tracing::*;
        trace!("def_id: {:?}", def_id);
        trace!("assoc: def_id: {:?}", assoc.def_id);
        // Retrieve the `DefId` of the trait declaration or the impl block.
        let container_def_id = match assoc.container {
            rustc_middle::ty::AssocItemContainer::TraitContainer => {
                tcx.trait_of_item(assoc.def_id).unwrap()
            }
            rustc_middle::ty::AssocItemContainer::ImplContainer => {
                tcx.impl_of_method(assoc.def_id).unwrap()
            }
        };
        // The generics are split in two: the arguments of the container (trait decl or impl block)
        // and the arguments of the method.
        //
        // For instance, if we have:
        // ```
        // trait Foo<T> {
        //     fn baz<U>(...) { ... }
        // }
        //
        // fn test<T : Foo<u32>(x: T) {
        //     x.baz(...);
        //     ...
        // }
        // ```
        // The generics for the call to `baz` will be the concatenation: `<T, u32, U>`, which we
        // split into `<T, u32>` and `<U>`.
        //
        // If we have:
        // ```
        // impl<T: Ord> Map<T> {
        //     pub fn insert<U: Clone>(&mut self, x: U) { ... }
        // }
        // pub fn test(mut tree: Map<u32>) {
        //     tree.insert(false);
        // }
        // ```
        // The generics for `insert` are `<u32>` for the impl and `<bool>` for the method.
        match assoc.container {
            rustc_middle::ty::AssocItemContainer::TraitContainer => {
                let num_container_generics = tcx.generics_of(container_def_id).own_params.len();
                // Retrieve the trait information
                let impl_expr = self_clause_for_item(s, &assoc, generics).unwrap();
                // Return only the method generics; the trait generics are included in `impl_expr`.
                let method_generics = &generics[num_container_generics..];
                (method_generics.sinto(s), Some(impl_expr))
            }
            rustc_middle::ty::AssocItemContainer::ImplContainer => {
                // Solve the trait constraints of the impl block.
                let container_generics = tcx.generics_of(container_def_id);
                let container_generics = generics.truncate_to(tcx, container_generics);
                // Prepend the container trait refs.
                let mut combined_trait_refs =
                    solve_item_required_traits(s, container_def_id, container_generics);
                combined_trait_refs.extend(std::mem::take(&mut trait_refs));
                trait_refs = combined_trait_refs;
                (generics.sinto(s), None)
            }
        }
    } else {
        // Regular function call
        (generics.sinto(s), None)
    };

    (def_id.sinto(s), generics, trait_refs, source)
}

#[cfg(feature = "rustc")]
fn translate_terminator_kind_call<'tcx, S: BaseState<'tcx> + HasMir<'tcx> + HasOwnerId>(
    s: &S,
    terminator: &rustc_middle::mir::TerminatorKind<'tcx>,
) -> TerminatorKind {
    let tcx = s.base().tcx;
    let mir::TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        fn_span,
        ..
    } = terminator
    else {
        unreachable!()
    };

    let ty = func.ty(&s.mir().local_decls, tcx);
    let hax_ty: crate::Ty = ty.sinto(s);
    let sig = match hax_ty.kind() {
        TyKind::Arrow(sig) => sig,
        TyKind::Closure(_, args) => &args.untupled_sig,
        _ => supposely_unreachable_fatal!(
            s,
            "TerminatorKind_Call_expected_fn_type";
            { ty }
        ),
    };
    let fun_op = if let ty::TyKind::FnDef(def_id, generics) = ty.kind() {
        // The type of the value is one of the singleton types that corresponds to each function,
        // which is enough information.
        let (def_id, generics, trait_refs, trait_info) =
            get_function_from_def_id_and_generics(s, *def_id, *generics);
        FunOperand::Static {
            def_id,
            generics,
            trait_refs,
            trait_info,
        }
    } else {
        use mir::Operand;
        match func {
            Operand::Constant(_) => {
                unimplemented!("{:?}", func);
            }
            Operand::Move(place) => {
                // Function pointer or closure.
                let place = place.sinto(s);
                FunOperand::DynamicMove(place)
            }
            Operand::Copy(_place) => {
                unimplemented!("{:?}", func);
            }
        }
    };

    let late_bound_generics = sig
        .bound_vars
        .iter()
        .map(|var| match var {
            BoundVariableKind::Region(r) => r,
            BoundVariableKind::Ty(..) | BoundVariableKind::Const => {
                supposely_unreachable_fatal!(
                    s,
                    "non_lifetime_late_bound";
                    { var }
                )
            }
        })
        .map(|_| {
            GenericArg::Lifetime(Region {
                kind: RegionKind::ReErased,
            })
        })
        .collect();
    TerminatorKind::Call {
        fun: fun_op,
        late_bound_generics,
        args: args.sinto(s),
        destination: destination.sinto(s),
        target: target.sinto(s),
        unwind: unwind.sinto(s),
        fn_span: fn_span.sinto(s),
    }
}

// We don't use the LitIntType on purpose (we don't want the "unsuffixed" case)
#[derive_group(Serializers)]
#[derive(Clone, Copy, Debug, JsonSchema, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntUintTy {
    Int(IntTy),
    Uint(UintTy),
}

#[derive_group(Serializers)]
#[derive(Clone, Debug, JsonSchema)]
pub struct ScalarInt {
    /// Little-endian representation of the integer
    pub data_le_bytes: [u8; 16],
    pub int_ty: IntUintTy,
}

// TODO: naming conventions: is "translate" ok?
/// Translate switch targets
#[cfg(feature = "rustc")]
fn translate_switch_targets<'tcx, S: UnderOwnerState<'tcx>>(
    s: &S,
    switch_ty: &Ty,
    targets: &rustc_middle::mir::SwitchTargets,
) -> SwitchTargets {
    let targets_vec: Vec<(u128, BasicBlock)> =
        targets.iter().map(|(v, b)| (v, b.sinto(s))).collect();

    match switch_ty.kind() {
        TyKind::Bool => {
            // This is an: `if ... then ... else ...`
            assert!(targets_vec.len() == 1);
            // It seems the block targets are inverted
            let (test_val, otherwise_block) = targets_vec[0];

            assert!(test_val == 0);

            // It seems the block targets are inverted
            let if_block = targets.otherwise().sinto(s);

            SwitchTargets::If(if_block, otherwise_block)
        }
        TyKind::Int(_) | TyKind::Uint(_) => {
            let int_ty = match switch_ty.kind() {
                TyKind::Int(ty) => IntUintTy::Int(*ty),
                TyKind::Uint(ty) => IntUintTy::Uint(*ty),
                _ => unreachable!(),
            };

            // This is a: switch(int).
            // Convert all the test values to the proper values.
            let mut targets_map: Vec<(ScalarInt, BasicBlock)> = Vec::new();
            for (v, tgt) in targets_vec {
                // We need to reinterpret the bytes (`v as i128` is not correct)
                let v = ScalarInt {
                    data_le_bytes: v.to_le_bytes(),
                    int_ty,
                };
                targets_map.push((v, tgt));
            }
            let otherwise_block = targets.otherwise().sinto(s);

            SwitchTargets::SwitchInt(int_ty, targets_map, otherwise_block)
        }
        _ => {
            fatal!(s, "Unexpected switch_ty: {:?}", switch_ty)
        }
    }
}

#[derive_group(Serializers)]
#[derive(Clone, Debug, JsonSchema)]
pub enum SwitchTargets {
    /// Gives the `if` block and the `else` block
    If(BasicBlock, BasicBlock),
    /// Gives the integer type, a map linking values to switch branches, and the
    /// otherwise block. Note that matches over enumerations are performed by
    /// switching over the discriminant, which is an integer.
    SwitchInt(IntUintTy, Vec<(ScalarInt, BasicBlock)>, BasicBlock),
}

/// A value of type `fn<...> A -> B` that can be called.
#[derive_group(Serializers)]
#[derive(Clone, Debug, JsonSchema)]
pub enum FunOperand {
    /// Call to a statically-known function.
    Static {
        def_id: DefId,
        /// If `Some`, this is a method call on the given trait reference. Otherwise this is a call
        /// to a known function.
        trait_info: Option<ImplExpr>,
        /// If this is a trait method call, this only includes the method generics; the trait
        /// generics are included in the `ImplExpr` in `trait_info`.
        generics: Vec<GenericArg>,
        /// Trait predicates required by the function generics. Like for `generics`, this only
        /// includes the predicates required by the method, if applicable.
        trait_refs: Vec<ImplExpr>,
    },
    /// Use of a closure or a function pointer value. Counts as a move from the given place.
    DynamicMove(Place),
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::TerminatorKind<'tcx>, state: S as s)]
pub enum TerminatorKind {
    Goto {
        target: BasicBlock,
    },
    #[custom_arm(
        rustc_middle::mir::TerminatorKind::SwitchInt { discr, targets } => {
          let discr = discr.sinto(s);
          let targets = translate_switch_targets(s, discr.ty(), targets);
          TerminatorKind::SwitchInt {
              discr,
              targets,
          }
        }
    )]
    SwitchInt {
        discr: Operand,
        targets: SwitchTargets,
    },
    Return,
    Unreachable,
    Drop {
        place: Place,
        target: BasicBlock,
        unwind: UnwindAction,
        replace: bool,
    },
    #[custom_arm(
        x @ rustc_middle::mir::TerminatorKind::Call { .. } => {
          translate_terminator_kind_call(s, x)
        }
    )]
    Call {
        fun: FunOperand,
        /// A `FunOperand` is a value of type `fn<...> A -> B`. The generics in `<...>` are called
        /// "late-bound" and are instantiated anew at each call site. This list provides the
        /// generics used at this call-site. They are all lifetimes and at the time of writing are
        /// all erased lifetimes.
        late_bound_generics: Vec<GenericArg>,
        args: Vec<Spanned<Operand>>,
        destination: Place,
        target: Option<BasicBlock>,
        unwind: UnwindAction,
        fn_span: Span,
    },
    TailCall {
        func: Operand,
        args: Vec<Spanned<Operand>>,
        fn_span: Span,
    },
    Assert {
        cond: Operand,
        expected: bool,
        msg: AssertMessage,
        target: BasicBlock,
        unwind: UnwindAction,
    },
    Yield {
        value: Operand,
        resume: BasicBlock,
        resume_arg: Place,
        drop: Option<BasicBlock>,
    },
    CoroutineDrop,
    FalseEdge {
        real_target: BasicBlock,
        imaginary_target: BasicBlock,
    },
    FalseUnwind {
        real_target: BasicBlock,
        unwind: UnwindAction,
    },
    UnwindResume,
    UnwindTerminate(UnwindTerminateReason),
    InlineAsm {
        template: Vec<InlineAsmTemplatePiece>,
        operands: Vec<InlineAsmOperand>,
        options: InlineAsmOptions,
        line_spans: Vec<Span>,
        targets: Vec<BasicBlock>,
        unwind: UnwindAction,
    },
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::Statement<'tcx>, state: S as s)]
pub struct Statement {
    pub source_info: SourceInfo,
    #[map(Box::new(x.sinto(s)))]
    pub kind: Box<StatementKind>,
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::StatementKind<'tcx>, state: S as s)]
pub enum StatementKind {
    Assign((Place, Rvalue)),
    FakeRead((FakeReadCause, Place)),
    SetDiscriminant {
        place: Place,
        variant_index: VariantIdx,
    },
    Deinit(Place),
    StorageLive(Local),
    StorageDead(Local),
    Retag(RetagKind, Place),
    PlaceMention(Place),
    AscribeUserType((Place, UserTypeProjection), Variance),
    Coverage(CoverageKind),
    Intrinsic(NonDivergingIntrinsic),
    ConstEvalCounter,
    Nop,
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::NonDivergingIntrinsic<'tcx>, state: S as s)]
pub enum NonDivergingIntrinsic {
    Assume(Operand),
    CopyNonOverlapping(CopyNonOverlapping),
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::CopyNonOverlapping<'tcx>, state: S as s)]
pub struct CopyNonOverlapping {
    pub src: Operand,
    pub dst: Operand,
    pub count: Operand,
}

#[derive_group(Serializers)]
#[derive(Clone, Debug, JsonSchema)]
pub struct Place {
    /// The type of the element on which we apply the projection given by `kind`
    pub ty: Ty,
    pub kind: PlaceKind,
}

#[derive_group(Serializers)]
#[derive(Clone, Debug, JsonSchema)]
pub enum PlaceKind {
    Local(Local),
    Projection {
        place: Box<Place>,
        kind: ProjectionElem,
    },
}

#[derive_group(Serializers)]
#[derive(Clone, Debug, JsonSchema)]
pub enum ProjectionElemFieldKind {
    Tuple(FieldIdx),
    Adt {
        typ: DefId,
        variant: Option<VariantIdx>,
        index: FieldIdx,
    },
    /// Get access to one of the fields of the state of a closure
    ClosureState(FieldIdx),
}

#[derive_group(Serializers)]
#[derive(Clone, Debug, JsonSchema)]
pub enum ProjectionElem {
    Deref,
    Field(ProjectionElemFieldKind),
    Index(Local),
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast(Option<Symbol>, VariantIdx),
    OpaqueCast,
}

// refactor
#[cfg(feature = "rustc")]
impl<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>> SInto<S, Place>
    for rustc_middle::mir::Place<'tcx>
{
    #[tracing::instrument(level = "info", skip(s))]
    fn sinto(&self, s: &S) -> Place {
        let local_decl = &s.mir().local_decls[self.local];
        let mut current_ty: rustc_middle::ty::Ty = local_decl.ty;
        let mut current_kind = PlaceKind::Local(self.local.sinto(s));
        let mut elems: &[rustc_middle::mir::PlaceElem] = self.projection.as_slice();

        loop {
            use rustc_middle::mir::ProjectionElem::*;
            let cur_ty = current_ty;
            let cur_kind = current_kind.clone();
            use rustc_middle::ty::TyKind;
            let mk_field =
                |index: &rustc_target::abi::FieldIdx,
                 variant_idx: Option<rustc_target::abi::VariantIdx>| {
                    ProjectionElem::Field(match cur_ty.kind() {
                        TyKind::Adt(adt_def, _) => {
                            assert!(
                                ((adt_def.is_struct() || adt_def.is_union())
                                    && variant_idx.is_none())
                                    || (adt_def.is_enum() && variant_idx.is_some())
                            );
                            ProjectionElemFieldKind::Adt {
                                typ: adt_def.did().sinto(s),
                                variant: variant_idx.map(|id| id.sinto(s)),
                                index: index.sinto(s),
                            }
                        }
                        TyKind::Tuple(_types) => ProjectionElemFieldKind::Tuple(index.sinto(s)),
                        ty_kind => {
                            supposely_unreachable_fatal!(
                                s, "ProjectionElemFieldBadType";
                                {index, ty_kind, variant_idx, &cur_ty, &cur_kind}
                            );
                        }
                    })
                };
            let elem_kind: ProjectionElem = match elems {
                [Downcast(_, variant_idx), Field(index, ty), rest @ ..] => {
                    elems = rest;
                    let r = mk_field(index, Some(*variant_idx));
                    current_ty = *ty;
                    r
                }
                [elem, rest @ ..] => {
                    elems = rest;
                    use rustc_middle::ty::TyKind;
                    match elem {
                        Deref => {
                            current_ty = match current_ty.kind() {
                                TyKind::Ref(_, ty, _) | TyKind::RawPtr(ty, _) => *ty,
                                TyKind::Adt(def, generics) if def.is_box() => generics.type_at(0),
                                _ => supposely_unreachable_fatal!(
                                    s, "PlaceDerefNotRefNorPtrNorBox";
                                    {current_ty, current_kind, elem}
                                ),
                            };
                            ProjectionElem::Deref
                        }
                        Field(index, ty) => {
                            if let TyKind::Closure(_, generics) = cur_ty.kind() {
                                // We get there when we access one of the fields
                                // of the the state captured by a closure.
                                use crate::rustc_index::Idx;
                                let generics = generics.as_closure();
                                let upvar_tys = generics.upvar_tys();
                                current_ty = upvar_tys[index.sinto(s).index()];
                                ProjectionElem::Field(ProjectionElemFieldKind::ClosureState(
                                    index.sinto(s),
                                ))
                            } else {
                                let r = mk_field(index, None);
                                current_ty = *ty;
                                r
                            }
                        }
                        Index(local) => {
                            let (TyKind::Slice(ty) | TyKind::Array(ty, _)) = current_ty.kind()
                            else {
                                supposely_unreachable_fatal!(
                                    s,
                                    "PlaceIndexNotSlice";
                                    {current_ty, current_kind, elem}
                                );
                            };
                            current_ty = *ty;
                            ProjectionElem::Index(local.sinto(s))
                        }
                        ConstantIndex {
                            offset,
                            min_length,
                            from_end,
                        } => {
                            let (TyKind::Slice(ty) | TyKind::Array(ty, _)) = current_ty.kind()
                            else {
                                supposely_unreachable_fatal!(
                                    s, "PlaceConstantIndexNotSlice";
                                    {current_ty, current_kind, elem}
                                )
                            };
                            current_ty = *ty;
                            ProjectionElem::ConstantIndex {
                                offset: *offset,
                                min_length: *min_length,
                                from_end: *from_end,
                            }
                        }
                        Subslice { from, to, from_end } =>
                        // TODO: We assume subslice preserves the type
                        {
                            ProjectionElem::Subslice {
                                from: *from,
                                to: *to,
                                from_end: *from_end,
                            }
                        }
                        OpaqueCast(ty) => {
                            current_ty = *ty;
                            ProjectionElem::OpaqueCast
                        }
                        // This is used for casts to a subtype, e.g. between `for<‘a> fn(&’a ())`
                        // and `fn(‘static ())` (according to @compiler-errors on Zulip).
                        Subtype { .. } => panic!("unexpected Subtype"),
                        Downcast { .. } => panic!("unexpected Downcast"),
                    }
                }
                [] => break,
            };

            current_kind = PlaceKind::Projection {
                place: Box::new(Place {
                    ty: cur_ty.sinto(s),
                    kind: current_kind.clone(),
                }),
                kind: elem_kind,
            };
        }
        Place {
            ty: current_ty.sinto(s),
            kind: current_kind.clone(),
        }
    }
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::AggregateKind<'tcx>, state: S as s)]
pub enum AggregateKind {
    Array(Ty),
    Tuple,
    #[custom_arm(rustc_middle::mir::AggregateKind::Adt(def_id, vid, generics, annot, fid) => {
        let adt_kind = s.base().tcx.adt_def(def_id).adt_kind().sinto(s);
        let trait_refs = solve_item_required_traits(s, *def_id, generics);
        AggregateKind::Adt(
            def_id.sinto(s),
            vid.sinto(s),
            adt_kind,
            generics.sinto(s),
            trait_refs,
            annot.sinto(s),
            fid.sinto(s))
    })]
    Adt(
        DefId,
        VariantIdx,
        AdtKind,
        Vec<GenericArg>,
        Vec<ImplExpr>,
        Option<UserTypeAnnotationIndex>,
        Option<FieldIdx>,
    ),
    #[custom_arm(rustc_middle::mir::AggregateKind::Closure(def_id, generics) => {
        let closure = generics.as_closure();
        let args = ClosureArgs::sfrom(s, *def_id, closure);
        AggregateKind::Closure(def_id.sinto(s), args)
    })]
    Closure(DefId, ClosureArgs),
    Coroutine(DefId, Vec<GenericArg>),
    CoroutineClosure(DefId, Vec<GenericArg>),
    RawPtr(Ty, Mutability),
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S>, from: rustc_middle::mir::CastKind, state: S as _s)]
pub enum CastKind {
    PointerExposeProvenance,
    PointerWithExposedProvenance,
    PointerCoercion(PointerCoercion, CoercionSource),
    IntToInt,
    FloatToInt,
    FloatToFloat,
    IntToFloat,
    PtrToPtr,
    FnPtrToPtr,
    Transmute,
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S>, from: rustc_middle::mir::CoercionSource, state: S as _s)]
pub enum CoercionSource {
    AsCast,
    Implicit,
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::NullOp<'tcx>, state: S as s)]
pub enum NullOp {
    SizeOf,
    AlignOf,
    OffsetOf(Vec<(usize, FieldIdx)>),
    UbChecks,
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::Rvalue<'tcx>, state: S as s)]
pub enum Rvalue {
    Use(Operand),
    #[custom_arm(
        rustc_middle::mir::Rvalue::Repeat(op, ce) => {
            let op = op.sinto(s);
            Rvalue::Repeat(op, ce.sinto(s))
        },
    )]
    Repeat(Operand, ConstantExpr),
    Ref(Region, BorrowKind, Place),
    ThreadLocalRef(DefId),
    RawPtr(Mutability, Place),
    Len(Place),
    Cast(CastKind, Operand, Ty),
    BinaryOp(BinOp, (Operand, Operand)),
    NullaryOp(NullOp, Ty),
    UnaryOp(UnOp, Operand),
    Discriminant(Place),
    Aggregate(AggregateKind, IndexVec<FieldIdx, Operand>),
    ShallowInitBox(Operand, Ty),
    CopyForDeref(Place),
}

#[derive_group(Serializers)]
#[derive(AdtInto, Clone, Debug, JsonSchema)]
#[args(<'tcx, S: UnderOwnerState<'tcx> + HasMir<'tcx>>, from: rustc_middle::mir::BasicBlockData<'tcx>, state: S as s)]
pub struct BasicBlockData {
    pub statements: Vec<Statement>,
    pub terminator: Option<Terminator>,
    pub is_cleanup: bool,
}

make_idx_wrapper!(rustc_middle::mir, BasicBlock);
make_idx_wrapper!(rustc_middle::mir, SourceScope);
make_idx_wrapper!(rustc_middle::mir, Local);
make_idx_wrapper!(rustc_middle::ty, UserTypeAnnotationIndex);
make_idx_wrapper!(rustc_target::abi, FieldIdx);

/// Reflects [`rustc_middle::mir::UnOp`]
#[derive_group(Serializers)]
#[derive(AdtInto, Copy, Clone, Debug, JsonSchema)]
#[args(<'slt, S: UnderOwnerState<'slt>>, from: rustc_middle::mir::UnOp, state: S as _s)]
pub enum UnOp {
    Not,
    Neg,
    PtrMetadata,
}

/// Reflects [`rustc_middle::mir::BinOp`]
#[derive_group(Serializers)]
#[derive(AdtInto, Copy, Clone, Debug, JsonSchema)]
#[args(<'slt, S: UnderOwnerState<'slt>>, from: rustc_middle::mir::BinOp, state: S as _s)]
pub enum BinOp {
    // We merge the checked and unchecked variants because in either case overflow is failure.
    #[custom_arm(
        rustc_middle::mir::BinOp::Add | rustc_middle::mir::BinOp::AddUnchecked => BinOp::Add,
    )]
    Add,
    #[custom_arm(
        rustc_middle::mir::BinOp::Sub | rustc_middle::mir::BinOp::SubUnchecked => BinOp::Sub,
    )]
    Sub,
    #[custom_arm(
        rustc_middle::mir::BinOp::Mul | rustc_middle::mir::BinOp::MulUnchecked => BinOp::Mul,
    )]
    Mul,
    AddWithOverflow,
    SubWithOverflow,
    MulWithOverflow,
    Div,
    Rem,
    BitXor,
    BitAnd,
    BitOr,
    #[custom_arm(
        rustc_middle::mir::BinOp::Shl | rustc_middle::mir::BinOp::ShlUnchecked => BinOp::Shl,
    )]
    Shl,
    #[custom_arm(
        rustc_middle::mir::BinOp::Shr | rustc_middle::mir::BinOp::ShrUnchecked => BinOp::Shr,
    )]
    Shr,
    Eq,
    Lt,
    Le,
    Ne,
    Ge,
    Gt,
    Cmp,
    Offset,
}

#[cfg(feature = "rustc")]
impl<'tcx, S: UnderOwnerState<'tcx>> SInto<S, ConstantExpr> for rustc_middle::mir::Const<'tcx> {
    fn sinto(&self, s: &S) -> ConstantExpr {
        use rustc_middle::mir::Const;
        let tcx = s.base().tcx;
        match self {
            Const::Val(const_value, ty) => {
                const_value_to_constant_expr(s, *ty, *const_value, rustc_span::DUMMY_SP)
            }
            Const::Ty(_ty, c) => c.sinto(s),
            Const::Unevaluated(ucv, _ty) => {
                use crate::rustc_middle::query::Key;
                let span = tcx
                    .def_ident_span(ucv.def)
                    .unwrap_or_else(|| ucv.def.default_span(tcx));
                if ucv.promoted.is_some() {
                    self.eval_constant(s)
                        .unwrap_or_else(|| {
                            supposely_unreachable_fatal!(s, "UnevalPromotedConstant"; {self, ucv});
                        })
                        .sinto(s)
                } else {
                    match self.translate_uneval(s, ucv.shrink(), span) {
                        TranslateUnevalRes::EvaluatedConstant(c) => c.sinto(s),
                        TranslateUnevalRes::GlobalName(c) => c,
                    }
                }
            }
        }
    }
}

/// Reflects [`rustc_middle::mir::BorrowKind`]
#[derive(AdtInto)]
#[args(<S>, from: rustc_middle::mir::BorrowKind, state: S as gstate)]
#[derive_group(Serializers)]
#[derive(Copy, Clone, Debug, JsonSchema, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum BorrowKind {
    Shared,
    Fake(FakeBorrowKind),
    Mut { kind: MutBorrowKind },
}

/// Reflects [`rustc_middle::mir::MutBorrowKind`]
#[derive(AdtInto)]
#[args(<S>, from: rustc_middle::mir::MutBorrowKind, state: S as _s)]
#[derive_group(Serializers)]
#[derive(Copy, Clone, Debug, JsonSchema, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum MutBorrowKind {
    Default,
    TwoPhaseBorrow,
    ClosureCapture,
}

/// Reflects [`rustc_middle::mir::FakeBorrowKind`]
#[derive(AdtInto)]
#[args(<S>, from: rustc_middle::mir::FakeBorrowKind, state: S as _s)]
#[derive_group(Serializers)]
#[derive(Copy, Clone, Debug, JsonSchema, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum FakeBorrowKind {
    /// A shared (deep) borrow. Data must be immutable and is aliasable.
    Deep,
    /// The immediately borrowed place must be immutable, but projections from
    /// it don't need to be. This is used to prevent match guards from replacing
    /// the scrutinee. For example, a fake borrow of `a.b` doesn't
    /// conflict with a mutable borrow of `a.b.c`.
    Shallow,
}

sinto_todo!(rustc_ast::ast, InlineAsmTemplatePiece);
sinto_todo!(rustc_ast::ast, InlineAsmOptions);
sinto_todo!(rustc_middle::mir, InlineAsmOperand<'tcx>);
sinto_todo!(rustc_middle::mir, AssertMessage<'tcx>);
sinto_todo!(rustc_middle::mir, UnwindAction);
sinto_todo!(rustc_middle::mir, FakeReadCause);
sinto_todo!(rustc_middle::mir, RetagKind);
sinto_todo!(rustc_middle::mir, UserTypeProjection);
sinto_todo!(rustc_middle::mir, UnwindTerminateReason);
sinto_todo!(rustc_middle::mir::coverage, CoverageKind);
