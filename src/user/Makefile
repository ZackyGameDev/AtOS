# THIS MAKE PROCEDURE IS NOW INCLUDED IN THE ROOT KERNEL MAKE PROCEDURE.

TARGET = aarch64-unknown-none
BUILD_DIR = target/$(TARGET)/debug
OUT_DIR = build

OBJCOPY = aarch64-linux-gnu-objcopy

# All binaries in src/bin/
PROGRAMS := $(basename $(notdir $(wildcard src/bin/*.rs)))

.PHONY: all build bin clean

all: bin

build:
	cargo build

bin: build
	mkdir -p $(OUT_DIR)
	mkdir -p $(OUT_DIR)/dump
	@for prog in $(PROGRAMS); do \
		cp $(BUILD_DIR)/$$prog $(OUT_DIR)/$$prog; \
		$(OBJCOPY) -O binary $(BUILD_DIR)/$$prog $(OUT_DIR)/dump/$$prog.bin; \
		echo "Built $$prog.bin"; \
	done

clean:
	cargo clean
	rm -rf $(OUT_DIR)