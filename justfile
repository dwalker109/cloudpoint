default:
    @just --list

test:
    cargo test --lib

build:
    cargo 3ds build --release

run ip:
    cargo 3ds run -a {{ ip }} --release

[working-directory('cloudpoint_app/cia')]
cia ver: build
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
        -ver {{ ver }} \
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

[working-directory('cloudpoint_app/src/ctr_gfx/c2d')]
citro2d:
    bindgen wrapper.h \
        --wrap-static-fns \
        --wrap-static-fns-path extern.c \
        --allowlist-function "C2D_.*" \
        --allowlist-function "C3D_.*" \
        --allowlist-type "C2D_.*" \
        --allowlist-type "C3D_.*" \
        --allowlist-var "C2D_.*" \
        --allowlist-var "C3D_.*" \
        --allowlist-type "gfxScreen_t" \
        --allowlist-type "gfx3dSide_t" \
        --allowlist-var "gfxScreen_t_.*" \
        --allowlist-var "gfx3dSide_t_.*" \
        --no-layout-tests \
        > bindings.rs 2>/dev/null \
        -- \
        --target=arm-none-eabi \
        -march=armv6k \
        -mtune=mpcore \
        -mfloat-abi=hard \
        -mfpu=vfpv2 \
        -mtp=soft \
        -D__3DS__ \
        -DARM11 \
        -I/opt/devkitpro/libctru/include \
        -I/opt/devkitpro/devkitARM/arm-none-eabi/include
    arm-none-eabi-gcc -c extern.c \
        -o /tmp/extern.o \
        -I . \
        -I/opt/devkitpro/libctru/include \
        -I/opt/devkitpro/devkitARM/arm-none-eabi/include \
        -march=armv6k -mtune=mpcore -mfloat-abi=hard \
        -mfpu=vfpv2 -mtp=soft -D__3DS__ -DARM11 -O2
    arm-none-eabi-ar rcs libextern.a /tmp/extern.o
