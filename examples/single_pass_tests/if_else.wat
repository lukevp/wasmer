(module
    (func $main (export "main")
        (local $a i32)
        (set_local $a (i32.const 33))

        (block
            (call $foo (if (result i32) (i32.eq (get_local $a) (i32.const 33))
                (then (i32.const 1))
                (else (i32.const 2))
            ))
            (i32.eq (i32.const 43))
            (br_if 0)
            (unreachable)
        )
        (block
            (call $foo (if (result i32) (i32.eq (get_local $a) (i32.const 30))
                (then (i32.const 1))
                (else (i32.const 2))
            ))
            (i32.eq (i32.const 44))
            (br_if 0)
            (unreachable)
        )
    )

    (func $foo (param $input i32) (result i32)
        (local $a i32)
        (set_local $a (i32.const 42))
        (get_local $a)
        (get_local $input)
        (i32.add)
    )
)
