use super::*;

#[test]
fn astar_finds_path_around_blocked_obstacle() {
    let mut grid = CollisionGrid::new(16, 16, 2.0);
    grid.set_footprint_blocked([4, 2], [1, 6], true);

    let path = grid.find_path([2, 4], [6, 4]);
    assert!(!path.is_empty());
    for wp in &path {
        let tile = grid.world_to_tile(*wp);
        assert!(
            !grid.blocked_tiles.contains(&tile),
            "Path should not cross blocked tile {tile:?}"
        );
    }
    let last_tile = grid.world_to_tile(*path.last().unwrap());
    assert_eq!(last_tile, [6, 4]);
}

#[test]
fn cliff_elevation_requires_ramp_tile_and_occludes_low_ground_los() {
    let mut grid = CollisionGrid::new(16, 16, 2.0);
    for tx in 6..=10 {
        for tz in 4..=8 {
            grid.elevation_tiles.insert([tx, tz]);
        }
    }
    grid.ramp_tiles.insert([8, 9]);
    grid.ramp_tiles.insert([8, 8]);

    assert!(!grid.has_line_of_sight([4, 6], 0.0, [7, 6]));
    assert!(grid.has_line_of_sight([8, 9], 0.675, [7, 6]));
    assert!(grid.has_line_of_sight([6, 6], 1.35, [7, 6]));

    let path = grid.find_path([4, 6], [7, 6]);
    assert!(!path.is_empty(), "Should find path via ramp");
    let visited_tiles: Vec<[i32; 2]> = path.iter().map(|wp| grid.world_to_tile(*wp)).collect();
    assert!(
        visited_tiles.iter().any(|t| *t == [8, 9] || *t == [8, 8]),
        "Path onto plateau must traverse ramp tiles, got {visited_tiles:?}"
    );
}

#[test]
fn brushfire_clearance_routes_heavy_2x2_around_narrow_alley() {
    let mut grid = CollisionGrid::new(20, 20, 2.0);
    grid.set_footprint_blocked([5, 4], [3, 9], true);
    grid.set_footprint_blocked([9, 4], [3, 9], true);

    assert_eq!(grid.clearance_at([8, 8]), 1);
    assert!(grid.clearance_at([13, 8]) >= 2);

    let inf_path =
        grid.find_path_with_clearance([8, 2], [8, 15], MovementDomain::Ground, 0.0, &[], 1);
    assert!(
        inf_path
            .iter()
            .any(|wp| grid.world_to_tile(*wp)[0] == 8
                && (4..=12).contains(&grid.world_to_tile(*wp)[1])),
        "1x1 unit should traverse narrow x=8 alley"
    );

    let tank_path =
        grid.find_path_with_clearance([8, 1], [8, 15], MovementDomain::Ground, 0.0, &[], 2);
    assert!(!tank_path.is_empty());
    for wp in &tank_path {
        let t = grid.world_to_tile(*wp);
        assert!(
            !(t[0] == 8 && (4..=12).contains(&t[1])),
            "2x2 heavy unit entered narrow 1x1 alley at {t:?}"
        );
    }
}

#[test]
fn flat_tile_set_and_map_preserve_sorted_iteration_and_o1_queries() {
    let mut set = FlatTileSet::with_dimensions(32, 32);
    assert!(set.insert([10, 5]));
    assert!(set.insert([2, 9]));
    assert!(set.insert([10, 1]));
    assert!(!set.insert([10, 5]));
    assert!(set.contains(&[10, 5]));
    assert!(!set.contains(&[11, 5]));
    let ordered: Vec<[i32; 2]> = set.iter().copied().collect();
    assert_eq!(ordered, vec![[2, 9], [10, 1], [10, 5]]);

    let mut map = FlatTileMap::with_dimensions(32, 32);
    assert_eq!(map.insert([8, 4], 500_u32), None);
    assert_eq!(map.insert([3, 7], 250_u32), None);
    assert_eq!(map.insert([8, 4], 900_u32), Some(500_u32));
    assert_eq!(map.get(&[8, 4]), Some(&900));
    assert_eq!(map.remove(&[3, 7]), Some(250));
    assert_eq!(map.get(&[3, 7]), None);
    assert_eq!(map.get(&[8, 4]), Some(&900));
}
