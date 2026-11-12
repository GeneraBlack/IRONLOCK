use super::inspect::is_lod1_name;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn distribute_vertex_budgets(
    counts: &[usize],
    total_budget: usize,
    min_floor: usize,
) -> Vec<usize> {
    let n = counts.len();
    if n == 0 {
        return Vec::new();
    }
    let total_orig: usize = counts.iter().sum();
    if total_orig <= total_budget {
        return counts.to_vec();
    }

    let mut budgets = vec![0usize; n];
    let mut remaining_budget = total_budget;
    let mut remaining_orig = total_orig;
    let mut locked = vec![false; n];

    // Lock tiny primitives first if affordable so small barrels/emissive caps stay crisp
    for _ in 0..2 {
        let mut changed = false;
        let unlocked_left = locked.iter().filter(|&&l| !l).count().max(1);
        for i in 0..n {
            if locked[i] {
                continue;
            }
            let min_keep = counts[i]
                .min(min_floor)
                .min(remaining_budget / unlocked_left);
            let prop = if remaining_orig > 0 {
                ((counts[i] as u128 * remaining_budget as u128) / remaining_orig as u128) as usize
            } else {
                min_keep
            };
            if prop <= min_keep {
                let grant = min_keep.max(3.min(counts[i]));
                budgets[i] = grant;
                locked[i] = true;
                remaining_budget = remaining_budget.saturating_sub(grant);
                remaining_orig = remaining_orig.saturating_sub(counts[i]);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let unlocked_indices: Vec<usize> = (0..n).filter(|&i| !locked[i]).collect();
    if !unlocked_indices.is_empty() {
        let mut assigned = 0usize;
        for (k, &i) in unlocked_indices.iter().enumerate() {
            let grant = if k + 1 == unlocked_indices.len() {
                remaining_budget.saturating_sub(assigned)
            } else if remaining_orig > 0 {
                ((counts[i] as u128 * remaining_budget as u128) / remaining_orig as u128) as usize
            } else {
                remaining_budget / unlocked_indices.len()
            };
            let clamped = grant.max(3.min(counts[i])).min(counts[i]);
            budgets[i] = clamped;
            assigned += clamped;
        }
    }

    // Final strict ceiling guard so sum(budgets) <= total_budget
    let mut sum_b: usize = budgets.iter().sum();
    while sum_b > total_budget {
        if let Some((max_i, _)) = budgets
            .iter()
            .enumerate()
            .filter(|&(_, &b)| b > 6)
            .max_by_key(|&(_, &b)| b)
        {
            let excess = (sum_b - total_budget).min(budgets[max_i] - 6);
            budgets[max_i] -= excess.max(1);
            sum_b = budgets.iter().sum();
        } else {
            break;
        }
    }

    budgets
}

pub(super) fn strip_existing_lod1_hierarchy(json: &mut Value) {
    let Some(old_nodes) = json.get("nodes").and_then(Value::as_array).cloned() else {
        return;
    };
    let mut keep_node = vec![true; old_nodes.len()];
    for (ni, node) in old_nodes.iter().enumerate() {
        let name = node.get("name").and_then(Value::as_str).unwrap_or("");
        if is_lod1_name(name) {
            keep_node[ni] = false;
        }
    }
    if keep_node.iter().all(|&k| k) {
        return;
    }

    let mut node_remap = vec![usize::MAX; old_nodes.len()];
    let mut new_nodes = Vec::new();
    for (old_i, node) in old_nodes.iter().enumerate() {
        if keep_node[old_i] {
            node_remap[old_i] = new_nodes.len();
            new_nodes.push(node.clone());
        }
    }

    for node in &mut new_nodes {
        if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
            let filtered: Vec<Value> = children
                .iter()
                .filter_map(|c| {
                    c.as_u64()
                        .and_then(|ci| node_remap.get(ci as usize).copied())
                        .filter(|&ni| ni != usize::MAX)
                        .map(|ni| json!(ni))
                })
                .collect();
            if filtered.is_empty() {
                if let Some(obj) = node.as_object_mut() {
                    obj.remove("children");
                }
            } else {
                *children = filtered;
            }
        }
    }

    if let Some(scenes) = json.get_mut("scenes").and_then(Value::as_array_mut) {
        for sc in scenes {
            if let Some(snodes) = sc.get_mut("nodes").and_then(Value::as_array_mut) {
                *snodes = snodes
                    .iter()
                    .filter_map(|c| {
                        c.as_u64()
                            .and_then(|ci| node_remap.get(ci as usize).copied())
                            .filter(|&ni| ni != usize::MAX)
                            .map(|ni| json!(ni))
                    })
                    .collect();
            }
        }
    }

    if let Some(anims) = json.get_mut("animations").and_then(Value::as_array_mut) {
        for anim in anims {
            if let Some(channels) = anim.get_mut("channels").and_then(Value::as_array_mut) {
                channels.retain_mut(|ch| {
                    if let Some(target) = ch.get_mut("target") {
                        if let Some(old_n) = target.get("node").and_then(Value::as_u64) {
                            let mapped = node_remap
                                .get(old_n as usize)
                                .copied()
                                .unwrap_or(usize::MAX);
                            if mapped == usize::MAX {
                                return false;
                            }
                            target["node"] = json!(mapped);
                        }
                    }
                    true
                });
            }
        }
    }

    if let Some(skins) = json.get_mut("skins").and_then(Value::as_array_mut) {
        for skin in skins {
            if let Some(joints) = skin.get_mut("joints").and_then(Value::as_array_mut) {
                *joints = joints
                    .iter()
                    .filter_map(|j| {
                        j.as_u64()
                            .and_then(|ji| node_remap.get(ji as usize).copied())
                            .filter(|&ni| ni != usize::MAX)
                            .map(|ni| json!(ni))
                    })
                    .collect();
            }
            if let Some(skel) = skin.get("skeleton").and_then(Value::as_u64) {
                let mapped = node_remap.get(skel as usize).copied().unwrap_or(usize::MAX);
                if mapped != usize::MAX {
                    skin["skeleton"] = json!(mapped);
                }
            }
        }
    }

    // Strip unreferenced _LOD1 meshes and remap node.mesh indices
    if let Some(old_meshes) = json.get("meshes").and_then(Value::as_array).cloned() {
        let mut referenced = vec![false; old_meshes.len()];
        for node in &new_nodes {
            if let Some(mi) = node.get("mesh").and_then(Value::as_u64) {
                if (mi as usize) < referenced.len() {
                    referenced[mi as usize] = true;
                }
            }
        }
        let mut mesh_remap = vec![usize::MAX; old_meshes.len()];
        let mut kept_meshes = Vec::new();
        for (mi, m) in old_meshes.into_iter().enumerate() {
            let mname = m.get("name").and_then(Value::as_str).unwrap_or("");
            if referenced[mi] || !is_lod1_name(mname) {
                mesh_remap[mi] = kept_meshes.len();
                kept_meshes.push(m);
            }
        }
        for node in &mut new_nodes {
            if let Some(mi) = node.get("mesh").and_then(Value::as_u64) {
                let nm = mesh_remap.get(mi as usize).copied().unwrap_or(usize::MAX);
                if nm != usize::MAX {
                    node["mesh"] = json!(nm);
                }
            }
        }
        json["meshes"] = Value::Array(kept_meshes);
    }

    json["nodes"] = Value::Array(new_nodes);
}

pub(super) fn preserve_non_mesh_buffers(
    json: &mut Value,
    old_bin: &[u8],
    new_bin: &mut Vec<u8>,
    new_buffer_views: &mut Vec<Value>,
    new_accessors: &mut Vec<Value>,
) {
    let mut needed_accessors = BTreeSet::new();
    let mut needed_bvs = BTreeSet::new();

    if let Some(anims) = json.get("animations").and_then(Value::as_array) {
        for anim in anims {
            if let Some(samplers) = anim.get("samplers").and_then(Value::as_array) {
                for s in samplers {
                    if let Some(inp) = s.get("input").and_then(Value::as_u64) {
                        needed_accessors.insert(inp as usize);
                    }
                    if let Some(out) = s.get("output").and_then(Value::as_u64) {
                        needed_accessors.insert(out as usize);
                    }
                }
            }
        }
    }

    if let Some(skins) = json.get("skins").and_then(Value::as_array) {
        for skin in skins {
            if let Some(ibm) = skin.get("inverseBindMatrices").and_then(Value::as_u64) {
                needed_accessors.insert(ibm as usize);
            }
        }
    }

    if let Some(images) = json.get("images").and_then(Value::as_array) {
        for img in images {
            if let Some(bv) = img.get("bufferView").and_then(Value::as_u64) {
                needed_bvs.insert(bv as usize);
            }
        }
    }

    if needed_accessors.is_empty() && needed_bvs.is_empty() {
        return;
    }

    let old_accessors = json
        .get("accessors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let old_bvs = json
        .get("bufferViews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for &acc_i in &needed_accessors {
        if let Some(acc) = old_accessors.get(acc_i) {
            if let Some(bv_i) = acc.get("bufferView").and_then(Value::as_u64) {
                needed_bvs.insert(bv_i as usize);
            }
        }
    }

    let mut bv_map = BTreeMap::new();
    for bv_i in needed_bvs {
        let Some(mut bv) = old_bvs.get(bv_i).cloned() else {
            continue;
        };
        let off = bv.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let len = bv.get("byteLength").and_then(Value::as_u64).unwrap_or(0) as usize;
        if off + len > old_bin.len() {
            continue;
        }
        while !new_bin.len().is_multiple_of(4) {
            new_bin.push(0);
        }
        let new_off = new_bin.len();
        new_bin.extend_from_slice(&old_bin[off..off + len]);
        bv["buffer"] = json!(0);
        bv["byteOffset"] = json!(new_off);
        bv["byteLength"] = json!(len);
        let mapped_bv = new_buffer_views.len();
        new_buffer_views.push(bv);
        bv_map.insert(bv_i, mapped_bv);
    }

    let mut acc_map = BTreeMap::new();
    for acc_i in needed_accessors {
        let Some(mut acc) = old_accessors.get(acc_i).cloned() else {
            continue;
        };
        if let Some(old_bv) = acc.get("bufferView").and_then(Value::as_u64) {
            if let Some(&mapped_bv) = bv_map.get(&(old_bv as usize)) {
                acc["bufferView"] = json!(mapped_bv);
            }
        }
        let mapped_acc = new_accessors.len();
        new_accessors.push(acc);
        acc_map.insert(acc_i, mapped_acc);
    }

    if let Some(anims) = json.get_mut("animations").and_then(Value::as_array_mut) {
        for anim in anims {
            if let Some(samplers) = anim.get_mut("samplers").and_then(Value::as_array_mut) {
                for s in samplers {
                    if let Some(inp) = s.get("input").and_then(Value::as_u64) {
                        if let Some(&m) = acc_map.get(&(inp as usize)) {
                            s["input"] = json!(m);
                        }
                    }
                    if let Some(out) = s.get("output").and_then(Value::as_u64) {
                        if let Some(&m) = acc_map.get(&(out as usize)) {
                            s["output"] = json!(m);
                        }
                    }
                }
            }
        }
    }

    if let Some(skins) = json.get_mut("skins").and_then(Value::as_array_mut) {
        for skin in skins {
            if let Some(ibm) = skin.get("inverseBindMatrices").and_then(Value::as_u64) {
                if let Some(&m) = acc_map.get(&(ibm as usize)) {
                    skin["inverseBindMatrices"] = json!(m);
                }
            }
        }
    }

    if let Some(images) = json.get_mut("images").and_then(Value::as_array_mut) {
        for img in images {
            if let Some(bv) = img.get("bufferView").and_then(Value::as_u64) {
                if let Some(&m) = bv_map.get(&(bv as usize)) {
                    img["bufferView"] = json!(m);
                }
            }
        }
    }
}
