[[block]]
struct ChaChaSetup {
    key: array<u32, 8>;
    start_counter: u32;
    nonce: array<u32, 3>;
};

[[group(0), binding(0)]]
var<storage> routine: [[access(read_write)]] ChaChaSetup;

// Routine state

[[block]]
struct ChaChaState {
    data: array<u32, 16>;
};

[[group(0), binding(1)]]
var<storage> state: [[access(read_write)]] ChaChaState;

// QUARTERROUND routine

fn qround_axr(x: u32, y: u32, z: u32, n: u32) {
    state.data[x] = state.data[x] + state.data[y];
    state.data[z] = state.data[z] ^ state.data[x];
    state.data[z] = (state.data[z] << n) | (state.data[z] >> (32u - n));
}

fn qround(a: u32, b: u32, c: u32, d: u32) {
    qround_axr(a, b, d, 16u);
    qround_axr(c, d, b, 12u);
    qround_axr(a, b, d, 8u);
    qround_axr(c, d, b, 7u);
}

// INNER_BLOCK routine

fn block_inner() {
    qround(0u, 4u, 8u, 12u);
    qround(1u, 5u, 9u, 13u);
    qround(2u, 6u, 10u, 14u);
    qround(3u, 7u, 11u, 15u);
    qround(0u, 5u, 10u, 15u);
    qround(1u, 6u, 11u, 12u);
    qround(2u, 7u, 8u, 13u);
    qround(3u, 4u, 9u, 14u);
}

// CHACHA20_BLOCK routine

fn block_setup() {
    state.data[0] = 1634760805u;
    state.data[1] = 857760878u;
    state.data[2] = 2036477234u;
    state.data[3] = 1797285235u;
    state.data[4] = routine.key[0];
    state.data[5] = routine.key[1];
    state.data[6] = routine.key[2];
    state.data[7] = routine.key[3];
    state.data[8] = routine.key[4];
    state.data[9] = routine.key[5];
    state.data[10] = routine.key[6];
    state.data[11] = routine.key[7];
    state.data[12] = routine.start_counter;
    state.data[13] = routine.nonce[0];
    state.data[14] = routine.nonce[1];
    state.data[15] = routine.nonce[2];
}

fn block() {
    block_setup();

    var i: u32 = 0u;
    loop { // for i in 0..10
        if (i >= 10u) { break; }
        block_inner();
        i = i + 1u;
    }
    
    // TODO: state + original_state
}

[[stage(compute), workgroup_size(1)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    block();
}