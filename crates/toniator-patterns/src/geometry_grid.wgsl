// Coordinates are CPU-authoritative binary64 bit patterns; shaders only pair and classify them.
@group(0) @binding(0) var<storage, read> data: array<u32>;
@group(0) @binding(1) var<storage, read_write> result: array<u32>;

fn less(raw_a: vec2<u32>, raw_b: vec2<u32>) -> bool {
    let a = select(raw_a,vec2(0u,0u),raw_a.x == 0u && (raw_a.y & 0x7fffffffu) == 0u);
    let b = select(raw_b,vec2(0u,0u),raw_b.x == 0u && (raw_b.y & 0x7fffffffu) == 0u);
    // IEEE total-order keys, with both zeros normalized by the CPU producer.
    let sa = select(0x80000000u, 0xffffffffu, (a.y & 0x80000000u) != 0u);
    let sb = select(0x80000000u, 0xffffffffu, (b.y & 0x80000000u) != 0u);
    let ah = a.y ^ sa;
    let bh = b.y ^ sb;
    let al = a.x ^ select(0u, 0xffffffffu, (a.y & 0x80000000u) != 0u);
    let bl = b.x ^ select(0u, 0xffffffffu, (b.y & 0x80000000u) != 0u);
    return ah < bh || (ah == bh && al < bl);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= data[0] { return; }
    let xi = i / data[2];
    let yi = i % data[2];
    let x = vec2(data[12u + xi*2u], data[13u + xi*2u]);
    let yo = 12u + data[1]*2u + yi*2u;
    let y = vec2(data[yo], data[yo+1u]);
    let zero = vec2(0u,0u);
    let inside = !less(x,zero) && !less(vec2(data[4],data[5]),x) && !less(y,zero) && !less(vec2(data[6],data[7]),y);
    let o = i*5u;
    result[o] = x.x;
    result[o+1u] = x.y;
    result[o+2u] = y.x;
    result[o+3u] = y.y;
    result[o+4u] = select(1u,2u,inside);
}
