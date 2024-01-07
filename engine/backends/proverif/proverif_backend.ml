open Hax_engine
open Utils
open Base

include
  Backend.Make
    (struct
      open Features
      include Off
      include On.Macro
      include On.Struct_pattern
    end)
    (struct
      let backend = Diagnostics.Backend.ProVerif
    end)

module SubtypeToInputLanguage
    (FA : Features.T
    (*   with *)
    (*  type loop = Features.Off.loop *)
    (* and type for_loop = Features.Off.for_loop *)
    (* and type for_index_loop = Features.Off.for_index_loop *)
    (* and type state_passing_loop = Features.Off.state_passing_loop *)
    (* and type continue = Features.Off.continue *)
    (* and type break = Features.Off.break *)
    (* and type mutable_variable = Features.Off.mutable_variable *)
    (* and type mutable_reference = Features.Off.mutable_reference *)
    (* and type mutable_pointer = Features.Off.mutable_pointer *)
    (* and type reference = Features.Off.reference *)
    (* and type slice = Features.Off.slice *)
    (* and type raw_pointer = Features.Off.raw_pointer *)
    (* and type early_exit = Features.Off.early_exit *)
    (* and type question_mark = Features.Off.question_mark *)
    (* and type macro = Features.On.macro *)
    (* and type as_pattern = Features.Off.as_pattern *)
    (* and type nontrivial_lhs = Features.Off.nontrivial_lhs *)
    (* and type arbitrary_lhs = Features.Off.arbitrary_lhs *)
    (* and type lifetime = Features.Off.lifetime *)
    (* and type construct_base = Features.Off.construct_base *)
    (* and type monadic_action = Features.Off.monadic_action *)
    (* and type monadic_binding = Features.Off.monadic_binding *)
    (* and type block = Features.Off.block *)) =
struct
  module FB = InputLanguage

  include
    Feature_gate.Make (FA) (FB)
      (struct
        module A = FA
        module B = FB
        include Feature_gate.DefaultSubtype

        let continue = reject
        let loop = reject
        let struct_pattern _ _ = Features.On.struct_pattern
        let for_loop = reject
        let for_index_loop = reject
        let state_passing_loop = reject
        let continue = reject
        let break = reject
        let mutable_variable = reject
        let mutable_reference = reject
        let mutable_pointer = reject
        let reference = reject
        let slice = reject
        let raw_pointer = reject
        let early_exit = reject
        let question_mark = reject
        let macro = reject
        let as_pattern = reject
        let nontrivial_lhs = reject
        let arbitrary_lhs = reject
        let lifetime = reject
        let construct_base = reject
        let monadic_action = reject
        let monadic_binding = reject
        let block = reject
        let metadata = Phase_reject.make_metadata (NotInBackendLang ProVerif)
      end)

  let metadata = Phase_utils.Metadata.make (Reject (NotInBackendLang backend))
end

module BackendOptions = Backend.UnitBackendOptions
open Ast

module ProVerifNamePolicy = struct
  include Concrete_ident.DefaultNamePolicy

  [@@@ocamlformat "disable"]

  let index_field_transform index = Fn.id index

  let reserved_words = Hash_set.of_list (module String) [
  "among"; "axiom"; "channel"; "choice"; "clauses"; "const"; "def"; "diff"; "do"; "elimtrue"; "else"; "equation"; "equivalence"; "event"; "expand"; "fail"; "for"; "forall"; "foreach"; "free"; "fun"; "get"; "if"; "implementation"; "in"; "inj-event"; "insert"; "lemma"; "let"; "letfun"; "letproba"; "new"; "noninterf"; "noselect"; "not"; "nounif"; "or"; "otherwise"; "out"; "param"; "phase"; "pred"; "proba"; "process"; "proof"; "public vars"; "putbegin"; "query"; "reduc"; "restriction"; "secret"; "select"; "set"; "suchthat"; "sync"; "table"; "then"; "type"; "weaksecret"; "yield"
  ]

  let field_name_transform ~struct_name field_name = struct_name ^ "_" ^ field_name

  let enum_constructor_name_transform ~enum_name constructor_name = enum_name ^ "_" ^ constructor_name ^ "_c"

  let struct_constructor_name_transform constructor_name =  constructor_name ^ "_c"
end

module U = Ast_utils.MakeWithNamePolicy (InputLanguage) (ProVerifNamePolicy)
open AST

module Print = struct
  module GenericPrint =
    Generic_printer.Make (InputLanguage) (U.Concrete_ident_view)

  open Generic_printer_base.Make (InputLanguage)
  open PPrint

  let iblock f = group >> jump 2 0 >> terminate (break 0) >> f >> group

  class print =
    object (print)
      inherit GenericPrint.print as super
      method ty_bool = string "bool"
      method ty_int _ = string "bitstring"

      method! item' item =
        let fun_and_reduc base_name constructor =
          let field_prefix =
            if constructor.is_record then empty
            else print#concrete_ident base_name
          in
          let fun_args = constructor.arguments in
          let fun_args_full =
            separate_map
              (comma ^^ break 1)
              (fun (x, y, _z) ->
                print#concrete_ident x ^^ string ": " ^^ print#ty_at Param_typ y)
              fun_args
          in
          let fun_args_names =
            separate_map
              (comma ^^ break 1)
              (fst3 >> fun x -> print#concrete_ident x)
              fun_args
          in
          let fun_args_types =
            separate_map
              (comma ^^ break 1)
              (snd3 >> print#ty_at Param_typ)
              fun_args
          in
          let constructor_name = print#concrete_ident constructor.name in

          let fun_line =
            string "fun" ^^ space ^^ constructor_name
            ^^ iblock parens fun_args_types
            ^^ string ": "
            ^^ print#concrete_ident base_name
            ^^ dot
          in
          let reduc_line =
            string "reduc forall " ^^ iblock Fn.id fun_args_full ^^ semi
          in
          let build_accessor (ident, ty, attr) =
            string "accessor" ^^ underscore ^^ print#concrete_ident ident
            ^^ iblock parens (constructor_name ^^ iblock parens fun_args_names)
            ^^ blank 1 ^^ equals ^^ blank 1 ^^ print#concrete_ident ident
          in
          let reduc_lines =
            separate_map (dot ^^ hardline)
              (fun arg -> reduc_line ^^ nest 4 (hardline ^^ build_accessor arg))
              fun_args
          in
          fun_line ^^ hardline ^^ reduc_lines
          ^^ if reduc_lines == empty then empty else dot
        in
        match item with
        (* `fn`s are transformed into `letfun` process macros. *)
        | Fn { name; generics; body; params } ->
            let params_string =
              iblock parens (separate_map (comma ^^ break 1) print#param params)
            in
            string "letfun" ^^ space
            ^^ align
                 (print#concrete_ident name ^^ params_string ^^ space ^^ equals
                ^^ hardline
                 ^^ print#expr_at Item_Fn_body body
                 ^^ dot)
        (* `struct` definitions are transformed into simple constructors and `reduc`s for accessing fields. *)
        | Type { name; generics; variants; is_struct } ->
            let type_line =
              string "type " ^^ print#concrete_ident name ^^ dot
            in
            let type_converter_line =
              string "fun " ^^ print#concrete_ident name
              ^^ string "_to_bitstring"
              ^^ iblock parens (print#concrete_ident name)
              ^^ string ": bitstring [typeConverter]."
            in
            if is_struct then
              let struct_constructor = List.hd variants in
              match struct_constructor with
              | None -> empty
              | Some constructor ->
                  type_line ^^ hardline ^^ type_converter_line ^^ hardline
                  ^^ fun_and_reduc name constructor
            else
              type_line ^^ hardline ^^ type_converter_line ^^ hardline
              ^^ separate_map (hardline ^^ hardline)
                   (fun variant -> fun_and_reduc name variant)
                   variants
        | _ -> empty

      method! expr_let : lhs:pat -> rhs:expr -> expr fn =
        fun ~lhs ~rhs body ->
          string "let" ^^ space
          ^^ iblock Fn.id (print#pat_at Expr_Let_lhs lhs)
          ^^ space ^^ equals ^^ space
          ^^ iblock Fn.id (print#expr_at Expr_Let_rhs rhs)
          ^^ space ^^ string "in" ^^ hardline
          ^^ (print#expr_at Expr_Let_body body |> group)

      method! doc_construct_inductive
          : is_record:bool ->
            is_struct:bool ->
            constructor:concrete_ident ->
            base:document option ->
            (global_ident * document) list fn =
        fun ~is_record ~is_struct:_ ~constructor ~base:_ args ->
          if is_record then
            string "t_"
            (* FIXME: How do I get at the ident from the struct definition instead? *)
            ^^ print#concrete_ident constructor
            ^^ iblock parens
                 (separate_map
                    (break 0 ^^ comma)
                    (fun (field, body) -> iblock Fn.id body |> group)
                    args)
          else
            print#concrete_ident constructor
            ^^ iblock parens (separate_map (break 0) snd args)

      method ty : Generic_printer_base.par_state -> ty fn =
        fun ctx ty ->
          match ty with
          | TBool -> print#ty_bool
          | TParam i -> print#local_ident i
          | TApp _ -> super#ty ctx ty
          | _ -> string "bitstring"

      method! expr_app : expr -> expr list -> generic_value list fn =
        fun f args _generic_args ->
          let dummy_fn =
            match List.length args with
            | n when n < 8 -> string "dummy_fn_" ^^ PPrint.OCaml.int n
            | _ ->
                Error.raise
                  {
                    kind =
                      ExplicitRejection
                        { reason = "Unsupported function arity." };
                    span = current_span;
                  }
          in
          let args =
            separate_map
              (comma ^^ break 1)
              (print#expr_at Expr_App_arg >> group)
              args
          in
          let f = print#expr_at Expr_App_f f |> group in
          f ^^ iblock parens args

      method! literal : Generic_printer_base.literal_ctx -> literal fn =
        fun _ctx -> function
          | Int { value; negative; _ } ->
              string "int2bitstring"
              ^^ iblock parens
                   (string value |> precede (if negative then minus else empty))
          | Bool b -> OCaml.bool b
          | _ ->
              Error.raise
                {
                  kind =
                    ExplicitRejection
                      { reason = "Literal unsupported by ProVerif backend." };
                  span = current_span;
                }
    end

  include Api (struct
    let new_print () = (new print :> print_object)
  end)
end

(* Insert a (empty, for now) top level process. *)
let insert_top_level contents = contents ^ "\n\nprocess\n    0\n"

(* Insert ProVerif code that will be necessary in any development.*)
let insert_preamble contents =
  "channel c.\n\
   type state.\n\
   fun int2bitstring(nat): bitstring.\n\
   fun dummy_fn_0(): bitstring.\n\
   fun dummy_fn_1(bitstring): bitstring.\n\
   fun dummy_fn_2(bitstring, bitstring): bitstring.\n\
   fun dummy_fn_3(bitstring, bitstring, bitstring): bitstring.\n\
   fun dummy_fn_4(bitstring, bitstring, bitstring, bitstring): bitstring.\n\
   fun dummy_fn_5(bitstring, bitstring, bitstring, bitstring, bitstring): \
   bitstring.\n\
   fun dummy_fn_6(bitstring, bitstring, bitstring, bitstring, bitstring, \
   bitstring): bitstring.\n\
   fun dummy_fn_7(bitstring, bitstring, bitstring, bitstring, bitstring, \
   bitstring, bitstring): bitstring.\n" ^ contents

let is_process_read : attrs -> bool =
  Attr_payloads.payloads >> List.exists ~f:(fst >> [%matches? Types.ProcessRead])

let is_process_write : attrs -> bool =
  Attr_payloads.payloads
  >> List.exists ~f:(fst >> [%matches? Types.ProcessWrite])

let is_process_init : attrs -> bool =
  Attr_payloads.payloads >> List.exists ~f:(fst >> [%matches? Types.ProcessInit])

let translate m (bo : BackendOptions.t) (items : AST.item list) :
    Types.file list =
  let processes, rest =
    List.partition_tf
      ~f:(fun item -> [%matches? Fn _] item.v && is_process_read item.attrs)
      items
  in
  let letfuns, rest =
    List.partition_tf ~f:(fun item -> [%matches? Fn _] item.v) rest
  in
  let letfun_content, _ = Print.items letfuns in
  let process_content, _ = Print.items processes in
  let contents, _ = Print.items rest in
  let contents =
    contents ^ "\n(* Process Macros *)\n\n" ^ letfun_content
    ^ "\n(* Processes *)" ^ process_content
    |> insert_top_level |> insert_preamble
  in
  let file = Types.{ path = "output.pv"; contents } in
  [ file ]

open Phase_utils
module DepGraph = Dependencies.Make (InputLanguage)
module DepGraphR = Dependencies.Make (Features.Rust)

module TransformToInputLanguage =
  [%functor_application
  Phases.Reject.RawOrMutPointer(Features.Rust)
  |> Phases.And_mut_defsite
  |> Phases.Reconstruct_for_loops
  |> Phases.Direct_and_mut
  |> Phases.Reject.Arbitrary_lhs
  |> Phases.Drop_blocks
  |> Phases.Drop_references
  |> Phases.Trivialize_assign_lhs
  |> Phases.Reconstruct_question_marks
  |> Side_effect_utils.Hoist
  |> Phases.Local_mutation
  |> Phases.Reject.Continue
  |> Phases.Cf_into_monads
  |> Phases.Reject.EarlyExit
  |> Phases.Functionalize_loops
  |> Phases.Reject.As_pattern
  |> SubtypeToInputLanguage
  |> Identity
  ]
  [@ocamlformat "disable"]

let apply_phases (bo : BackendOptions.t) (items : Ast.Rust.item list) :
    AST.item list =
  TransformToInputLanguage.ditems items
