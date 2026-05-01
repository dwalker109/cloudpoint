default:
    @just --list

test:
    cargo test --lib

build:
    cargo 3ds build --release

run ip:
    cargo 3ds run -a {{ ip }} --release

[working-directory('cloudpoint_app/cia')]
cia major minor micro: build
    ./bannertool makesmdh \
        -f visible,nosavebackups \
        -s "Cloudpoint" \
        -l "Cloudpoint save and extdata sync" \
        -p "Dan Walker" \
        -i icon.png \
        -o cloudpoint.smdh
    ./bannertool makecwav \
        -i banner.wav \
        -o banner.bcwav
    ./bannertool makebanner \
        -i banner.png \
        -ca banner.bcwav \
        -o banner.bnr
    ./makerom -f cia \
        -major {{ major }} -minor {{ minor }} -micro {{ micro }} \
        -target t \
        -elf ../../target/armv6k-nintendo-3ds/release/cloudpoint_app.elf \
        -DROMFS_PATH="../romfs" \
        -rsf cloudpoint.rsf \
        -icon cloudpoint.smdh \
        -banner banner.bnr \
        -logo logo.bcma.lz \
        -o cloudpoint.cia
    rm banner.bcwav banner.bnr cloudpoint.smdh

deploy:
    scp cloudpoint_app/cia/cloudpoint.cia root@62.238.18.193:/root/data
