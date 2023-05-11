open Base
open Ppxlib

let name = "inlined_contents"

let cons_lid_of_pattern (p : pattern) =
  match p.ppat_desc with
  | Ppat_construct ({ txt; _ }, _) -> Some txt
  | _ -> None

let name_of_pattern (p : pattern) =
  match p.ppat_desc with Ppat_var { txt; _ } -> Some txt | _ -> None

type inlinable_item_kind =
  | MatchCase of (case[@opaque])
  | Binding of (value_binding[@opaque])
  | StrItem of (structure_item[@opaque])
[@@deriving show]

type inlinable_item = { path : string list; kind : inlinable_item_kind }
[@@deriving show]

let collect_ast_nodes (result : inlinable_item list ref) =
  let add (l : inlinable_item list) = result := !result @ l in
  object
    inherit [string list] Ast_traverse.map_with_context as super

    method! module_binding path x =
      let path =
        match x.pmb_name.txt with Some name -> path @ [ name ] | None -> path
      in
      super#module_binding path x

    method! value_binding path x =
      let path =
        match name_of_pattern x.pvb_pat with
        | Some name ->
            let path = path @ [ name ] in
            add @@ [ { path; kind = Binding x } ];
            path
        | None -> path
      in
      super#value_binding path x

    method! structure_item path s =
      (match s.pstr_desc with
      | Pstr_value (_, bindings) ->
          List.iter bindings ~f:(fun { pvb_pat; _ } ->
              match name_of_pattern pvb_pat with
              | Some n -> add [ { path = path @ [ n ]; kind = StrItem s } ]
              | _ -> ())
      | Pstr_type (_, bindings) ->
          List.iter bindings ~f:(fun { ptype_name = { txt = n; _ }; _ } ->
              add [ { path = path @ [ n ]; kind = StrItem s } ])
      | _ -> ());
      super#structure_item path s

    method! expression path e =
      let e' = super#expression path e in
      match e.pexp_desc with
      | Pexp_match (_, cases) ->
          add
          @@ List.filter_map
               ~f:(fun case ->
                 match cons_lid_of_pattern case.pc_lhs with
                 | Some chunk ->
                     Some
                       {
                         path = path @ [ Longident.last_exn chunk ];
                         kind = MatchCase case;
                       }
                 | None -> None)
               cases;
          e'
      | _ -> e'
  end

let replace_every_location (location : location) =
  object
    inherit Ast_traverse.map
    method! location = Fn.const location
  end

let locate_module (name : string) : string =
  let rec find = function
    | path when Caml.Sys.is_directory path ->
        Caml.Sys.readdir path
        |> Array.find_map ~f:(fun name ->
               find @@ Caml.Filename.concat path name)
    | path when String.(Caml.Filename.basename path = name) -> Some path
    | _ -> None
  in
  find (Caml.Sys.getcwd ())
  |> Option.value_exn ~message:("ppx_inbline: could not locate module " ^ name)

let inlinable_items_of_module : loc:location -> string -> inlinable_item list =
  let memo = Hashtbl.create (module String) in
  fun ~loc path ->
    Hashtbl.find_or_add memo
      ~default:(fun () ->
        let results = ref [] in
        let _ =
          locate_module path |> Caml.open_in |> Lexing.from_channel
          |> Parse.implementation |> (replace_every_location loc)#structure
          |> (collect_ast_nodes results)#structure [ path ]
        in
        !results)
      path

let inlinable_items_of_modules ~loc : string list -> inlinable_item list =
  List.concat_map ~f:(inlinable_items_of_module ~loc)

type flag = Include | Exclude [@@deriving show]

type inline_error =
  | NotFound of { search : string list; available : string list list }
  | NotPlusMinusList
[@@deriving show]

exception InlineError of inline_error

let raise_inline_err x = raise @@ InlineError x

let rec plus_minus_list_of_expr' (e : expression) : (flag * string list) list =
  match e with
  | [%expr [%e? x] + [%e? y]] ->
      plus_minus_list_of_expr' x @ plus_minus_list_of_expr' y
  | [%expr [%e? x] - [%e? y]] ->
      plus_minus_list_of_expr' x
      @ List.map ~f:(fun (_, v) -> (Exclude, v))
      @@ plus_minus_list_of_expr' y
  | { pexp_desc = Pexp_constant (Pconst_string (s, _, _)); _ } ->
      [ (Include, String.split ~on:'.' s) ]
  | { pexp_desc = Pexp_ident { txt; _ }; _ }
  | { pexp_desc = Pexp_construct ({ txt; _ }, _); _ } ->
      [ (Include, Longident.flatten_exn txt) ]
  | _ -> raise_inline_err NotPlusMinusList

let plus_minus_list_of_expr (e : expression) : (flag * string list) list option
    =
  try Some (plus_minus_list_of_expr' e)
  with InlineError NotPlusMinusList -> None

let elast l =
  match (List.last l, List.drop_last l) with
  | Some last, Some init -> Some (init, last)
  | _ -> None

let diff_list (type a) (x : a list) (y : a list) ~(equal : a -> a -> bool) :
    a list =
  List.filter
    ~f:(fun elem_x ->
      List.for_all ~f:(fun elem_y -> not @@ equal elem_x elem_y) y)
    x

(* TODO: ppx_inline reports badly locations (I actually don't use `_loc`...) *)
let map_inline_nodes opens _loc =
  let rec match_glob (glob : string list) (against : string list) =
    match (elast glob, elast against) with
    | Some (glob, "*"), Some (against, _) -> match_glob glob against
    | _ -> List.is_suffix ~equal:String.equal ~suffix:glob against
  in
  let inlinable_items = inlinable_items_of_modules opens in
  let matches ~loc (glob : string list) : inlinable_item list =
    List.filter ~f:(fun { path; _ } -> match_glob glob path)
    @@ inlinable_items ~loc
  in
  let find_one (type a) ~loc (glob : string list)
      (f : inlinable_item_kind -> a option) : (string list * a) list =
    match
      List.filter_map ~f:(fun { path; kind } ->
          Option.map ~f:(fun r -> (path, r)) @@ f kind)
      @@ matches glob ~loc
    with
    | [] ->
        raise_inline_err
        @@ NotFound
             {
               search = glob;
               available =
                 List.map ~f:(fun { path; _ } -> path) @@ inlinable_items ~loc;
             }
    | l -> l
  in
  let find (type a) ~loc (flags : (flag * string list) list)
      (f : inlinable_item_kind -> a option) =
    List.fold_left ~init:[]
      ~f:(fun acc (flag, path) ->
        let matches = find_one ~loc path f in
        match flag with
        | Include -> acc @ matches
        | Exclude ->
            diff_list
              ~equal:(fun (x, _) (y, _) -> [%eq: string list] x y)
              acc matches)
      flags
    |> List.map ~f:snd
  in

  object
    inherit Ast_traverse.map as super

    method! structure e =
      let e = super#structure e in
      let each_item e =
        let loc = e.pstr_loc in
        match e.pstr_desc with
        | Pstr_extension
            ( ( { txt = "inline_defs"; _ },
                PStr [ { pstr_desc = Pstr_eval (payload, _); _ } ] ),
              _ ) -> (
            match plus_minus_list_of_expr payload with
            | Some opts -> (
                try find ~loc opts (function StrItem x -> Some x | _ -> None)
                with InlineError err ->
                  let err =
                    [%show: inline_error] err
                    |> Ast_builder.Default.estring ~loc
                  in
                  [%str [%ocaml.error [%e err]]])
            | _ -> [ e ])
        | _ -> [ e ]
      in
      List.concat_map ~f:each_item e

    method! expression e =
      let e = super#expression e in
      let loc = e.pexp_loc in
      match e with
      | { pexp_desc = Pexp_match (scrut, cases); _ } ->
          let cases =
            List.concat_map
              ~f:(fun case ->
                match case.pc_lhs with
                | [%pat? [%inline_arms [%e? e]]] -> (
                    let pc_rhs_map =
                      match case.pc_rhs with
                      | [%expr map [%e? f]] -> fun e -> [%expr [%e f] [%e e]]
                      | _ -> Fn.id
                    in
                    match plus_minus_list_of_expr e with
                    | Some opts -> (
                        try
                          find ~loc opts (function
                            | MatchCase case -> Some case
                            | _ -> None)
                          |> List.map ~f:(fun case ->
                                 { case with pc_rhs = pc_rhs_map case.pc_rhs })
                        with InlineError err ->
                          let err =
                            [%show: inline_error] err
                            |> Ast_builder.Default.estring ~loc
                          in
                          [
                            {
                              case with
                              pc_lhs = [%pat? [%ocaml.error [%e err]]];
                            };
                          ])
                    | None -> [ case ])
                | _ -> [ case ])
              cases
          in
          { e with pexp_desc = Pexp_match (scrut, cases) }
      | [%expr [%inline_body [%e? e]]] -> (
          match plus_minus_list_of_expr e with
          | Some opts -> (
              try
                match
                  find ~loc opts (function
                    | Binding { pvb_expr; _ } -> Some pvb_expr
                    | _ -> None)
                with
                | [ x ] -> x
                | _ -> failwith "inline_body: matched multiple"
              with InlineError err ->
                let err =
                  [%show: inline_error] err |> Ast_builder.Default.estring ~loc
                in
                [%expr [%ocaml.error [%e err]]])
          | None -> e)
      | _ -> e
  end

let attributes_of_structure_item (str : structure_item) =
  match str.pstr_desc with
  | Pstr_module { pmb_attributes = attrs; _ } | Pstr_eval (_, attrs) -> attrs
  | _ -> failwith "attributes_of_structure_item: not implemented"

let string_of_payload ~loc e =
  Ast_pattern.(
    parse_res
    @@ pstr
         (pstr_eval (pexp_constant @@ pconst_string __ drop drop) drop ^:: nil))
    loc e Fn.id

let expand ~(ctxt : Expansion_context.Extension.t) (str : structure_item) :
    structure_item =
  let loc = Expansion_context.Extension.extension_point_loc ctxt in
  let attrs =
    attributes_of_structure_item str
    |> List.filter_map ~f:(fun attr ->
           match string_of_payload ~loc attr.attr_payload with
           | Result.Ok payload -> Some (attr.attr_name.txt, payload)
           | _ -> None)
  in
  let opens =
    List.filter_map
      ~f:(fun (name, path) ->
        if String.equal name "add" then Some path else None)
      attrs
  in
  (map_inline_nodes opens loc)#structure_item str

let ext =
  Extension.V3.declare name Extension.Context.structure_item
    Ast_pattern.(pstr (__ ^:: nil))
    expand

let rule = Ppxlib.Context_free.Rule.extension ext
let () = Ppxlib.Driver.register_transformation ~rules:[ rule ] name
