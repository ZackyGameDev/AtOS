# I'm gonna be honest i asked chatgpt to generate this idk how this works
TARGET = aarch64-unknown-none
KERNEL = at-os
BUILD = target/$(TARGET)/release/$(KERNEL)

OBJCOPY = aarch64-linux-gnu-objcopy

QEMU = qemu-system-aarch64

# Default target
all: kernel8.img

# Build release and debug
build:
	cargo build --release --target $(TARGET)
	cargo build --target $(TARGET)

# Convert ELF to raw binary
kernel8.img: build
	$(OBJCOPY) $(BUILD) -O binary kernel8.img

# Run in QEMU (Emulating Raspberry Pi 3B+ with Mini UART redirected to terminal)
run:
	$(QEMU) -M raspi3b -kernel kernel8.img -serial null -serial stdio

debug:
	$(QEMU) \
		-M raspi3b \
		-kernel kernel8.img \
		-serial null \
		-serial stdio \
		-S \
		-gdb tcp::1234

# Clean everything
clean:
	cargo clean
	rm -f kernel8.img