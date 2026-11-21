// All arithmetic is exact bounded integer arithmetic. No shader float feature is used.
@group(0) @binding(0) var<storage, read> data: array<u32>;
@group(0) @binding(1) var<storage, read_write> owners: array<u32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let query = id.x;
    if query >= data[0] { return; }
    let qx = data[4] + (query % data[9]) * 2u;
    let qy = data[4] + data[9]*2u + (query/data[9])*2u;
    let x = bitcast<i32>(data[qx]);
    let y = bitcast<i32>(data[qy]);
    let cx = bitcast<i32>(data[qx+1u]) - bitcast<i32>(data[7]);
    let cy = bitcast<i32>(data[qy+1u]) - bitcast<i32>(data[8]);
    var first_lower = 0xffffffffu;
    var second_lower = 0xffffffffu;
    var lower_owner = 0xffffffffu;
    var best_upper = 0xffffffffu;
    var best_owner = 0xffffffffu;
    var valid = true;
    for (var dy = -2; dy <= 2; dy++) {
        for (var dx = -2; dx <= 2; dx++) {
            let bx = cx + dx;
            let by = cy + dy;
            if bx < 0 || by < 0 || bx >= i32(data[5]) || by >= i32(data[6]) { continue; }
            let bucket = u32(by) * data[5] + u32(bx);
            let begin = data[data[2] + bucket];
            let end = data[data[2] + bucket + 1u];
            for (var item = begin; item < end; item++) {
                let ordinal = data[data[3] + item];
                let px = bitcast<i32>(data[data[1] + ordinal*2u]);
                let py = bitcast<i32>(data[data[1] + ordinal*2u+1u]);
                let ax = u32(abs(x - px));
                let ay = u32(abs(y - py));
                if ax > 16384u || ay > 16384u { valid = false; continue; }
                let squared = ax*ax + ay*ay;
                let error = 2u*(ax+ay) + 8u;
                let lower = squared - min(squared, error);
                let upper = squared + error;
                if lower < first_lower {
                    second_lower = first_lower;
                    first_lower = lower;
                    lower_owner = ordinal;
                } else { second_lower = min(second_lower, lower); }
                if upper < best_upper { best_upper = upper; best_owner = ordinal; }
            }
        }
    }
    let competing_lower = select(first_lower, second_lower, best_owner == lower_owner);
    owners[query] = select(0xffffffffu, best_owner, valid && best_upper < competing_lower);
}
