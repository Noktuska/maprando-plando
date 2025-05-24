echo Copying files from MapRandomizer...
cd maprando-plando
mkdir -p data/maprando-data
mkdir -p data/maps

cp -r ../MapRandomizer/rust/data ./data/maprando-data
cp -r ../MapRandomizer/patches ./data
cp -r ../MapRandomizer/gfx ./data
cp -r ../MapRandomizer/visualizer ./data
cp -r ../MapRandomizer/MapRandoSprites ./data
cp -r ../MapRandomizer/sm-json-data ./data
cp ../MapRandomizer/room_geometry.json ./data/room_geometry.json
mkdir -p ./data/TitleScreen/Images
cp -r ../MapRandomizer/TitleScreen/Images ./data/TitleScreen
