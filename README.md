maprando-plando is a WiP Super Metroid Plandomizer dependent on https://github.com/blkerby/MapRandomizer

Credits go out to the MapRandomizer dev-team, the team behind the sm-json-data project (https://github.com/vg-json-data/sm-json-data), the Mosaic Tilesets team (https://github.com/blkerby/Mosaic/) and the custom Samus sprite artists.

## How to use

1. Download and extract the [Newest Release](https://github.com/Noktuska/maprando-plando/releases)
2. Open maprando-plando.exe

### Download Map Repositories
To use the maprando Map Pool, in the menu bar of the Application, hit Map -> Download Map Repositories. This will start a download of roughly 750 MB.

### Plandomize the Map
You can change the logic settings (Difficulty/Starting Items/etc.) under Settings -> Logic Settings in the menu bar. The settings will look similar to the MapRandomizer Generate page and presets can be saved. Custom presets will be saved in a new folder called "custom-presets" in the installation folder.

### Controls
- Pan the view with the Middle Mouse Button
- Select an Item from the sidepanel to the right
- Place the item into the world with the Left Mouse Button
- Remove an item with the Right Mouse Button

The Spoiler Log will automatically update as you place Items, or if you disable automatic spoiler updates, you can press F5 to update it manually. The Spoiler Log will also provide Obtain/Return routes for each Item and Flag by clicking on it in the Spoiler Summary Window or the Map while no Item in the Sidebar is selected. The Plando does *not* have to be logically beatable for you to create it, it mainly functions as a guide to keep it as closely to something the Randomizer would generate.

Remember to frequently Save your seed by hitting File -> Save Seed to not lose progress in case you want to revert changes, or a crash, as the Program is still in Beta. You can load the seed from File -> Load Seed again.

### Sidebar Tabs
- Items: Allows you to select and place/remove the start location, items and door locks. To revert the start location to Ship, simply try to "remove" the currently placed start location
- Rooms: Allows you to search for and spawn in rooms
- Areas: Allows you to configure the 6 Super Metroid Areas and their Sub-Areas to fully customize the experience
- Errors: Shows potential issues with the current Map Layout. Errors need to be fixed before the Plando can be created, Warnings point out potential oversights but aren't a necessity to address
- Benchmark: Displays the amount of time each part of a rendering cycle consumes. Ideally, the "Other (e.g. FPS Limiter)" time should be as high as possible, as this is time the process spends idling. On low-end systems it is recommended to lower the FPS limiter in the Plando Settings under "Settings -> Plando Settings" (default 60 FPS).

### Patch/Share the Seed
To share the seed for someone else to play it, please share the JSON File created by hitting File -> Save Seed. The Player then can patch the ROM themselves without getting spoiled, by hitting File -> Patch ROM from Seed File while allowing them to customize their ROM with custom Sprites/Room Tiles as they would in MapRandomizer.

If you want to test your own seed, you can patch the seed that is currently loaded directly by hitting File -> Patch ROM. Please refrain from sharing ROMs.

### Hotkeys
* +/- to increment/decrement current spoiler step
* F5 to manually update the Spoiler Log
* F6 to open Spoiler Overrides for the current step
* F7 to toggle automatic Spoiler Log updates
* DEL to remove a selected room. You can spawn them back in from the sidebar under the "Rooms" tab
* CTRL+A to select all rooms currently placed

## How to Build
1. Clone the repository with recursive submodules enabled
```sh
git clone --recurse-submodules https://github.com/Noktuska/maprando-plando.git
cd maprando-plando
```
2. Setup the MapRandomizer
```sh
cd MapRandomizer
sh scripts/download_data.sh
cd ..
```
3. Run the setup.sh script to copy needed dependencies from the MapRandomizer submodule
```sh
sh setup.sh
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