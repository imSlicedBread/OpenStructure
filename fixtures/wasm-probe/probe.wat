;; Independently assembled ABI fixture, not a parametric Wall plugin.
;; Returns a fixed unit prism. Input buffer and output data do not overlap.
(module
  (memory (export "memory") 32 32)
  (data (i32.const 0) "{\22api_version\22:9,\22response\22:{\22result\22:\22Solid\22,\22data\22:{\22profile\22:{\22vertices\22:[{\22x\22:0,\22y\22:0},{\22x\22:1,\22y\22:0},{\22x\22:1,\22y\22:1},{\22x\22:0,\22y\22:1}]},\22height\22:1,\22transform\22:{\22translation\22:{\22x\22:0,\22y\22:0,\22z\22:0},\22rotation_z\22:0}}}}")
  (func (export "os_abi_version") (result i32) i32.const 1)
  (func (export "os_alloc") (param i32) (result i32) i32.const 65536)
  (func (export "os_invoke") (param i32 i32) (result i64)
    ;; Find the zero terminator in the initially zero-filled linear memory.
    (local $length i32)
    (block $done (loop $scan
      (br_if $done (i32.eqz (i32.load8_u (local.get $length))))
      (local.set $length (i32.add (local.get $length) (i32.const 1)))
      (br $scan)))
    (i64.shl (i64.extend_i32_u (local.get $length)) (i64.const 32)))
)
