open Base
open Utils

module%inlined_contents Make
    (F : Features.T
           with type continue = Features.Off.continue
            and type early_exit = Features.Off.early_exit) =
struct
  open Ast
  module FA = F

  let metadata = Phase_utils.Metadata.make TrivializeAssignLhs

  module FB = struct
    include F
    include Features.Off.Loop
  end

  module UA = Ast_utils.Make (F)
  module UB = Ast_utils.Make (FB)
  module A = Ast.Make (F)
  module B = Ast.Make (FB)
  include Phase_utils.DefaultError

  module S = struct
    include Features.SUBTYPE.Id
  end

  [%%inline_defs dmutability + dty + dborrow_kind + dpat]

  let rec dexpr (expr : A.expr) : B.expr =
    let span = expr.span in
    match expr.e with
    | Loop
        {
          body;
          kind = ForLoop { start; end_; var; var_typ; _ };
          state = Some { init; bpat; _ };
          _;
        } ->
        let body = dexpr body in
        let var_typ = dty span var_typ in
        let bpat = dpat bpat in
        let fn : B.expr' =
          Closure
            {
              params = [ UB.make_var_pat var var_typ span; bpat ];
              body;
              captures = [];
            }
        in
        let fn : B.expr =
          {
            e = fn;
            typ = TArrow ([ var_typ; bpat.typ ], body.typ);
            span = body.span;
          }
        in
        UB.call "dummy" "sfoldi" []
          [ dexpr start; dexpr end_; dexpr init; fn ]
          span (dty span expr.typ)
    | Loop _ ->
        Error.unimplemented
          ~details:"Only for loop are being functionalized for now" span
    | Break _ ->
        Error.unimplemented
          ~details:
            "For now, the AST node [Break] is feature gated only by [loop], \
             there is nothing for having loops but no breaks."
          span
    | [%inline_arms "dexpr'.*" - Loop - Break - Continue - Return] ->
        map (fun e -> B.{ e; typ = dty expr.span expr.typ; span = expr.span })
    | _ -> .

  and dloop_kind = [%inline_body dloop_kind]
  and dloop_state = [%inline_body dloop_state]
  and darm = [%inline_body darm]
  and darm' = [%inline_body darm']
  and dlhs = [%inline_body dlhs]

  [%%inline_defs "Item.*"]
end
[@@add "subtype.ml"]
