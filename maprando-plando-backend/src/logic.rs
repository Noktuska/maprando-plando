use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{bail, Result};
use maprando::{randomize::{DifficultyConfig, DoorState, FlagLocationState, ItemLocationState, Randomization, RandomizationState, Randomizer, SaveLocationState, StartLocationData, TraverserPair}, settings::{Objective, RandomizerSettings}, spoiler_log::{SpoilerLog, SpoilerRouteEntry, get_spoiler_route}, traverse::{LockedDoorData, Traverser, apply_requirement, get_bireachable_idxs, get_spoiler_trail_ids_by_idx, simple_cost_config}};
use maprando_game::{Capacity, GameData, HubLocation, Item, ItemLocationId, LinksDataGroup, Map, Requirement, StartLocation, VertexKey};
use maprando_logic::{GlobalState, LocalState};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use tokio::task::JoinHandle;

use crate::{Plando, SpoilerOverride};

pub struct HubLocationData {
    pub hub_location: HubLocation,
    pub hub_obtain_route: Vec<SpoilerRouteEntry>,
    pub hub_return_route: Vec<SpoilerRouteEntry>
}

struct LogicData {
    randomization: Randomization,
    spoiler_log: SpoilerLog,
    start_location_data: StartLocationData
}

pub struct Logic {

    game_data: Arc<GameData>,
    randomization: Arc<Mutex<Option<(Randomization, SpoilerLog)>>>,
    start_location: Arc<Mutex<HubLocationData>>

}

impl Logic {

    pub fn new(game_data: Arc<GameData>) -> Self {
        let ship_hub = Plando::get_ship_hub(&game_data);

        Self {
            game_data,
            randomization: Arc::new(Mutex::new(None)),
            start_location: Arc::new(Mutex::new(HubLocationData {
                hub_location: ship_hub,
                hub_obtain_route: Vec::new(),
                hub_return_route: Vec::new()
            }))
        }
    }

    pub fn get_randomization_arc(&self) -> Arc<Mutex<Option<(Randomization, SpoilerLog)>>> {
        self.randomization.clone()
    }

    pub fn get_randomization(&'_ self) -> MutexGuard<'_, Option<(Randomization, SpoilerLog)>> {
        self.randomization.lock().unwrap()
    }

    pub fn get_hub_data(&'_ self) -> MutexGuard<'_, HubLocationData> {
        self.start_location.lock().unwrap()
    }

    pub fn reset(&mut self) {
        let mut lock = self.randomization.lock().unwrap();
        *lock = None;
    }

    pub fn update_hub_and_randomization(
        &self,
        initial_global_state: GlobalState,
        initial_local_state: LocalState,
        start_location: StartLocation,
        locked_door_data: LockedDoorData,
        objectives: Vec<Objective>,
        difficulty: DifficultyConfig,
        item_locations: Vec<Item>,
        spoiler_overrides: Vec<SpoilerOverride>,
        randomizer_settings: RandomizerSettings,
        difficulty_tiers: Vec<DifficultyConfig>,
        map: Map,
        custom_escape_time: Option<usize>,
        rebuild_steps: bool
    ) -> JoinHandle<Result<()>> {
        let arc_hub = self.start_location.clone();
        let arc_r = self.randomization.clone();
        let game_data = self.game_data.clone();

        tokio::spawn(async move {
            let mut rng = StdRng::from_entropy();

            let game_data2 = game_data.clone();
            let game_data3 = game_data.clone();

            let filtered_base_links = maprando::randomize::filter_links(&game_data.links, &game_data, &difficulty);
            let base_links_data = LinksDataGroup::new(
                filtered_base_links,
                game_data.vertex_isv.keys.len(),
                0,
            );

            let randomizer = Randomizer::new(
                &map,
                &locked_door_data,
                objectives.clone(),
                &randomizer_settings,
                &difficulty_tiers,
                &game_data2,
                &base_links_data,
                &mut rng
            );

            let logic_data = update_randomization_impl(
                game_data3,
                randomizer,
                initial_global_state,
                initial_local_state,
                start_location,
                &item_locations,
                &spoiler_overrides,
                custom_escape_time,
                rebuild_steps
            ).await?;

            let mut lock = arc_hub.lock().unwrap();
            lock.hub_location = logic_data.start_location_data.hub_location;
            lock.hub_obtain_route = logic_data.start_location_data.hub_obtain_route;
            lock.hub_return_route = logic_data.start_location_data.hub_return_route;
            let mut lock = arc_r.lock().unwrap();
            *lock = Some((logic_data.randomization, logic_data.spoiler_log));

            Ok(())
        })
    }

}

async fn update_randomization_impl(
    game_data: Arc<GameData>,
    randomizer: Randomizer<'_>,
    initial_global_state: GlobalState,
    initial_local_state: LocalState,
    start_location: StartLocation,
    item_locations: &[Item],
    spoiler_overrides: &[SpoilerOverride],
    custom_escape_time: Option<usize>,
    rebuild_steps: bool
) -> Result<LogicData> {
    let initial_item_location_state = ItemLocationState {
        placed_item: None,
        placed_tier: None,
        collected: false,
        reachable_traversal: None,
        bireachable_traversal: None,
        bireachable_vertex_id: None,
        difficulty_tier: None,
    };
    let initial_flag_location_state = FlagLocationState {
        reachable_traversal: None,
        reachable_vertex_id: None,
        bireachable_traversal: None,
        bireachable_vertex_id: None,
    };
    let initial_save_location_state = SaveLocationState {
        bireachable_traversal: None,
    };
    let initial_door_state = DoorState {
        bireachable_traversal: None,
        bireachable_vertex_id: None,
    };

    let num_vertices = game_data.vertex_isv.keys.len();
    let mut traverser_pair = TraverserPair {
        forward: Traverser::new(
            num_vertices,
            false,
            initial_local_state,
            &initial_global_state
        ),
        reverse: Traverser::new(
            num_vertices,
            true,
            initial_local_state,
            &initial_global_state
        )
    };
    let (hub, hub_obtain_route, hub_return_route) = update_hub_location_impl(
        game_data.clone(),
        &randomizer,
        initial_global_state.clone(),
        start_location.clone(),
        &mut traverser_pair
    )?;

    let start_location_data = StartLocationData {
        start_location: start_location.clone(),
        hub_location: hub.clone(),
        hub_obtain_route,
        hub_return_route
    };

    let mut state = RandomizationState {
        step_num: 1,
        item_precedence: Vec::new(),
        start_location: start_location,
        hub_location: hub,
        save_location_state: vec![initial_save_location_state; game_data.save_locations.len()],
        item_location_state: vec![initial_item_location_state; game_data.item_locations.len()],
        flag_location_state: vec![initial_flag_location_state; game_data.flag_ids.len()],
        door_state: vec![initial_door_state; randomizer.locked_door_data.locked_doors.len()],
        items_remaining: randomizer.initial_items_remaining.clone(),
        starting_local_state: initial_local_state,
        global_state: initial_global_state,
        last_key_areas: Vec::new(),
    };

    let start_vertex_id = game_data.vertex_isv.index_by_key[&VertexKey {
        room_id: state.hub_location.room_id,
        node_id: state.hub_location.node_id,
        obstacle_mask: 0,
        actions: vec![]
    }];
    traverser_pair.forward.add_origin(
        initial_local_state,
        &state.global_state.inventory,
        start_vertex_id
    );
    traverser_pair.forward.finish_step(1);
    traverser_pair.reverse.add_origin(
        LocalState::full(true),
        &state.global_state.inventory,
        start_vertex_id
    );
    traverser_pair.reverse.finish_step(1);

    randomizer.update_reachability(&mut state, &mut traverser_pair);

    for i in 0..state.item_location_state.len() {
        if item_locations[i] != Item::Nothing {
            state.item_location_state[i].placed_item = Some(item_locations[i]);
        }
    }

    let max_override_step = spoiler_overrides.iter().map(|x| x.step).reduce(|acc, e| acc.max(e)).unwrap_or_default();

    loop {
        let last_cnt_bireachable = state
            .item_location_state
            .iter()
            .filter(|x| x.bireachable_traversal.is_some())
            .count();
        let last_cnt_flag_bireachable = state
            .flag_location_state
            .iter()
            .filter(|x| x.bireachable_traversal.is_some())
            .count();

        update_step(
            &mut state,
            &randomizer,
            &game_data,
            randomizer.settings,
            &randomizer.difficulty_tiers[0].tech,
            item_locations,
            spoiler_overrides,
            &mut traverser_pair
        );

        let cnt_bireachable = state
            .item_location_state
            .iter()
            .filter(|x| x.bireachable_traversal.is_some())
            .count();
        let cnt_flag_bireachable = state
            .flag_location_state
            .iter()
            .filter(|x| x.bireachable_traversal.is_some())
            .count();

        let any_progress = cnt_bireachable > last_cnt_bireachable || cnt_flag_bireachable > last_cnt_flag_bireachable;

        if !any_progress && state.step_num > max_override_step {
            break;
        }
    }

    for item_loc_state in &mut state.item_location_state {
        if item_loc_state.placed_item.is_none() {
            item_loc_state.placed_item = Some(Item::Nothing);
        }
    }

    let mut rng = StdRng::from_entropy();

    let seed_part = (rng.next_u32() % 0xFE) + 1; // Generate seed_part 1-255 so seed can't be 0
    let seed = seed_part | (seed_part << 8) | (seed_part << 16) | (seed_part << 24);

    let (mut r, mut s) = randomizer.get_randomization(
        &state,
        seed as usize,
        seed as usize,
        &mut rng,
        &mut traverser_pair,
        &start_location_data,
        true,
        rebuild_steps
    ).unwrap();

    // Apply custom escape time
    if let Some(custom_escape_time) = custom_escape_time {
        let base_igt_frames = custom_escape_time * 60;
        let base_igt_seconds = custom_escape_time as f32;
        let raw_time_seconds = base_igt_seconds; // Ignore multiplier for custom time
        let final_time_seconds = raw_time_seconds.min(5995.0);

        s.escape.base_igt_frames = base_igt_frames;
        s.escape.base_igt_seconds = base_igt_seconds;
        s.escape.raw_time_seconds = raw_time_seconds;
        s.escape.final_time_seconds = final_time_seconds;

        r.escape_time_seconds = final_time_seconds;
    }

    Ok(LogicData {
        randomization: r,
        spoiler_log: s,
        start_location_data
    })
}

fn update_step(
    state: &mut RandomizationState,
    randomizer: &Randomizer<'_>,
    game_data: &GameData,
    randomizer_settings: &RandomizerSettings,
    tech: &[bool],
    item_locations: &[Item],
    spoiler_overrides: &[SpoilerOverride],
    traverser_pair: &mut TraverserPair
) {
    loop {
        let mut any_update = false;
        for (i, &flag_id) in game_data.flag_ids.iter().enumerate() {
            if state.global_state.flags[flag_id] {
                continue;
            }
            if state.flag_location_state[i].reachable_traversal.is_some() && flag_id == game_data.mother_brain_defeated_flag_id {
                any_update = true;
                state.global_state.flags[flag_id] = true;
            } else if state.flag_location_state[i].bireachable_traversal.is_some() {
                any_update = true;
                state.global_state.flags[flag_id] = true;
            }
        }
        for i in 0..randomizer.locked_door_data.locked_doors.len() {
            if state.global_state.doors_unlocked[i] {
                continue;
            }
            if state.door_state[i].bireachable_traversal.is_some() {
                any_update = true;
                state.global_state.doors_unlocked[i] = true;
            }
        }
        if any_update {
            randomizer.update_reachability(state, traverser_pair);
        } else {
            break;
        }
    }

    let mut placed_uncollected_bireachable_loc: Vec<ItemLocationId> = Vec::new();
    let mut placed_uncollected_bireachable_items: Vec<Item> = Vec::new();
    for (i, item_location_state) in state.item_location_state.iter().enumerate() {
        if let Some(item) = item_location_state.placed_item {
            if !item_location_state.collected && item_location_state.bireachable_traversal.is_some() {
                placed_uncollected_bireachable_loc.push(i);
                placed_uncollected_bireachable_items.push(item);
            }
        }
    }

    let mut new_state = RandomizationState {
        step_num: state.step_num + 1,
        start_location: state.start_location.clone(),
        hub_location: state.hub_location.clone(),
        item_precedence: state.item_precedence.clone(),
        item_location_state: state.item_location_state.clone(),
        flag_location_state: state.flag_location_state.clone(),
        save_location_state: state.save_location_state.clone(),
        door_state: state.door_state.clone(),
        items_remaining: state.items_remaining.clone(),
        global_state: state.global_state.clone(),
        starting_local_state: get_initial_local_state(state, &traverser_pair, game_data),
        last_key_areas: Vec::new()
    };

    for &item in &placed_uncollected_bireachable_items {
        new_state.global_state.collect(item, &game_data, randomizer_settings.item_progression_settings.ammo_collect_fraction, tech);
    }
    // Add overrides to the current step
    let overrides: Vec<_> = spoiler_overrides.iter().filter(|x| x.step == state.step_num).collect();
    for item_override in &overrides {
        let item = item_locations[item_override.item_idx];
        new_state.global_state.collect(item, &game_data, randomizer_settings.item_progression_settings.ammo_collect_fraction, tech);
    }

    randomizer.update_reachability(&mut new_state, traverser_pair);

    for &loc in &placed_uncollected_bireachable_loc {
        new_state.item_location_state[loc].collected = true;
    }

    // Mark items as collected after getting spoiler data as they are not logically bireachable
    for item_override in overrides {
        let state = &mut new_state.item_location_state[item_override.item_idx];
        if state.collected {
            continue;
        }
        state.collected = true;
        let traverse_num = traverser_pair.forward.past_steps.len().saturating_sub(2);
        state.bireachable_traversal = Some(traverse_num);
        state.bireachable_vertex_id = game_data.item_vertex_ids[item_override.item_idx].first().copied();
        state.reachable_traversal = Some(traverse_num);
    }

    *state = new_state;
}

fn get_initial_local_state(state: &RandomizationState, traverser_pair: &TraverserPair, game_data: &GameData) -> LocalState {
    let start_vertex_id = game_data.vertex_isv.index_by_key[&VertexKey {
        room_id: state.hub_location.room_id,
        node_id: state.hub_location.node_id,
        obstacle_mask: 0,
        actions: vec![],
    }];
    let cost_metric_idx = 0; // use energy-sensitive cost metric
    let i = traverser_pair.forward.lsr[start_vertex_id].best_cost_idxs[cost_metric_idx];
    traverser_pair.forward.lsr[start_vertex_id].local[i as usize]
}

fn update_hub_location_impl(
    game_data: Arc<GameData>,
    randomizer: &Randomizer<'_>,
    global: GlobalState,
    start_location: StartLocation,
    traverser_pair: &mut TraverserPair
) -> Result<(HubLocation, Vec<SpoilerRouteEntry>, Vec<SpoilerRouteEntry>)> {
    if start_location.room_id == 8 && start_location.node_id == 5 && start_location.x == 72.0 && start_location.y == 69.5 {
        let ship_hub = Plando::get_ship_hub(&game_data);
        return Ok((ship_hub, Vec::new(), Vec::new()));
    }

    let start_vertex_id = game_data.vertex_isv.index_by_key[&VertexKey {
        room_id: start_location.room_id,
        node_id: start_location.node_id,
        obstacle_mask: 0,
        actions: vec![],
    }];

    let cost_config = simple_cost_config();
    let local = apply_requirement(
        &start_location.requires_parsed.as_ref().unwrap(),
        &global,
        LocalState::full(false),
        false,
        randomizer.settings,
        &randomizer.difficulty_tiers[0],
        &game_data,
        &randomizer.door_map,
        &randomizer.locked_door_data,
        &randomizer.objectives,
        &cost_config
    );
    let Some(local) = local else {
        bail!("Invalid start location")
    };

    traverser_pair.forward.add_origin(local, &global.inventory, start_vertex_id);
    traverser_pair.forward.traverse(
        randomizer.base_links_data,
        &randomizer.seed_links_data,
        &global,
        randomizer.settings,
        &randomizer.difficulty_tiers[0],
        &game_data,
        &randomizer.door_map,
        &randomizer.locked_door_data,
        &randomizer.objectives,
        0
    );
    let forward = &traverser_pair.forward;

    traverser_pair.reverse.add_origin(LocalState::full(true), &global.inventory, start_vertex_id);
    traverser_pair.reverse.traverse(
        randomizer.base_links_data,
        &randomizer.seed_links_data,
        &global,
        randomizer.settings,
        &randomizer.difficulty_tiers[0],
        &game_data,
        &randomizer.door_map,
        randomizer.locked_door_data,
        &randomizer.objectives,
        0
    );
    let reverse = &traverser_pair.reverse;

    let mut best_hub_vertex_id = start_vertex_id;
    let mut best_hub_cost = global.inventory.max_energy - 1 + global.inventory.max_reserves;
    for &(hub_vertex_id, ref hub_req) in [(start_vertex_id, Requirement::Free)].iter().chain(game_data.hub_farms.iter()) {
        if get_bireachable_idxs(&global, hub_vertex_id, &forward, &reverse).is_none() {
            continue;
        }

        let new_local = apply_requirement(
            hub_req,
            &global,
            LocalState::empty(),
            false,
            randomizer.settings,
            &randomizer.difficulty_tiers[0],
            &game_data,
            &randomizer.door_map,
            randomizer.locked_door_data,
            &randomizer.objectives,
            &cost_config
        );

        let hub_cost = match new_local {
            Some(loc) => loc.energy_missing(&global.inventory, true),
            None => Capacity::MAX
        };
        if hub_cost < best_hub_cost {
            best_hub_cost = hub_cost;
            best_hub_vertex_id = hub_vertex_id;
        }
    }

    let Some((forward_cost_idx, reverse_cost_idx)) = get_bireachable_idxs(&global, best_hub_vertex_id, &forward, &reverse)
    else {
        bail!("Inconsistent result from get_bireachable_idxs")
    };

    let vertex_key = game_data.vertex_isv.keys[best_hub_vertex_id].clone();
    let hub_location = HubLocation {
        room_id: vertex_key.room_id,
        node_id: vertex_key.node_id,
        vertex_id: best_hub_vertex_id
    };

    let hub_obtain_trail_ids = get_spoiler_trail_ids_by_idx(
        forward,
        best_hub_vertex_id,
        forward_cost_idx
    );
    let hub_return_trail_ids = get_spoiler_trail_ids_by_idx(
        reverse,
        best_hub_vertex_id,
        reverse_cost_idx
    );

    let hub_obtain_link_idxs = get_spoiler_route(&randomizer, &global, &hub_obtain_trail_ids, forward, false);
    let hub_return_link_idxs = get_spoiler_route(&randomizer, &global, &hub_return_trail_ids, reverse, true);

    traverser_pair.forward.pop_step();
    traverser_pair.reverse.pop_step();

    Ok((hub_location, hub_obtain_link_idxs, hub_return_link_idxs))
}