# ELSA

## Installing Dependencies

### Install EMP Tools (optional: if running baseline Prio+)
```sh
wget https://raw.githubusercontent.com/emp-toolkit/emp-readme/master/scripts/install.py
python3 install.py --install --tool --ot
```
### Install Clang
```sh
sudo apt-get update
sudo apt-get install clang
```

### Install Rust
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Unit Testing
This repository includes unit tests for all of our building blocks which can be run using:
```sh
cargo test
```

Rust Version used during testing: 1.65.0

## End-to-end Testing 
To run end-to-end tests with our malicious-private backend with one-shot clients, use the following commands (parameter size `gsize = 1000`, `10` clients, `32` bit input values):

- Meta Client (start last): 
```sh
RUSTFLAGS='-C target-cpu=native' cargo run --release --package client-mp   --  -g 1000 -n 10 -a localhost:6666 -b localhost:6667 -i 32
```
- Alice (start first):
```sh
RUSTFLAGS='-C target-cpu=native' cargo run --release --package server-mp   -- -g 1000 -n 10 -m 7777 -p 6666 -s 16 -i 32
```
- Bob (start second):
```sh
RUSTFLAGS='-C target-cpu=native' cargo run --release --package server-mp   -- -g 1000 -n 10 -m localhost:7777 -b -p 6667 -s 16 -i 32
```

To change parameters, alter `benchmark_config.toml` and run `python benchmark_commands.py` to get the commands. 

To run other backends (e.g., only L<sub>$\infty$</sub>, a.k.a. po2, with malicious privacy), alter the `bin` field for both client and server (to `client-mp-po2` and `server-mp-po2`) in `benchmark_config.toml` and run `python benchmark_commands.py` to get the commands.

## Comments
A minor comment regarding the current state of this code is that it doesn't implement the $\ell_2$ enforcement phase and we haven't yet refactored our code to defer the opening of the results of all intermediate checks (OT and square correlation verification result) to after the transcript digest matching has occured. We now elabore on both in more detail:

- $\ell_2$ enforcement phase: In this phase, the shares of $\ell_2$ value of each client are fed through an adder to compute the sign bit. The cost of this phase doesn't directly grow with `gsize` (our largest parameter), and therefore, both communication and runtime are completely overshadowed by other phases. This phase just executes a *single* adder per client which is a practically negligible cost given other much heavier phases. For completeness, this phase can be implemented by plugging in any 2PC library like EMPToolkit, ABY, CryptFlow2, etc.
- Deferred opening of intermediate checks: Opening of the results of intermediate checks currently happens along with the rest of the steps of the corresponding check and is not deferred to the later point after the observed transcript digests have been cross-checked against the ones submitted by the clients. To peform deferred checks (as mentioned in our paper) rather than interspersed ones, some code refactoring is pending. Note that the refactoring doesn't affect the performance in any way, and the current code does implement our ideas for malicious privacy by transcript emulation and cross-checking.

Neither of these affect the evaluation results in any noticeable way, and are quite benign in their impact on the overall system.
