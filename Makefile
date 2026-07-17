# I'm gonna be honest i asked chatgpt to generate this idk how this works
TARGET = aarch64-unknown-none
KERNEL = AtOS
BUILD = target/$(TARGET)/release/$(KERNEL)

OBJCOPY = aarch64-linux-gnu-objcopy

QEMU = qemu-system-aarch64

USER_DIR = src/user

# Default target
all: kernel8.img

# Build user programs
user:
	$(MAKE) -C $(USER_DIR)

# Build release and debug
build: user
	cargo build --release --target $(TARGET)
	cargo build --target $(TARGET)

# Convert ELF to raw binary
kernel8.img: build
	$(OBJCOPY) $(BUILD) -O binary kernel8.img

# Run in QEMU (Emulating Raspberry Pi 3B+ with Mini UART redirected to terminal)
run:
	$(QEMU) -M raspi3b -kernel kernel8.img -serial null -serial stdio -display none

.PHONY: debug
debug:
	$(QEMU) \
		-M raspi3b \
		-kernel kernel8.img \
		-serial null \
		-serial stdio \
		-S \
		-gdb tcp::1234

gdb:
	gdb target/aarch64-unknown-none/debug/AtOS \
		-ex "target remote :1234"

# Clean everything
clean:
	$(MAKE) -C $(USER_DIR) clean
	cargo clean
	rm -f kernel8.img

.PHONY: all build user clean run debug gdb