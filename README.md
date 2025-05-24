maprando-plando is a WiP Super Metroid Plandomizer dependent on https://github.com/blkerby/MapRandomizer

Credits go out to the MapRandomizer dev-team, the team behind the sm-json-data project (https://github.com/vg-json-data/sm-json-data), the Mosaic Tilesets team (https://github.com/blkerby/Mosaic/) and the custom Samus sprite artists.

## How to use

1. Download the dependencies from the [Initial Release](https://github.com/Noktuska/maprando-plando/releases/tag/v0.1.0)
2. Replace the .exe with the [Newest Release](https://github.com/Noktuska/maprando-plando/releases)

### Download Map Repositories
To use the maprando Map Pool, in the menu bar of the Application, hit Map -> Download Map Repositories. This will start a download of roughly 2.5 GB.

### Plandomize the Map
You can change the logic settings (Difficulty/Starting Items/etc.) under Settings -> Logic Settings in the menu bar. You will be able to Save/Load your custom logic settings, so you can send them to other people for them to make a Plando using your preferred difficulty.

Start by selecting the Start Location/Items/Doors from the right sidebar and place them into the world by left clicking on a valid location. To reset the start position to the Ship, or to remove an Item/Door, right click on the corresponding icon on the map.

The Spoiler Log will automatically update as you place Items, or if you disable automatic spoiler updates, you can press F5 to update it manually. The Spoiler Log will also provide Obtain/Return routes for each Item and Flag by clicking on it in the Spoiler Summary Window or the Map while no Item in the Sidebar is selected. The Plando does *not* have to be logically beatable for you to create it, it mainly functions as a guide to keep it as closely to something the Randomizer would generate.

Remember to frequently Save your seed by hitting File -> Save Seed to not lose progress in case you want to revert changes, or a crash, as the Program is still in Beta. You can load the seed from File -> Load Seed again.

### Patch/Share the Seed
To share the seed for someone else to play it, please share the JSON File created by hitting File -> Save Seed. The Player then can patch the ROM themselves without getting spoiled, by hitting File -> Patch ROM from Seed File while allowing them to customize their ROM with custom Sprites/Room Tiles as they would in Maprando.

If you want to test your own seed, you can patch the seed that is currently loaded directly by hitting File -> Patch ROM. Please refrain from sharing ROMs.

### Map Editor
You can roll Maps from the Mappool under the "Map" Menu Button. You can also load specific Maps from a JSON file, if you have a favorite Map you would like to use.

With the "Map Editor" Menu Button, you can open up the Map Editor to modify the current Map of the Seed you are working on. You can move around rooms by left-click and dragging them, select multiple rooms by making a selection, and temporarily remove rooms by right clicking on them.

Orphaned doors are highlighted red and deleted Rooms can be spawned back in from the sidebar. The sidebar also functions as a room searcher.

At the top of the sidebar you can change the dropdown to also modify the Areas of the map, allowing you to fully customize both Areas, which affect what a Map Stations unlock, as well as Subareas, which mainly affect music that is playing within an Area.

You can Apply/Discard the changes you make also under the "Map Editor" Menu Button. For a Map to be valid, it currently places the restrictions for all Rooms to be in a 72x72 grid, no doors to be orphaned, all rooms have to be present, no single Area can exceed a 64x32 boundary, Phantoon's Map Station and Phantoon's Room need to be connected through a singular room, all of the same Area, each Area can only have one Map Station and there can only be 23 maximum Area Transitions.

You can Save/Load your custom maps under tha "Map" Menu Button.

## How to Build
1. Clone the repository with recursive submodules enabled
```sh
git clone --recurse-submodules https://github.com/Noktuska/maprando-plando.git
cd maprando-plando
```
2. Setup the MapRandomizer
```sh
sh MapRandomizer/scripts/download_data.sh
```
3. Run the setup.sh script to copy needed dependencies from the MapRandomizer submodule
```sh
sh maprando-plando/setup.sh
```
4. Build the maprando-plando project
```sh
cd maprando-plando
cargo build
```
5. Download [SFML](https://www.sfml-dev.org/download/sfml/2.6.1/) and copy the DLLs from the SFML-2.6.1/bin/ folder into the maprando-plando/target/debug/ folder next to the executable
6. Run the maprando-plando project
```sh
cargo run
```