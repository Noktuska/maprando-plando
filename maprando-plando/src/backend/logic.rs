use std::{ops::DerefMut, sync::{Arc, Mutex, MutexGuard}};

use anyhow::{bail, Result};
use hashbrown::HashSet;
use maprando::{randomize::{DebugData, DifficultyConfig, DoorState, FlagLocationState, ItemLocationState, Randomization, RandomizationState, Randomizer, SaveLocationState, SpoilerDetails, SpoilerDoorDetails, SpoilerDoorSummary, SpoilerFlagDetails, SpoilerFlagSummary, SpoilerItemDetails, SpoilerItemSummary, SpoilerLocation, SpoilerLog, SpoilerRouteEntry, SpoilerSummary, StartLocationData}, settings::{Objective, RandomizerSettings}, traverse::{apply_requirement, get_bireachable_idxs, get_spoiler_route, traverse, LockedDoorData}};
use maprando_game::{Capacity, GameData, HubLocation, Item, ItemLocationId, LinksDataGroup, Map, Requirement, StartLocation, VertexKey};
use maprando_logic::{GlobalState, LocalState};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use strum::VariantNames;
use tokio::task::JoinHandle;

use crate::backend::{plando::{Plando, SpoilerOverride}, randomize::get_vertex_info_by_id};

pub struct HubLocationData {
    pub hub_location: HubLocation,
    pub hub_obtain_route: Vec<SpoilerRouteEntry>,
    pub hub_return_route: Vec<SpoilerRouteEntry>
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

    pub fn _update_randomization(
        &self,
        initial_global_state: GlobalState,
        initial_local_state: LocalState,
        start_location_data: StartLocationData,
        locked_door_data: LockedDoorData,
        objectives: Vec<Objective>,
        base_links_data: LinksDataGroup,
        item_locations: Vec<Item>,
        spoiler_overrides: Vec<SpoilerOverride>,
        randomizer_settings: RandomizerSettings,
        difficulty_tiers: Vec<DifficultyConfig>,
        map: Map,
        custom_escape_time: Option<usize>
    ) -> JoinHandle<()> {
        let arc = self.randomization.clone();
        let game_data = self.game_data.clone();

        tokio::spawn(async move {
            let mut rng = StdRng::from_entropy();

            let game_data2 = game_data.clone();
            let randomizer = Randomizer::new(
                &map,
                &locked_door_data,
                objectives,
                &randomizer_settings,
                &difficulty_tiers,
                &game_data2,
                &base_links_data,
                &mut rng
            );

            let (r, s) = update_randomization_impl(
                game_data,
                randomizer,
                initial_global_state,
                initial_local_state,
                start_location_data,
                &locked_door_data,
                &item_locations,
                &spoiler_overrides,
                &randomizer_settings,
                &difficulty_tiers,
                &map,
                custom_escape_time
            ).await;

            let mut lock = arc.lock().unwrap();
            *lock = Some((r, s));
        })
    }

    pub fn _update_hub_location(
        &self,
        start_location: StartLocation,
        initial_global_state: GlobalState,
        locked_door_data: LockedDoorData,
        objectives: Vec<Objective>,
        base_links_data: LinksDataGroup,
        randomizer_settings: RandomizerSettings,
        difficulty_tiers: Vec<DifficultyConfig>,
        map: Map,
    ) -> JoinHandle<Result<()>> {
        let arc = self.start_location.clone();
        let game_data = self.game_data.clone();

        tokio::spawn(async move {
            let mut rng = StdRng::from_entropy();

            let game_data2 = game_data.clone();
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

            match update_hub_location_impl(
                game_data,
                randomizer,
                initial_global_state,
                start_location,
                &locked_door_data,
                &objectives,
                &randomizer_settings,
                &difficulty_tiers
            ).await {
                Ok((hub_location, hub_obtain_route, hub_return_route)) => {
                    let mut lock = arc.lock().unwrap();
                    lock.hub_location = hub_location;
                    lock.hub_obtain_route = hub_obtain_route;
                    lock.hub_return_route = hub_return_route;
                    Ok(())
                }
                Err(err) => Err(err)
            }
        })
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
        custom_escape_time: Option<usize>
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

            let (hub_location, hub_obtain_route, hub_return_route) = update_hub_location_impl(
                game_data,
                randomizer,
                initial_global_state.clone(),
                start_location.clone(),
                &locked_door_data,
                &objectives,
                &randomizer_settings,
                &difficulty_tiers
            ).await?;

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
            let start_location_data = StartLocationData {
                start_location: start_location,
                hub_location: hub_location.clone(),
                hub_obtain_route: hub_obtain_route.clone(),
                hub_return_route: hub_return_route.clone()
            };

            let (r, s) = update_randomization_impl(
                game_data3,
                randomizer,
                initial_global_state,
                initial_local_state,
                start_location_data,
                &locked_door_data,
                &item_locations,
                &spoiler_overrides,
                &randomizer_settings,
                &difficulty_tiers,
                &map,
                custom_escape_time
            ).await;

            let mut lock = arc_hub.lock().unwrap();
            lock.hub_location = hub_location;
            lock.hub_obtain_route = hub_obtain_route;
            lock.hub_return_route = hub_return_route;
            let mut lock = arc_r.lock().unwrap();
            *lock = Some((r, s));

            Ok(())
        })
    }

}

async fn update_randomization_impl(
    game_data: Arc<GameData>,
    randomizer: Randomizer<'_>,
    initial_global_state: GlobalState,
    initial_local_state: LocalState,
    start_location_data: StartLocationData,
    locked_door_data: &LockedDoorData,
    item_locations: &[Item],
    spoiler_overrides: &[SpoilerOverride],
    randomizer_settings: &RandomizerSettings,
    difficulty_tiers: &[DifficultyConfig],
    map: &Map,
    custom_escape_time: Option<usize>
) -> (Randomization, SpoilerLog) {
    let initial_item_location_state = ItemLocationState {
        placed_item: None,
        collected: false,
        reachable_step: None,
        bireachable: false,
        bireachable_vertex_id: None,
        difficulty_tier: None,
    };
    let initial_flag_location_state = FlagLocationState {
        reachable_step: None,
        reachable_vertex_id: None,
        bireachable: false,
        bireachable_vertex_id: None,
    };
    let initial_save_location_state = SaveLocationState { bireachable: false };
    let initial_door_state = DoorState {
        bireachable: false,
        bireachable_vertex_id: None,
    };

    let mut state = RandomizationState {
        step_num: 1,
        start_location: start_location_data.start_location.clone(),
        hub_location: start_location_data.hub_location.clone(),
        hub_obtain_route: start_location_data.hub_obtain_route.clone(),
        hub_return_route: start_location_data.hub_return_route.clone(),
        item_precedence: Vec::new(),
        save_location_state: vec![initial_save_location_state; game_data.save_locations.len()],
        item_location_state: vec![initial_item_location_state; game_data.item_locations.len()],
        flag_location_state: vec![initial_flag_location_state; game_data.flag_ids.len()],
        door_state: vec![initial_door_state; locked_door_data.locked_doors.len()],
        items_remaining: randomizer.initial_items_remaining.clone(),
        starting_local_state: initial_local_state,
        global_state: initial_global_state,
        debug_data: None,
        previous_debug_data: None,
        key_visited_vertices: HashSet::new(),
        last_key_areas: Vec::new(),
    };

    randomizer.update_reachability(&mut state);

    for i in 0..state.item_location_state.len() {
        if item_locations[i] != Item::Nothing {
            state.item_location_state[i].placed_item = Some(item_locations[i]);
        }
    }

    let mut spoiler_summary_vec = vec![];
    let mut spoiler_details_vec = vec![];
    let mut debug_data_vec: Vec<DebugData> = Vec::new();

    let max_override_step = spoiler_overrides.iter().map(|x| x.step).reduce(|acc, e| acc.max(e)).unwrap_or_default();

    loop {
        let (spoiler_summary, spoiler_details) = update_step(
            &mut state,
            &randomizer,
            &game_data,
            &randomizer_settings,
            &difficulty_tiers[0].tech,
            &map,
            item_locations,
            spoiler_overrides
        );
        let any_progress = spoiler_summary.items.len() > 0 || spoiler_summary.flags.len() > 0;
        spoiler_summary_vec.push(spoiler_summary);
        spoiler_details_vec.push(spoiler_details);
        debug_data_vec.push(state.previous_debug_data.as_ref().unwrap().clone());

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
        spoiler_summary_vec,
        spoiler_details_vec,
        debug_data_vec,
        seed as usize,
        seed as usize,
        &mut rng
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

    (r, s)
}

fn update_step(
    state: &mut RandomizationState,
    randomizer: &Randomizer<'_>,
    game_data: &GameData,
    randomizer_settings: &RandomizerSettings,
    tech: &[bool],
    map: &Map,
    item_locations: &[Item],
    spoiler_overrides: &[SpoilerOverride],
) -> (SpoilerSummary, SpoilerDetails) {
    let orig_global_state = state.global_state.clone();
    let mut spoiler_flag_summaries: Vec<SpoilerFlagSummary> = Vec::new();
    let mut spoiler_flag_details: Vec<SpoilerFlagDetails> = Vec::new();
    let mut spoiler_door_summaries: Vec<SpoilerDoorSummary> = Vec::new();
    let mut spoiler_door_details: Vec<SpoilerDoorDetails> = Vec::new();
    loop {
        let mut any_update = false;
        for (i, &flag_id) in game_data.flag_ids.iter().enumerate() {
            if state.global_state.flags[flag_id] {
                continue;
            }
            if state.flag_location_state[i].reachable_step.is_some() && flag_id == game_data.mother_brain_defeated_flag_id {
                any_update = true;
                let flag_vertex_id = state.flag_location_state[i].reachable_vertex_id.unwrap();
                spoiler_flag_summaries.push(randomizer.get_spoiler_flag_summary(&state, flag_vertex_id, flag_id));
                spoiler_flag_details.push(randomizer.get_spoiler_flag_details_one_way(&state, flag_vertex_id, flag_id, i));
                state.global_state.flags[flag_id] = true;
            } else if state.flag_location_state[i].bireachable {
                any_update = true;
                let flag_vertex_id = state.flag_location_state[i].bireachable_vertex_id.unwrap();
                spoiler_flag_summaries.push(randomizer.get_spoiler_flag_summary(&state, flag_vertex_id, flag_id));
                spoiler_flag_details.push(randomizer.get_spoiler_flag_details(&state, flag_vertex_id, flag_id, i));
                state.global_state.flags[flag_id] = true;
            }
        }
        for i in 0..randomizer.locked_door_data.locked_doors.len() {
            if state.global_state.doors_unlocked[i] {
                continue;
            }
            if state.door_state[i].bireachable {
                any_update = true;
                let door_vertex_id = state.door_state[i].bireachable_vertex_id.unwrap();
                spoiler_door_summaries.push(randomizer.get_spoiler_door_summary(door_vertex_id, i));
                spoiler_door_details.push(randomizer.get_spoiler_door_details(&state, door_vertex_id, i));
                state.global_state.doors_unlocked[i] = true;
            }
        }
        if any_update {
            randomizer.update_reachability(state);
        } else {
            break;
        }
    }

    let mut placed_uncollected_bireachable_loc: Vec<ItemLocationId> = Vec::new();
    let mut placed_uncollected_bireachable_items: Vec<Item> = Vec::new();
    for (i, item_location_state) in state.item_location_state.iter().enumerate() {
        if let Some(item) = item_location_state.placed_item {
            if !item_location_state.collected && item_location_state.bireachable {
                placed_uncollected_bireachable_loc.push(i);
                placed_uncollected_bireachable_items.push(item);
            }
        }
    }

    let mut new_state = RandomizationState {
        step_num: state.step_num + 1,
        start_location: state.start_location.clone(),
        hub_location: state.hub_location.clone(),
        hub_obtain_route: state.hub_obtain_route.clone(),
        hub_return_route: state.hub_return_route.clone(),
        item_precedence: state.item_precedence.clone(),
        item_location_state: state.item_location_state.clone(),
        flag_location_state: state.flag_location_state.clone(),
        save_location_state: state.save_location_state.clone(),
        door_state: state.door_state.clone(),
        items_remaining: state.items_remaining.clone(),
        global_state: state.global_state.clone(),
        starting_local_state: get_initial_local_state(state, game_data),
        debug_data: None,
        previous_debug_data: None,
        key_visited_vertices: HashSet::new(),
        last_key_areas: Vec::new()
    };
    new_state.previous_debug_data = state.debug_data.clone();
    new_state.key_visited_vertices = state.key_visited_vertices.clone();

    for &item in &placed_uncollected_bireachable_items {
        new_state.global_state.collect(item, &game_data, randomizer_settings.item_progression_settings.ammo_collect_fraction, tech, &mut new_state.starting_local_state);
    }
    // Add overrides to the current step
    let overrides: Vec<_> = spoiler_overrides.iter().filter(|x| x.step == state.step_num).collect();
    for item_override in &overrides {
        let item = item_locations[item_override.item_idx];
        new_state.global_state.collect(item, &game_data, randomizer_settings.item_progression_settings.ammo_collect_fraction, tech, &mut new_state.starting_local_state);
    }

    randomizer.update_reachability(&mut new_state);

    for &loc in &placed_uncollected_bireachable_loc {
        new_state.item_location_state[loc].collected = true;
    }

    let mut spoiler_summary = randomizer.get_spoiler_summary(
        &orig_global_state,
        &state,
        &new_state,
        spoiler_flag_summaries,
        spoiler_door_summaries
    );
    let mut spoiler_details = randomizer.get_spoiler_details(
        &orig_global_state,
        &state,
        &new_state,
        spoiler_flag_details,
        spoiler_door_details
    );

    // Mark items as collected after getting spoiler data as they are not logically bireachable
    for item_override in overrides {
        let state = &mut new_state.item_location_state[item_override.item_idx];
        if state.collected {
            continue;
        }
        state.collected = true;
        state.bireachable = true;
        state.bireachable_vertex_id = game_data.item_vertex_ids[item_override.item_idx].first().copied();
        state.reachable_step = Some(new_state.step_num);

        let item = item_locations[item_override.item_idx];
        let item_str: String = Item::VARIANTS[item as usize].to_string();
        let (room_id, node_id) = game_data.item_locations[item_override.item_idx];
        let vertex_info = get_vertex_info_by_id(room_id, node_id, &game_data, map);

        // Dummy fill spoiler summary and details
        spoiler_summary.items.push(SpoilerItemSummary {
            item: item_str.clone(),
            location: SpoilerLocation {
                area: vertex_info.area_name.clone(),
                room_id: vertex_info.room_id,
                room: vertex_info.room_name.clone(),
                node_id: vertex_info.node_id,
                node: vertex_info.node_name.clone(),
                coords: vertex_info.room_coords
            }
        });
        spoiler_details.items.push(SpoilerItemDetails {
            item: item_str,
            location: SpoilerLocation {
                area: vertex_info.area_name,
                room_id: vertex_info.room_id,
                room: vertex_info.room_name,
                node_id: vertex_info.node_id,
                node: vertex_info.node_name,
                coords: vertex_info.room_coords
            },
            reachable_step: new_state.step_num,
            difficulty: Some("Custom".to_string()),
            obtain_route: Vec::new(),
            return_route: Vec::new()
        });
    }

    *state = new_state;
    (spoiler_summary, spoiler_details)
}

fn get_initial_local_state(state: &RandomizationState, game_data: &GameData) -> LocalState {
    let start_vertex_id = game_data.vertex_isv.index_by_key[&VertexKey {
        room_id: state.hub_location.room_id,
        node_id: state.hub_location.node_id,
        obstacle_mask: 0,
        actions: vec![],
    }];
    let cost_metric_idx = 0; // use energy-sensitive cost metric
    let forward_traverse = &state.debug_data.as_ref().unwrap().forward;
    forward_traverse.local_states[start_vertex_id][cost_metric_idx]
}

async fn update_hub_location_impl(
    game_data: Arc<GameData>,
    randomizer: Randomizer<'_>,
    global: GlobalState,
    start_location: StartLocation,
    locked_door_data: &LockedDoorData,
    objectives: &[Objective],
    randomizer_settings: &RandomizerSettings,
    difficulty_tiers: &[DifficultyConfig],
) -> Result<(HubLocation, Vec<SpoilerRouteEntry>, Vec<SpoilerRouteEntry>)> {
    if start_location.room_id == 8 && start_location.node_id == 5 && start_location.x == 72.0 && start_location.y == 69.5 {
        let ship_hub = Plando::get_ship_hub(&game_data);
        return Ok((ship_hub, Vec::new(), Vec::new()));
    }

    let num_vertices = game_data.vertex_isv.keys.len();
    let start_vertex_id = game_data.vertex_isv.index_by_key[&VertexKey {
        room_id: start_location.room_id,
        node_id: start_location.node_id,
        obstacle_mask: 0,
        actions: vec![],
    }];

    let local = apply_requirement(
        &start_location.requires_parsed.as_ref().unwrap(),
        &global,
        LocalState::full(),
        false,
        randomizer_settings,
        &difficulty_tiers[0],
        &game_data,
        &randomizer.door_map,
        locked_door_data,
        objectives,
    );
    if local.is_none() {
        bail!("Invalid start location");
    }
    let forward = traverse(
        &randomizer.base_links_data,
        &randomizer.seed_links_data,
        None,
        &global,
        local.unwrap(),
        num_vertices,
        start_vertex_id,
        false,
        randomizer_settings,
        &difficulty_tiers[0],
        &game_data,
        &randomizer.door_map,
        locked_door_data,
        objectives,
        randomizer.next_traversal_number.borrow_mut().deref_mut()
    );
    let reverse = traverse(
        &randomizer.base_links_data,
        &randomizer.seed_links_data,
        None,
        &global,
        LocalState::full(),
        num_vertices,
        start_vertex_id,
        true,
        randomizer_settings,
        &difficulty_tiers[0],
        &game_data,
        &randomizer.door_map,
        locked_door_data,
        objectives,
        randomizer.next_traversal_number.borrow_mut().deref_mut()
    );

    let mut best_hub_vertex_id = start_vertex_id;
    let mut best_hub_cost = global.inventory.max_energy - 1;
    for &(hub_vertex_id, ref hub_req) in [(start_vertex_id, Requirement::Free)].iter().chain(game_data.hub_farms.iter()) {
        if get_bireachable_idxs(&global, hub_vertex_id, &forward, &reverse).is_none() {
            continue;
        }

        let new_local = apply_requirement(
            hub_req,
            &global,
            LocalState::empty(&global),
            false,
            randomizer_settings,
            &difficulty_tiers[0],
            &game_data,
            &randomizer.door_map,
            locked_door_data,
            objectives
        );

        let hub_cost = match new_local {
            Some(loc) => loc.energy_used,
            None => Capacity::MAX
        };
        if hub_cost < best_hub_cost {
            best_hub_cost = hub_cost;
            best_hub_vertex_id = hub_vertex_id;
        }
    }

    let Some((forward_cost_idx, reverse_cost_id)) = get_bireachable_idxs(&global, best_hub_vertex_id, &forward, &reverse)
    else {
        bail!("Inconsistent result from get_bireachable_idxs")
    };

    let vertex_key = game_data.vertex_isv.keys[best_hub_vertex_id].clone();
    let hub_location = HubLocation {
        room_id: vertex_key.room_id,
        node_id: vertex_key.node_id,
        vertex_id: best_hub_vertex_id
    };

    let hub_obtain_link_idxs = get_spoiler_route(&forward, best_hub_vertex_id, forward_cost_idx);
    let hub_return_link_idxs = get_spoiler_route(&reverse, best_hub_vertex_id, reverse_cost_id);

    let hub_obtain_route = randomizer.get_spoiler_route(
        &global,
        local.unwrap(),
        &hub_obtain_link_idxs,
        &difficulty_tiers[0],
        false
    );
    let hub_return_route = randomizer.get_spoiler_route(
        &global,
        LocalState::full(),
        &hub_return_link_idxs,
        &difficulty_tiers[0],
        true
    );

    Ok((hub_location, hub_obtain_route, hub_return_route))
}