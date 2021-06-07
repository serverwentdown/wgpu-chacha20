[[block]]
struct ChaChaRoutine {
    state: array<u32, 16>;
};

[[group(0), binding(0)]]
var<storage> routine: [[access(read_write)]] ChaChaRoutine;

fn qround_axr(x: u32, y: u32, z: u32, n: u32) {
    routine.state[x] = routine.state[x] + routine.state[y];
    routine.state[z] = routine.state[z] ^ routine.state[x];
    routine.state[z] = (routine.state[z] << n) | (routine.state[z] >> (32u - n));
}

fn qround(a: u32, b: u32, c: u32, d: u32) {
    qround_axr(a, b, d, 16u);
    qround_axr(c, d, b, 12u);
    qround_axr(a, b, d, 8u);
    qround_axr(c, d, b, 7u);
}

[[stage(compute), workgroup_size(1)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    qround(2u, 7u, 8u, 13u);
}