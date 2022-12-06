# Read address from json file
import toml

config = toml.load('benchmark_config.toml')
addr = config["address"]
params = config["parameters"]
client = config["client"]
server = config["server"]

client_addr = addr['client']
alice_addr = addr['alice']
bob_addr = addr['bob']

client_name = client["bin"]
server_name = server["bin"]

gsize = params["gsize"]
num_clients = params["num_clients"]
num_mpc_sockets = server["num_mpc_sockets"]
input_size = params["input_size"]

client_flags = client["run_flags"]
server_flags = server["run_flags"]
client_compile_flags = client["build_flags"]
server_compile_flags = server["build_flags"]

flamegraph_alice = server["flamegraph_alice"]
flamegraph_bob = server["flamegraph_bob"]

run_cmd_alice = "cargo flamegraph" if flamegraph_alice else "cargo run --release"
run_cmd_bob = "cargo flamegraph" if flamegraph_bob else "cargo run --release"

meta_client_command = "RUSTFLAGS='-C target-cpu=native' cargo run --release --package {} {}  -- " \
                      "{} -g {} -n {} -a {}:6666 -b {}:6667 -i {}".format(
                          client_name, client_compile_flags, client_flags, gsize, num_clients, alice_addr, bob_addr, input_size)
alice_command = "RUSTFLAGS='-C target-cpu=native' {} --package {} {}  -- -g {} -n {} -m 7777 -p 6666 -s {} -i {} {}".format(
    run_cmd_alice, server_name, server_compile_flags, gsize, num_clients, num_mpc_sockets, input_size,
    server_flags)
bob_command = "RUSTFLAGS='-C target-cpu=native' {} --package {} {}  -- -g {} -n {} -m {}:7777 -b -p 6667 -s {} -i {} {}".format(
    run_cmd_bob, server_name, server_compile_flags, gsize, num_clients, alice_addr, num_mpc_sockets, input_size,
    server_flags)

print("Meta Client: {}\n".format(meta_client_command))
print("Alice: {}\n".format(alice_command))
print("Bob: {}\n".format(bob_command))

if flamegraph_bob:
    flamegraph_command = "scp -i ~/SecureFL.pem ubuntu@{}:~/eiffel-rs/flamegraph.svg .".format(
        bob_addr)
    print("Flamegraph Bob: {}\n".format(flamegraph_command))
