default:
    @just --list

test:
    cargo test --lib -p cloudpoint_lib

build:
    cargo 3ds build --release

run ip:
    cargo 3ds run -a {{ ip }} --release

[working-directory('cloudpoint_app/cia')]
cia major minor micro: build
    ./bannertool makesmdh \
        -f visible,nosavebackups \
        -s "Cloudpoint" \
        -l "Cloudpoint save & extdata sync" \
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
    ./makerom -f cia -v \
        -ver $(just ver {{ major }} {{ minor }} {{ micro }}) \
        -target t \
        -elf ../../target/armv6k-nintendo-3ds/release/cloudpoint.elf \
        -DROMFS_PATH="../romfs" \
        -rsf cloudpoint.rsf \
        -icon cloudpoint.smdh \
        -banner banner.bnr \
        -logo logo.bcma.lz \
        -o cloudpoint.cia

ver major minor micro:
    @[ {{ major }} -le 63 ] || { echo "error: major max 63" >&2; exit 1; }
    @[ {{ minor }} -le 63 ] || { echo "error: minor max 63" >&2; exit 1; }
    @[ {{ micro }} -le 15 ] || { echo "error: micro max 15" >&2; exit 1; }
    @echo $(( ({{ major }} << 10) | ({{ minor }} << 4) | {{ micro }} ))

deploy:
    scp cloudpoint_app/cia/cloudpoint.cia root@62.238.18.193:/mnt/data

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

[working-directory('cloudpoint_app')]
pack-icons:
    tex3ds --format rgba8888 \
        --compress auto \
        --atlas \
        `ls icons/*.png | sort` \
        -o romfs/icons.t3x \
        --header src/ctr_gfx/icons/icons.h
    bindgen src/ctr_gfx/icons/icons.h \
        --no-layout-tests \
        --raw-line '// @generated — run `just pack-icons` to regenerate' \
        -o src/ctr_gfx/icons/bindings.rs
    sed -i 's/icons_\([a-z_]*\)_idx/ICON_\U\1/g' src/ctr_gfx/icons/bindings.rs

docker-build-server:
    cargo clean
    docker build --ssh default -t dwalker109/cloudpoint -f cloudpoint_server/Dockerfile .

docker-run-server-local:
    docker compose -f cloudpoint_server/compose.local.yml up
