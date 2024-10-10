use crate::prelude::*;

#[derive(AdtInto)]
#[args(<'tcx, S: UnderOwnerState<'tcx> >, from: rustc::PathChunk<'tcx>, state: S as s)]
#[derive_group(Serializers)]
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, JsonSchema)]
pub enum ImplExprPathChunk {
    AssocItem {
        item: AssocItem,
        predicate: Binder<TraitPredicate>,
        #[value(<_ as SInto<_, Clause>>::sinto(predicate, s).id)]
        predicate_id: PredicateId,
        index: usize,
    },
    Parent {
        predicate: Binder<TraitPredicate>,
        #[value(<_ as SInto<_, Clause>>::sinto(predicate, s).id)]
        predicate_id: PredicateId,
        index: usize,
    },
}

/// The source of a particular trait implementation. Most often this is either `Concrete` for a
/// concrete `impl Trait for Type {}` item, or `LocalBound` for a context-bound `where T: Trait`.
#[derive(AdtInto)]
#[args(<'tcx, S: UnderOwnerState<'tcx> >, from: rustc::ImplExprAtom<'tcx>, state: S as s)]
#[derive_group(Serializers)]
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, JsonSchema)]
pub enum ImplExprAtom {
    /// A concrete `impl Trait for Type {}` item.
    Concrete {
        #[from(def_id)]
        id: GlobalIdent,
        generics: Vec<GenericArg>,
    },
    /// A context-bound clause like `where T: Trait`.
    LocalBound {
        #[not_in_source]
        #[value({
            let Self::LocalBound { predicate, .. } = self else { unreachable!() };
            predicate.sinto(s).id
        })]
        predicate_id: PredicateId,
        r#trait: Binder<TraitRef>,
        path: Vec<ImplExprPathChunk>,
    },
    /// The automatic clause `Self: Trait` present inside a `impl Trait for Type {}` item.
    SelfImpl {
        r#trait: Binder<TraitRef>,
        path: Vec<ImplExprPathChunk>,
    },
    /// `dyn Trait` is a wrapped value with a virtual table for trait
    /// `Trait`.  In other words, a value `dyn Trait` is a dependent
    /// triple that gathers a type τ, a value of type τ and an
    /// instance of type `Trait`.
    /// `dyn Trait` implements `Trait` using a built-in implementation; this refers to that
    /// built-in implementation.
    Dyn,
    /// A built-in trait whose implementation is computed by the compiler, such as `Sync`.
    Builtin { r#trait: Binder<TraitRef> },
    /// An error happened while resolving traits.
    Error(String),
}

/// An `ImplExpr` describes the full data of a trait implementation. Because of generics, this may
/// need to combine several concrete trait implementation items. For example, `((1u8, 2u8),
/// "hello").clone()` combines the generic implementation of `Clone` for `(A, B)` with the
/// concrete implementations for `u8` and `&str`, represented as a tree.
#[derive_group(Serializers)]
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, JsonSchema, AdtInto)]
#[args(<'tcx, S: UnderOwnerState<'tcx> >, from: rustc::ImplExpr<'tcx>, state: S as s)]
pub struct ImplExpr {
    /// The trait this is an impl for.
    pub r#trait: Binder<TraitRef>,
    /// The kind of implemention of the root of the tree.
    pub r#impl: ImplExprAtom,
    /// A list of `ImplExpr`s required to fully specify the trait references in `impl`.
    pub args: Vec<ImplExpr>,
}

#[cfg(feature = "rustc")]
pub mod rustc {
    use rustc_hir::def_id::DefId;
    use rustc_middle::ty::*;

    /// This is the entrypoint of the solving.
    impl<'tcx, S: crate::UnderOwnerState<'tcx>> crate::SInto<S, crate::ImplExpr>
        for rustc_middle::ty::PolyTraitRef<'tcx>
    {
        fn sinto(&self, s: &S) -> crate::ImplExpr {
            use crate::ParamEnv;
            let warn = |msg: &str| crate::warning!(s, "{}", msg);
            match impl_expr(s.base().tcx, s.owner_id(), s.param_env(), self, &warn) {
                Ok(x) => x.sinto(s),
                Err(e) => crate::fatal!(s, "{}", e),
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    pub struct AnnotatedClause<'tcx> {
        pub is_extra_self_predicate: bool,
        /// Note: they are all actually `Clause`s.
        pub clause: Clause<'tcx>,
        pub span: rustc_span::Span,
    }

    #[extension_traits::extension(pub trait TyCtxtExtPredOrAbove)]
    impl<'tcx> TyCtxt<'tcx> {
        /// Just like `TyCtxt::predicates_defined_on`, but in the case of
        /// a trait or impl item, also includes the predicates defined on
        /// the parent.
        fn predicates_defined_on_or_above(
            self,
            did: rustc_span::def_id::DefId,
        ) -> Vec<AnnotatedClause<'tcx>> {
            let mut next_did = Some(did);
            let mut predicates = vec![];
            while let Some(did) = next_did {
                let (preds, parent) = self.annotated_predicates_of(did);
                next_did = parent;
                predicates.extend(preds)
            }
            predicates
        }

        fn annotated_predicates_of(
            self,
            did: rustc_span::def_id::DefId,
        ) -> (
            impl Iterator<Item = AnnotatedClause<'tcx>>,
            Option<rustc_span::def_id::DefId>,
        ) {
            let with_self = self.predicates_of(did);
            let parent = with_self.parent;
            let with_self = {
                let extra_predicates: Vec<(Clause<'_>, rustc_span::Span)> =
                    if rustc_hir::def::DefKind::OpaqueTy == self.def_kind(did) {
                        // An opaque type (e.g. `impl Trait`) provides
                        // predicates by itself: we need to account for them.
                        self.explicit_item_bounds(did)
                            .skip_binder() // Skips an `EarlyBinder`, likely for GATs
                            .iter()
                            .copied()
                            .collect()
                    } else {
                        vec![]
                    };
                with_self.predicates.iter().copied().chain(extra_predicates)
            };
            let without_self: Vec<Clause<'_>> = self
                .predicates_defined_on(did)
                .predicates
                .iter()
                .copied()
                .map(|(clause, _)| clause)
                .collect();
            (
                with_self.map(move |(clause, span)| AnnotatedClause {
                    is_extra_self_predicate: !without_self.contains(&clause),
                    clause,
                    span,
                }),
                parent,
            )
        }
    }

    // FIXME: this has visibility `pub(crate)` only because of https://github.com/rust-lang/rust/issues/83049
    pub(crate) mod search_clause {
        use super::{AnnotatedClause, Path, PathChunk, TyCtxtExtPredOrAbove};
        use crate::rustc_utils::*;
        use rustc_middle::ty::*;

        /// Custom equality on `Predicate`s.
        ///
        /// Sometimes Rustc inserts extra generic arguments: I noticed
        /// some `__H` second argument given to core::hash::Hash for
        /// instance. `__H` seems to be inserted in [1]. Such extra
        /// arguments seems to be ignored by `default_print_def_path` [2].
        ///
        /// Hence, for now, equality is decided by comparing the debug
        /// string representations of `Predicate`s.
        ///
        /// Note there exist also predicates that are different,
        /// `Eq`-wise, but whose `sinto` counterpart are equal.
        ///
        /// TODO: figure out how to implement this function in a sane way.
        ///
        /// [1]: https://github.com/rust-lang/rust/blob/b0889cb4ed0e6f3ed9f440180678872b02e7052c/compiler/rustc_builtin_macros/src/deriving/hash.rs#L20
        /// [2]: https://github.com/rust-lang/rust/blob/b0889cb4ed0e6f3ed9f440180678872b02e7052c/compiler/rustc_middle/src/ty/print/mod.rs#L141
        fn predicate_equality<'tcx>(
            tcx: TyCtxt<'tcx>,
            x: Predicate<'tcx>,
            y: Predicate<'tcx>,
            param_env: rustc_middle::ty::ParamEnv<'tcx>,
        ) -> bool {
            let erase_and_norm =
                |x| tcx.erase_regions(tcx.try_normalize_erasing_regions(param_env, x).unwrap_or(x));
            // Lifetime and constantness are irrelevant when resolving instances
            let x = erase_and_norm(x);
            let y = erase_and_norm(y);
            let sx = format!("{:?}", x.kind().skip_binder());
            let sy = format!("{:?}", y.kind().skip_binder());
            let result = sx == sy;
            // const DEBUG: bool = false;
            // if DEBUG && result {
            //     use crate::{Predicate, SInto};
            //     let xs: Predicate = x.sinto(s);
            //     let ys: Predicate = y.sinto(s);
            //     if x != y {
            //         eprintln!(
            //             "######################## predicate_equality ########################"
            //         );
            //         eprintln!("x={:#?}", x);
            //         eprintln!("y={:#?}", y);
            //         eprintln!(
            //             "########################        sinto       ########################"
            //         );
            //         eprintln!("sinto(x)={:#?}", xs);
            //         eprintln!("sinto(y)={:#?}", ys);
            //     }
            // }
            result
        }

        #[extension_traits::extension(pub trait TraitPredicateExt)]
        impl<'tcx> PolyTraitPredicate<'tcx> {
            fn predicates_to_poly_trait_predicates(
                self,
                tcx: TyCtxt<'tcx>,
                predicates: impl Iterator<Item = Predicate<'tcx>>,
            ) -> impl Iterator<Item = PolyTraitPredicate<'tcx>> {
                // Warning: this skip_binder seems dangerous
                let generics = self.skip_binder().trait_ref.args;
                predicates
                    .filter_map(|pred| pred.as_trait_clause())
                    .map(move |clause| clause.subst(tcx, generics))
            }

            #[tracing::instrument(level = "trace", skip(tcx))]
            fn parents_trait_predicates(
                self,
                tcx: TyCtxt<'tcx>,
            ) -> Vec<(usize, PolyTraitPredicate<'tcx>)> {
                let predicates = tcx
                    .predicates_defined_on_or_above(self.def_id())
                    .into_iter()
                    .map(|apred| apred.clause.as_predicate());
                self.predicates_to_poly_trait_predicates(tcx, predicates)
                    .enumerate()
                    .collect()
            }
            #[tracing::instrument(level = "trace", skip(tcx))]
            fn associated_items_trait_predicates(
                self,
                tcx: TyCtxt<'tcx>,
            ) -> Vec<(
                AssocItem,
                EarlyBinder<'tcx, Vec<(usize, PolyTraitPredicate<'tcx>)>>,
            )> {
                tcx.associated_items(self.def_id())
                    .in_definition_order()
                    .filter(|item| item.kind == AssocKind::Type)
                    .copied()
                    .map(|item| {
                        let bounds = tcx.item_bounds(item.def_id).map_bound(|clauses| {
                            self.predicates_to_poly_trait_predicates(
                                tcx,
                                clauses.into_iter().map(|clause| clause.as_predicate()),
                            )
                            .enumerate()
                            .collect()
                        });
                        (item, bounds)
                    })
                    .collect()
            }
        }

        #[tracing::instrument(level = "trace", skip(tcx, param_env))]
        pub(super) fn path_to<'tcx>(
            tcx: TyCtxt<'tcx>,
            starting_points: &[AnnotatedClause<'tcx>],
            target: PolyTraitRef<'tcx>,
            param_env: rustc_middle::ty::ParamEnv<'tcx>,
        ) -> Option<(Path<'tcx>, AnnotatedClause<'tcx>)> {
            /// A candidate projects `self` along a path reaching some
            /// predicate. A candidate is selected when its predicate
            /// is the one expected, aka `target`.
            #[derive(Debug)]
            struct Candidate<'tcx> {
                path: Path<'tcx>,
                pred: PolyTraitPredicate<'tcx>,
                origin: AnnotatedClause<'tcx>,
            }

            use std::collections::VecDeque;
            let mut candidates: VecDeque<Candidate<'tcx>> = starting_points
                .into_iter()
                .filter_map(|pred| {
                    let clause = pred.clause.as_trait_clause();
                    clause.map(|clause| Candidate {
                        path: vec![],
                        pred: clause,
                        origin: *pred,
                    })
                })
                .collect();

            let target_pred = target.upcast(tcx);
            let mut seen = std::collections::HashSet::new();

            while let Some(candidate) = candidates.pop_front() {
                {
                    // If a predicate was already seen, we know it is
                    // not the one we are looking for: we skip it.
                    if seen.iter().any(|seen_pred: &PolyTraitPredicate<'tcx>| {
                        predicate_equality(
                            tcx,
                            candidate.pred.upcast(tcx),
                            (*seen_pred).upcast(tcx),
                            param_env,
                        )
                    }) {
                        continue;
                    }
                    seen.insert(candidate.pred);
                }

                // if the candidate equals the target, let's return its path
                if predicate_equality(tcx, candidate.pred.upcast(tcx), target_pred, param_env) {
                    return Some((candidate.path, candidate.origin));
                }

                // otherwise, we add to the queue all paths reachable from the candidate
                for (index, parent_pred) in candidate.pred.parents_trait_predicates(tcx) {
                    let mut path = candidate.path.clone();
                    path.push(PathChunk::Parent {
                        predicate: parent_pred,
                        index,
                    });
                    candidates.push_back(Candidate {
                        pred: parent_pred,
                        path,
                        origin: candidate.origin,
                    });
                }
                for (item, binder) in candidate.pred.associated_items_trait_predicates(tcx) {
                    // Warning: this skip_binder seems dangerous
                    for (index, parent_pred) in binder.skip_binder().into_iter() {
                        let mut path = candidate.path.clone();
                        path.push(PathChunk::AssocItem {
                            item,
                            predicate: parent_pred,
                            index,
                        });
                        candidates.push_back(Candidate {
                            pred: parent_pred,
                            path,
                            origin: candidate.origin,
                        });
                    }
                }
            }
            None
        }
    }

    #[derive(Debug, Clone)]
    pub enum PathChunk<'tcx> {
        AssocItem {
            item: AssocItem,
            predicate: PolyTraitPredicate<'tcx>,
            index: usize,
        },
        Parent {
            predicate: PolyTraitPredicate<'tcx>,
            index: usize,
        },
    }
    pub type Path<'tcx> = Vec<PathChunk<'tcx>>;

    #[derive(Debug, Clone)]
    pub enum ImplExprAtom<'tcx> {
        /// A concrete `impl Trait for Type {}` item.
        Concrete {
            def_id: DefId,
            generics: GenericArgsRef<'tcx>,
        },
        /// A context-bound clause like `where T: Trait`.
        LocalBound {
            predicate: Predicate<'tcx>,
            r#trait: PolyTraitRef<'tcx>,
            path: Path<'tcx>,
        },
        /// The automatic clause `Self: Trait` present inside a `impl Trait for Type {}` item.
        SelfImpl {
            r#trait: PolyTraitRef<'tcx>,
            path: Path<'tcx>,
        },
        /// `dyn Trait` is a wrapped value with a virtual table for trait
        /// `Trait`.  In other words, a value `dyn Trait` is a dependent
        /// triple that gathers a type τ, a value of type τ and an
        /// instance of type `Trait`.
        /// `dyn Trait` implements `Trait` using a built-in implementation; this refers to that
        /// built-in implementation.
        Dyn,
        /// A built-in trait whose implementation is computed by the compiler, such as `Sync`.
        Builtin { r#trait: PolyTraitRef<'tcx> },
        /// An error happened while resolving traits.
        Error(String),
    }

    #[derive(Clone, Debug)]
    pub struct ImplExpr<'tcx> {
        /// The trait this is an impl for.
        pub r#trait: PolyTraitRef<'tcx>,
        /// The kind of implemention of the root of the tree.
        pub r#impl: ImplExprAtom<'tcx>,
        /// A list of `ImplExpr`s required to fully specify the trait references in `impl`.
        pub args: Vec<Self>,
    }

    impl<'tcx> ImplExprAtom<'tcx> {
        fn with_args(
            self,
            args: Vec<ImplExpr<'tcx>>,
            r#trait: PolyTraitRef<'tcx>,
        ) -> ImplExpr<'tcx> {
            ImplExpr {
                r#impl: self,
                args,
                r#trait,
            }
        }
    }

    #[tracing::instrument(level = "trace", skip(tcx, warn))]
    fn impl_exprs<'tcx>(
        tcx: TyCtxt<'tcx>,
        owner_id: DefId,
        obligations: &Vec<
            rustc_trait_selection::traits::Obligation<'tcx, rustc_middle::ty::Predicate<'tcx>>,
        >,
        warn: &impl Fn(&str),
    ) -> Result<Vec<ImplExpr<'tcx>>, String> {
        obligations
            .into_iter()
            .flat_map(|obligation| {
                obligation.predicate.as_trait_clause().map(|trait_ref| {
                    impl_expr(
                        tcx,
                        owner_id,
                        obligation.param_env,
                        &trait_ref.map_bound(|p| p.trait_ref),
                        warn,
                    )
                })
            })
            .collect()
    }

    #[tracing::instrument(level = "trace", skip(tcx, param_env, warn))]
    fn impl_expr<'tcx>(
        tcx: TyCtxt<'tcx>,
        owner_id: DefId,
        param_env: rustc_middle::ty::ParamEnv<'tcx>,
        tref: &rustc_middle::ty::PolyTraitRef<'tcx>,
        // Call back into hax-related code to display a nice warning.
        warn: &impl Fn(&str),
    ) -> Result<ImplExpr<'tcx>, String> {
        use rustc_trait_selection::traits::*;
        let impl_source = copy_paste_from_rustc::codegen_select_candidate(tcx, (param_env, *tref))
            .map_err(|e| format!("Cannot handle error `{e:?}` selecting `{tref:?}`"))?;
        Ok(match impl_source {
            ImplSource::UserDefined(ImplSourceUserDefinedData {
                impl_def_id,
                args: generics,
                nested,
            }) => ImplExprAtom::Concrete {
                def_id: impl_def_id,
                generics,
            }
            .with_args(impl_exprs(tcx, owner_id, &nested, warn)?, *tref),
            ImplSource::Param(nested) => {
                let nested = impl_exprs(tcx, owner_id, &nested, warn)?;
                let predicates = tcx.predicates_defined_on_or_above(owner_id);
                let Some((path, apred)) =
                    search_clause::path_to(tcx, &predicates, tref.clone(), param_env)
                else {
                    let msg =
                        format!("Could not find a clause for `{tref:?}` in the item parameters");
                    warn(&msg);
                    return Ok(ImplExprAtom::Error(msg).with_args(nested, *tref));
                };

                let Some(trait_clause) = apred.clause.as_trait_clause() else {
                    return Err(format!(
                        "Candidate origin for `{tref:?}` is a clause but not a \
                        trait clause: `{:?}`",
                        apred.clause
                    ));
                };
                use rustc_middle::ty::ToPolyTraitRef;
                let r#trait = trait_clause.to_poly_trait_ref();
                if apred.is_extra_self_predicate {
                    ImplExprAtom::SelfImpl { r#trait, path }
                } else {
                    ImplExprAtom::LocalBound {
                        predicate: apred.clause.as_predicate(),
                        r#trait,
                        path,
                    }
                }
                .with_args(nested, *tref)
            }
            // We ignore the contained obligations here. For example for `(): Send`, the
            // obligations contained would be `[(): Send]`, which leads to an infinite loop. There
            // might be important obligation shere inother cases; we'll have to see if that comes
            // up.
            ImplSource::Builtin(source, _ignored) => {
                let atom = match source {
                    BuiltinImplSource::Object { .. } => ImplExprAtom::Dyn,
                    _ => ImplExprAtom::Builtin {
                        r#trait: tref.clone(),
                    },
                };
                atom.with_args(vec![], *tref)
            }
        })
    }

    mod copy_paste_from_rustc {
        use rustc_infer::infer::TyCtxtInferExt;
        use rustc_middle::traits::CodegenObligationError;
        use rustc_middle::ty::{self, TyCtxt, TypeVisitableExt};
        use rustc_trait_selection::error_reporting::InferCtxtErrorExt;
        use rustc_trait_selection::traits::{
            Obligation, ObligationCause, ObligationCtxt, ScrubbedTraitError, SelectionContext,
            Unimplemented,
        };

        /// Attempts to resolve an obligation to an `ImplSource`. The result is
        /// a shallow `ImplSource` resolution, meaning that we do not
        /// (necessarily) resolve all nested obligations on the impl. Note
        /// that type check should guarantee to us that all nested
        /// obligations *could be* resolved if we wanted to.
        ///
        /// This also expects that `trait_ref` is fully normalized.
        pub fn codegen_select_candidate<'tcx>(
            tcx: TyCtxt<'tcx>,
            (param_env, trait_ref): (ty::ParamEnv<'tcx>, ty::PolyTraitRef<'tcx>),
        ) -> Result<rustc_trait_selection::traits::Selection<'tcx>, CodegenObligationError>
        {
            let trait_ref = tcx.normalize_erasing_regions(param_env, trait_ref);

            // Do the initial selection for the obligation. This yields the
            // shallow result we are looking for -- that is, what specific impl.
            let infcx = tcx.infer_ctxt().ignoring_regions().build();
            let mut selcx = SelectionContext::new(&infcx);

            let obligation_cause = ObligationCause::dummy();
            let obligation = Obligation::new(tcx, obligation_cause, param_env, trait_ref);

            let selection = match selcx.poly_select(&obligation) {
                Ok(Some(selection)) => selection,
                Ok(None) => return Err(CodegenObligationError::Ambiguity),
                Err(Unimplemented) => return Err(CodegenObligationError::Unimplemented),
                Err(e) => {
                    panic!(
                        "Encountered error `{:?}` selecting `{:?}` during codegen",
                        e, trait_ref
                    )
                }
            };

            // Currently, we use a fulfillment context to completely resolve
            // all nested obligations. This is because they can inform the
            // inference of the impl's type parameters.
            // FIXME(-Znext-solver): Doesn't need diagnostics if new solver.
            let ocx = ObligationCtxt::new(&infcx);
            let impl_source = selection.map(|obligation| {
                ocx.register_obligation(obligation.clone());
                obligation
            });

            // In principle, we only need to do this so long as `impl_source`
            // contains unbound type parameters. It could be a slight
            // optimization to stop iterating early.
            let errors = ocx.select_all_or_error();
            if !errors.is_empty() {
                // `rustc_monomorphize::collector` assumes there are no type errors.
                // Cycle errors are the only post-monomorphization errors possible; emit them now so
                // `rustc_ty_utils::resolve_associated_item` doesn't return `None` post-monomorphization.
                for err in errors {
                    if let ScrubbedTraitError::Cycle(cycle) = err {
                        infcx.err_ctxt().report_overflow_obligation_cycle(&cycle);
                    }
                }
                return Err(CodegenObligationError::FulfillmentError);
            }

            let impl_source = infcx.resolve_vars_if_possible(impl_source);
            let impl_source = infcx.tcx.erase_regions(impl_source);

            if impl_source.has_infer() {
                // Unused lifetimes on an impl get replaced with inference vars, but never resolved,
                // causing the return value of a query to contain inference vars. We do not have a concept
                // for this and will in fact ICE in stable hashing of the return value. So bail out instead.
                infcx.tcx.dcx().has_errors().unwrap();
                return Err(CodegenObligationError::FulfillmentError);
            }

            Ok(impl_source)
        }
    }
}

#[cfg(feature = "rustc")]
impl<'tcx, S: UnderOwnerState<'tcx>> SInto<S, ImplExpr>
    for rustc_middle::ty::PolyTraitPredicate<'tcx>
{
    fn sinto(&self, s: &S) -> ImplExpr {
        use rustc_middle::ty::ToPolyTraitRef;
        self.to_poly_trait_ref().sinto(s)
    }
}

/// Given a clause `clause` in the context of some impl block `impl_did`, susbts correctly `Self`
/// from `clause` and (1) derive a `Clause` and (2) resolve an `ImplExpr`.
#[cfg(feature = "rustc")]
pub fn super_clause_to_clause_and_impl_expr<'tcx, S: UnderOwnerState<'tcx>>(
    s: &S,
    impl_did: rustc_span::def_id::DefId,
    clause: rustc_middle::ty::Clause<'tcx>,
    span: rustc_span::Span,
) -> Option<(Clause, ImplExpr, Span)> {
    let tcx = s.base().tcx;
    let impl_trait_ref = tcx
        .impl_trait_ref(impl_did)
        .map(|binder| rustc_middle::ty::Binder::dummy(binder.instantiate_identity()))?;
    let original_predicate_id = {
        // We don't want the id of the substituted clause id, but the
        // original clause id (with, i.e., `Self`)
        let s = &with_owner_id(s.base(), (), (), impl_trait_ref.def_id());
        clause.sinto(s).id
    };
    let new_clause = clause.instantiate_supertrait(tcx, impl_trait_ref);
    let impl_expr = new_clause.as_predicate().as_trait_clause()?.sinto(s);
    let mut new_clause_no_binder = new_clause.sinto(s);
    new_clause_no_binder.id = original_predicate_id;
    Some((new_clause_no_binder, impl_expr, span.sinto(s)))
}
