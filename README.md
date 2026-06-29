# AtOS
A basic Operating System created in rust, for the Raspberry Pi 3+ Hardware

# Guide
There is a guide book written over the development of AtOS. It also serves as a documentation of the design choices. it can be found at [zackygamedev.github.io/RustOSGuideforRPi3/](https://zackygamedev.github.io/RustOSGuideforRPi3/)

# Building
you can just do 
```
make clean
make
```

otherwise for manual effort

```
cargo clean
cargo build --release
```

Then flattening it to the image,

```
aarch64-linux-gnu-objcopy \
target/aarch64-unknown-none/release/at-os \
-O binary kernel8.img
```
