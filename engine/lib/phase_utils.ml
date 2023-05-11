open Base

module Metadata : sig
  type t = private {
    current_phase : Diagnostics.Phase.t;
    previous_phase : t option;
  }

  val make : Diagnostics.Phase.t -> t
  val bind : t -> t -> t
  val previous_phases : t -> Diagnostics.Phase.t list
end = struct
  type t = { current_phase : Diagnostics.Phase.t; previous_phase : t option }

  let make name = { current_phase = name; previous_phase = None }
  let bind (x : t) (y : t) : t = { y with previous_phase = Some x }

  let rec previous_phases' (p : t) : Diagnostics.Phase.t list =
    previous_phases p @ [ p.current_phase ]

  and previous_phases (p : t) : Diagnostics.Phase.t list =
    Option.map ~f:previous_phases' p.previous_phase |> Option.value ~default:[]
end

module type PHASE_ERROR = sig
  type t [@@deriving show, eq]

  val lift : t -> Diagnostics.Phase.t -> Diagnostics.t

  exception E of t
end

module DefaultError = struct
  module Error = struct
    type t = { kind : Diagnostics.kind; span : Ast.span } [@@deriving show, eq]

    let lift { kind; span } (phase : Diagnostics.Phase.t) : Diagnostics.t =
      { kind; span = Diagnostics.to_thir_span span; context = Phase phase }

    exception E of t

    let raise err = raise @@ E err

    let unimplemented ?issue_id ?details span =
      raise { kind = Unimplemented { issue_id; details }; span }
  end

  module _ : PHASE_ERROR = Error
end

module NoError = struct
  module Error = struct
    type t = | [@@deriving show, eq]

    let lift (x : t) (_phase : Diagnostics.Phase.t) : Diagnostics.t =
      match x with _ -> .

    exception E of t
  end

  module _ : PHASE_ERROR = Error
end

module type PHASE = sig
  val metadata : Metadata.t

  module FA : Features.T
  module FB : Features.T
  module A : Ast.T
  module B : Ast.T
  module Error : PHASE_ERROR

  val ditem : A.item -> B.item list
end

module Identity (F : Features.T) = struct
  module FA = F
  module FB = F
  module A = Ast.Make (F)
  module B = Ast.Make (F)
  include NoError

  let ditem (x : A.item) : B.item list = [ x ]
  let metadata = Metadata.make Diagnostics.Phase.Identity
end

module _ (F : Features.T) : PHASE = Identity (F)

(* module type PHASE_EXN = sig *)
(*   include PHASE *)

(*   type error [@@deriving show] *)

(*   exception Error of error *)
(* end *)

let _DEBUG_SHOW_ITEM = false
let _DEBUG_SHOW_BACKTRACE = false

module CatchErrors (D : PHASE) = struct
  include D

  let ditem (i : D.A.item) : D.B.item list =
    try D.ditem i
    with D.Error.E e ->
      raise @@ Diagnostics.Error (D.Error.lift e D.metadata.current_phase)
end

(* TODO: This module should disappear entierly when issue #14 is
   closed (#14: Improve/add errors in simplification phases) *)
module AddErrorHandling (D : PHASE) = struct
  include D

  exception PhaseError

  let ditem (i : D.A.item) : D.B.item list =
    try D.ditem i
    with Failure e ->
      Caml.prerr_endline
        ("Phase "
        ^ [%show: Diagnostics.Phase.t] metadata.current_phase
        ^ " failed with exception: " ^ e ^ "\nTerm: "
        ^
        if _DEBUG_SHOW_ITEM then
          [%show: A.item] i
          ^ "\n"
          ^ if _DEBUG_SHOW_BACKTRACE then Caml.Printexc.get_backtrace () else ""
        else "");
      raise PhaseError
end

module DebugPhaseInfo = struct
  type t = Before | Phase of Diagnostics.Phase.t
  [@@deriving eq, sexp, hash, compare, yojson]

  let show (s : t) : string =
    match s with
    | Before -> "initial_input"
    | Phase p -> Diagnostics.Phase.display p

  let pp (fmt : Caml.Format.formatter) (s : t) : unit =
    Caml.Format.pp_print_string fmt @@ show s
end

module DebugBindPhase : sig
  val add : DebugPhaseInfo.t -> int -> (unit -> Ast.Full.item list) -> unit
  val export : unit -> unit
  val enable : string -> unit
end = struct
  let prefix_path = ref None
  let enable (path : string) = prefix_path := Some path
  let enabled () = Option.is_some !prefix_path

  let cache : (DebugPhaseInfo.t, int * Ast.Full.item list ref) Hashtbl.t =
    Hashtbl.create (module DebugPhaseInfo)

  let add (phase_info : DebugPhaseInfo.t) (nth : int)
      (mk_item : unit -> Ast.Full.item list) =
    if enabled () then
      let _, l =
        Hashtbl.find_or_add cache phase_info ~default:(fun _ -> (nth, ref []))
      in
      l := !l @ mk_item ()
    else ()

  let export_print prefix_path =
    let files =
      Hashtbl.to_alist cache
      |> List.sort ~compare:(fun (_, (a, _)) (_, (b, _)) -> Int.compare a b)
      |> List.map ~f:(fun (k, (nth, l)) ->
             ( Printf.sprintf "%02d" nth ^ "_" ^ [%show: DebugPhaseInfo.t] k,
               String.concat ~sep:"\n\n" (List.map ~f:Print_rust.pitem !l) ))
    in
    List.iter
      ~f:(fun (path, data) ->
        Core.Out_channel.write_all ~data
        @@ [%string "%{prefix_path}/%{path}.rs"])
      files

  let export_as_json prefix_path =
    let all =
      Hashtbl.to_alist cache
      |> List.sort ~compare:(fun (_, (a, _)) (_, (b, _)) -> Int.compare a b)
      |> List.map ~f:(fun (k, (nth, l)) ->
             `Assoc
               [
                 ("name", `String ([%show: DebugPhaseInfo.t] k));
                 ("nth", `Int nth);
                 ("items", [%yojson_of: Ast.Full.item list] !l);
               ])
    in
    Core.Out_channel.write_all ~data:(`List all |> Yojson.Safe.pretty_to_string)
    @@ prefix_path ^ "/debug-circus-engine.json"

  let export () =
    match !prefix_path with
    | Some prefix_path ->
        export_print prefix_path;
        export_as_json prefix_path
    | None -> ()
end

module type S = sig
  module A : Ast.T

  val ditem : A.item -> Ast.Full.item list
end

module BindPhase
    (D1 : PHASE)
    (D2 : PHASE with module FA = D1.FB and module A = D1.B) =
struct
  module D1' = AddErrorHandling (D1)
  module D2' = AddErrorHandling (D2)
  module FA = D1.FA
  module FB = D2.FB
  module A = D1.A
  module B = D2.B

  module Error = struct
    type t = ErrD1 of D1.Error.t | ErrD2 of D2.Error.t [@@deriving show, eq]

    let lift (x : t) (_phase : Diagnostics.Phase.t) : Diagnostics.t =
      match x with
      | ErrD1 e -> D1.Error.lift e D1.metadata.current_phase
      | ErrD2 e -> D2.Error.lift e D2.metadata.current_phase

    exception E of t
  end

  module _ : PHASE_ERROR = Error

  let metadata = Metadata.bind D1.metadata D2.metadata

  let ditem : A.item -> B.item list =
   fun item0 ->
    let nth = List.length @@ Metadata.previous_phases D1.metadata in
    (if Int.equal nth 0 then
     let coerce_to_full_ast : D1'.A.item -> Ast.Full.item = Caml.Obj.magic in
     DebugBindPhase.add Before 0 (fun _ -> [ coerce_to_full_ast item0 ]));
    let item1 =
      try D1'.ditem item0
      with D1.Error.E e -> raise @@ Error.E (Error.ErrD1 e)
    in
    let coerce_to_full_ast : D2'.A.item list -> Ast.Full.item list =
      Caml.Obj.magic
    in
    DebugBindPhase.add (Phase D1.metadata.current_phase) (nth + 1) (fun _ ->
        coerce_to_full_ast item1);
    let item2 =
      try List.concat_map ~f:D2'.ditem item1
      with D2.Error.E e -> raise @@ Error.E (Error.ErrD2 e)
    in
    item2
end
