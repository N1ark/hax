module Rust_primitives.Integers

open FStar.Mul

module LI = Lib.IntTypes

#set-options "--max_fuel 0 --max_ifuel 1 --z3rlimit 20"

val pow2_values: x:nat -> Lemma
  (let p = pow2 x in
   match x with
   | 0  -> p=1
   | 1  -> p=2
   | 8  -> p=256
   | 16 -> p=65536
   | 31 -> p=2147483648
   | 32 -> p=4294967296
   | 63 -> p=9223372036854775808
   | 64 -> p=18446744073709551616
   | 2 | 3 | 4 | 5 | 6 | 7
   | 9 | 10 | 11 | 12 | 13 | 14 | 15 
   | 17 | 18 | 19 | 20 | 21 | 22 | 23
   | 24 | 25 | 26 | 27 | 28 | 29 | 30
   | 33 | 34 | 35 | 36 | 37 | 38 | 39
   | 40 | 41 | 42 | 43 | 44 | 45 | 46
   | 47 | 48 | 49 | 50 | 51 | 52 | 53
   | 54 | 55 | 56 | 57 | 58 | 59 | 60
   | 61 | 62 | 65 -> p = normalize_term (pow2 x)
   | _ -> True)
  [SMTPat (pow2 x)]

type inttype = LI.inttype
let unsigned = LI.unsigned
let signed = LI.signed
type uinttype = t:inttype{unsigned t}
let int_t t = LI.int_t t LI.PUB

let bits t = LI.bits t
val usize_inttype: t:inttype{unsigned t /\ (t = LI.U32 \/ t = LI.U64)}
val isize_inttype: t:inttype{signed t /\ (t = LI.S32 \/ t = LI.S64)}

type u8 = int_t LI.U8 
type i8 = int_t LI.S8
type u16 = int_t LI.U16
type i16 = int_t LI.S16
type u32 = int_t LI.U32
type i32 = int_t LI.S32
type u64 = int_t LI.U64
type i64=  int_t LI.S64
type u128 = int_t LI.U128
type i128 = int_t LI.S128
type usize = int_t usize_inttype
type isize = int_t isize_inttype

let minint (t:LI.inttype) =
  if unsigned t then 0 else -(pow2 (bits t - 1))
let maxint (t:LI.inttype) =
  if unsigned t then pow2 (bits t) - 1
  else pow2 (bits t - 1) - 1
let modulus (t:LI.inttype) = pow2 (bits t)

let max_usize = maxint usize_inttype
let max_isize = maxint isize_inttype

let range_bits (n:int) (n:bits) : bool =
  minint t <= n && n <= maxint t

let range (n:int) (t:inttype) : bool =
  minint t <= n && n <= maxint t
type range_t (t:inttype) = x:int{range x t}

[@(strict_on_arguments [0])]
let v (#t:inttype) (x:int_t t) : range_t t = LI.v #t #LI.PUB x

[@(strict_on_arguments [0])]
val mk_int (#t:inttype) (n:range_t t) : int_t t

let sz (n:range_t usize_inttype) : usize = mk_int n
let isz (n:range_t isize_inttype) : isize = mk_int n

val mk_int_v_lemma: #t:inttype -> a:int_t t -> Lemma
  (mk_int #t (v #t a) == a)
  [SMTPat (mk_int #t (v #t a))]

val v_mk_int_lemma: #t:inttype -> n:range_t t -> Lemma
  (v #t (mk_int #t n) == n)
  [SMTPat (v #t (mk_int #t n))]

(* Wrap-around modulo: wraps into [-p/2; p/2[ *)
let op_At_Percent (v:int) (p:int{p>0/\ p%2=0}) : Tot int =
  let m = v % p in if m >= p/2 then m - p else m

[@(strict_on_arguments [0])]
let op_At_Percent_Dot x t : range_t t =
  if unsigned t then x % modulus t
  else x @% modulus t

let cast (#t:inttype) (#t':inttype)
    (u1:int_t t{range (v u1) t'}) =
    mk_int #t' (v u1)

let cast_mod (#t:inttype) (#t':inttype)
    (u1:int_t t) = 
    mk_int #t' (v u1 @%. t')

/// Arithmetic operations
/// 
let add_mod (#t:inttype) (a:int_t t) (b:int_t t) =
    mk_int #t ((v a + v b) @%. t)

val add_mod_equiv_lemma: #t:uinttype
  -> a:int_t t
  -> b:int_t t
  -> Lemma
    (add_mod a b == LI.add_mod #t #LI.PUB a b)

let add (#t:inttype) (a:int_t t)
        (b:int_t t{range (v a + v b) t}) =
    mk_int #t (v a + v b)

val add_equiv_lemma: #t:uinttype
  -> a:int_t t
  -> b:int_t t{range (v a + v b) t}
  -> Lemma
    (add a b == LI.add #t #LI.PUB a b)

let incr (#t:inttype) (a:int_t t{v a < maxint t}) =
    mk_int #t (v a + 1)

val incr_equiv_lemma: #t:inttype
  -> a:int_t t{v a < maxint t}
  -> Lemma (incr a == LI.incr #t #LI.PUB a)

let mul_mod (#t:inttype) (a:int_t t)
            (b:int_t t) =
            mk_int #t (v a * v b @%. t)

val mul_mod_equiv_lemma: #t:uinttype{not (LI.U128? t)}
  -> a:int_t t
  -> b:int_t t
  -> Lemma (mul_mod a b == LI.mul_mod #t #LI.PUB a b)

let mul (#t:inttype) (a:int_t t)
        (b:int_t t{range (v a * v b) t}) =
        mk_int #t (v a * v b)

val mul_equiv_lemma: #t:uinttype{not (LI.U128? t)}
  -> a:int_t t
  -> b:int_t t{range (v a * v b) t}
  -> Lemma (mul a b == LI.mul #t #LI.PUB a b)

let sub_mod (#t:inttype) (a:int_t t) (b:int_t t) =
    mk_int #t ((v a - v b) @%. t)

val sub_mod_equiv_lemma: #t:uinttype
  -> a:int_t t
  -> b:int_t t
  -> Lemma
    (sub_mod a b == LI.sub_mod #t #LI.PUB a b)

let sub (#t:inttype) (a:int_t t)
        (b:int_t t{range (v a - v b) t}) =
    mk_int #t (v a - v b)

val sub_equiv_lemma: #t:uinttype
  -> a:int_t t
  -> b:int_t t{range (v a - v b) t}
  -> Lemma
    (sub a b == LI.sub #t #LI.PUB a b)

let decr (#t:inttype) (a:int_t t{minint t < v a}) =
    mk_int #t (v a - 1)

val decr_equiv_lemma: #t:inttype
  -> a:int_t t{minint t < v a}
  -> Lemma (decr a == LI.decr #t #LI.PUB a)

let div (#t:inttype) (a:int_t t) (b:int_t t{v b <> 0}) =
  assume (range (v a / v b) t);
  mk_int #t (v a / v b)
  
val div_equiv_lemma: #t:inttype{~(LI.U128? t) /\ ~(LI.S128? t)}
  -> a:int_t t
  -> b:int_t t{v b <> 0 /\ (unsigned t \/ range FStar.Int.(v a / v b) t)}
  -> Lemma (div a b == LI.div a b)

let mod (#t:inttype) (a:int_t t) (b:int_t t{v b <> 0}) =
  mk_int #t (v a % v b)


val mod_equiv_lemma: #t:inttype{~(LI.U128? t) /\ ~(LI.S128? t)}
  -> a:int_t t
  -> b:int_t t{v b <> 0 /\ (unsigned t \/ range FStar.Int.(v a / v b) t)}
  -> Lemma (mod a b == LI.mod a b)
  

/// Comparison Operators
/// 
let eq (#t:inttype) (a:int_t t) (b:int_t t) = v a = v b
let ne (#t:inttype) (a:int_t t) (b:int_t t) = v b <> v b
let lt (#t:inttype) (a:int_t t) (b:int_t t) = v a < v b
let lte (#t:inttype) (a:int_t t) (b:int_t t) = v a <= v b
let gt (#t:inttype) (a:int_t t) (b:int_t t) = v a > v b
let gte (#t:inttype) (a:int_t t) (b:int_t t) = v a >= v b


/// Bitwise Operations


let ones (#t:inttype) : n:int_t t =
  if unsigned t then mk_int #t (pow2 (bits t) - 1)
  else mk_int #t (-1)

let zero (#t:inttype) : n:int_t t =
  mk_int #t 0

val lognot: #t:inttype -> int_t t -> int_t t
val lognot_lemma: #t:inttype -> a:int_t t -> Lemma
  (lognot a == LI.lognot #t #LI.PUB a /\
   lognot #t zero == ones /\
   lognot #t ones == zero /\
   lognot (lognot a) == a)

val logxor: #t:inttype
  -> int_t t
  -> int_t t
  -> int_t t
val logxor_lemma: #t:inttype -> a:int_t t -> b:int_t t -> Lemma
  (logxor a b == LI.logxor #t #LI.PUB a b /\
   a `logxor` (a `logxor` b) == b /\
   a `logxor` (b `logxor` a) == b /\
   a `logxor` zero == a /\
   a `logxor` ones == lognot a)
    
val logand: #t:inttype
  -> int_t t
  -> int_t t
  -> int_t t

val logand_lemma: #t:inttype -> a:int_t t -> b:int_t t ->
  Lemma (logand a b == LI.logand #t #LI.PUB a b /\
         logand a zero == zero /\
         logand a ones == a)

val logand_mask_lemma: #t:uinttype
  -> a:int_t t
  -> m:pos{m < bits t} ->
  Lemma (pow2 m < maxint t /\
         logand a (sub_mod #t (mk_int #t (pow2 m)) (mk_int #t 1)) ==
         mk_int (v a % pow2 m))
  [SMTPat (logand #t a (sub_mod #t (mk_int #t (pow2 m)) (mk_int #t 1)))]

val logor: #t:inttype
  -> int_t t
  -> int_t t
  -> int_t t

val logor_lemma: #t:inttype -> a:int_t t -> b:int_t t ->
  Lemma (logor a b == LI.logor #t #LI.PUB a b /\
         logor a zero == a /\
         logor a ones == ones)

unfold type shiftval (t:inttype) (t':inttype) =
     b:int_t t'{v b >= 0 /\ v b < bits t}
unfold type rotval (t:inttype) (t':inttype) =
     b:int_t t'{v b > 0 /\ v b < bits t}

let shift_right (#t:inttype) (#t':inttype)
    (a:int_t t) (b:shiftval t t') =
    LI.shift_right_lemma #t #LI.PUB a (LI.size (v b));
    mk_int #t (v a / pow2 (v b))

val shift_right_equiv_lemma: #t:inttype -> #t':inttype
  -> a:int_t t -> b:shiftval t t'
  -> Lemma
    (v (cast b <: u32) < bits t /\
     shift_right #t #t' a b ==
     LI.shift_right #t #LI.PUB a (cast b))
     
let shift_left (#t:inttype) (#t':inttype)
    (a:int_t t{v a >= 0}) (b:shiftval t t') =
    let x:range_t t = (v a * pow2 (v b)) @%. t in
    mk_int #t x

val shift_left_equiv_lemma: #t:inttype -> #t':inttype
  -> a:int_t t -> b:shiftval t t'
  -> Lemma
    ((v a >= 0 /\ v a * pow2 (v b) <= maxint t) ==>
     (v (cast b <: u32) < bits t /\
      shift_left #t #t' a b ==
      LI.shift_left #t #LI.PUB a (cast b)))

val rotate_right: #t:uinttype -> #t':inttype
  -> a:int_t t
  -> rotval t t'
  -> int_t t

val rotate_right_equiv_lemma: #t:uinttype -> #t':inttype
  -> a:int_t t -> b:rotval t t'
  -> Lemma (v (cast b <: u32) > 0 /\ 
           rotate_right a b ==
           LI.rotate_right #t #LI.PUB a (cast b))
  
val rotate_left: #t:uinttype -> #t':inttype
  -> a:int_t t
  -> rotval t t'
  -> int_t t

val rotate_left_equiv_lemma: #t:uinttype -> #t':inttype
  -> a:int_t t -> b:rotval t t'
  -> Lemma (v (cast b <: u32) > 0 /\ 
           rotate_left a b ==
           LI.rotate_left #t #LI.PUB a (cast b))

let shift_right_i (#t:inttype) (#t':inttype) (s:shiftval t t') (u:int_t t) : int_t t = shift_right u s

let shift_left_i (#t:inttype) (#t':inttype) (s:shiftval t t') (u:int_t t{v u >= 0}) : int_t t = shift_left u s

let rotate_right_i (#t:uinttype) (#t':inttype) (s:rotval t t') (u:int_t t) : int_t t = rotate_right u s

let rotate_left_i (#t:uinttype) (#t':inttype) (s:rotval t t') (u:int_t t) : int_t t = rotate_left u s

let abs_int (#t:inttype) (a:int_t t{minint t < v a}) =
    mk_int #t (abs (v a))

val abs_int_equiv_lemma: #t:inttype{signed t /\ not (LI.S128? t)}
  -> a:int_t t{minint t < v a}
  -> Lemma (abs_int a == LI.ct_abs #t #LI.PUB a)


///
/// Operators available for all machine integers
///

// Strict: with precondition
unfold
let (+!) #t = add #t

// Wrapping: no precondition
unfold
let (+%) #t = add #t

unfold
let (+.) #t = add #t

unfold
let ( *! ) #t = mul #t

unfold
let ( *% ) #t = mul_mod #t

unfold
let ( *. ) #t = mul #t

unfold
let ( -! ) #t = sub #t

unfold
let ( -% ) #t = sub_mod #t

unfold
let ( -. ) #t = sub #t

unfold
let ( >>. ) #t #t' = shift_right #t #t'

unfold
let ( <<. ) #t #t' = shift_left #t #t'

unfold
let ( >>>. ) #t #t' = rotate_right #t #t'

unfold
let ( <<<. ) #t #t' = rotate_left #t #t'

unfold
let ( ^. ) #t = logxor #t

unfold
let ( |. ) #t = logor #t

unfold
let ( &. ) #t = logand #t

unfold
let ( ~. ) #t = lognot #t

unfold
let (/.) #t = div #t

unfold
let (%.) #t = mod #t

unfold
let (=.) #t = eq #t

unfold
let (<>.) #t = ne #t

unfold
let (<.) #t = lt #t

unfold
let (<=.) #t = lte #t

unfold
let (>.) #t = gt #t

unfold
let (>=.) #t = gte #t

