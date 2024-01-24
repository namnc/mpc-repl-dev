# mpc-repl-dev

UI: https://zkrepl.dev
with additional box to specify party inputs
peer discovery
wasm for GC2PC
https://github.com/0xPARC/zkrepl

interpreter: https://github.com/namnc/circom-2-arithc
2PC: https://github.com/tkmct/mpz/tree/bmr16

circom editor
- write circom program: like this https://github.com/namnc/circom-2-arithc/blob/main/src/assets/circuit.circom
- write configuration: like in zkrepl, but specify which are inputs, outputs and which input are from whom (similar to test in zkrepl)

server will run circom-2-arithc on circom program to generate arithemtic circuit in json format: like this https://github.com/tkmct/mpz/blob/bmr16/garble/mpz-garble/examples/circ.json
- binary execution: circom-2-arithc circ.circom [libs], can edit here: https://github.com/namnc/circom-2-arithc/blob/main/src/compiler.rs

mpz-bmr16 will be compiled into wasm (1 time only): like this https://github.com/tkmct/mpz/blob/bmr16/garble/mpz-garble/examples/bmr16_demo.rs
- load circuit json
- load configuration (which input is from G and which is from E)
- allow inputing argument: like this
-- mpz-bmr16 circ.json circ.cfg -G (-E) [inputs]
- run in browser using server as proxy

