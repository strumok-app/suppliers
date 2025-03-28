.DEFAULT_GOAL := help
PROJECTNAME=$(shell basename "$(PWD)")
SOURCES=$(sort $(wildcard ./src/*.rs ./src/**/*.rs))

OS_NAME=$(shell uname | tr '[:upper:]' '[:lower:]')
PATH := $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/$(OS_NAME)-x86_64/bin:$(PATH)

ANDROID_AARCH64_LINKER=$(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/$(OS_NAME)-x86_64/bin/aarch64-linux-android29-clang
ANDROID_ARMV7_LINKER=$(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/$(OS_NAME)-x86_64/bin/armv7a-linux-androideabi29-clang
ANDROID_I686_LINKER=$(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/$(OS_NAME)-x86_64/bin/i686-linux-android29-clang
ANDROID_X86_64_LINKER=$(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/$(OS_NAME)-x86_64/bin/x86_64-linux-android29-clang

SHELL := /bin/bash

# ##############################################################################
# # GENERAL
# ##############################################################################

.PHONY: help
help: makefile
	@echo
	@echo " Available actions in "$(PROJECTNAME)":"
	@echo
	@sed -n 's/^##//p' $< | column -t -s ':' |  sed -e 's/^/ /'
	@echo

# rustup target add aarch64-apple-ios x86_64-apple-ios
# rustup target add aarch64-apple-darwin x86_64-apple-darwin
# rustup target add i686-linux-android
# rustup target add x86_64-linux-android
# @if [ $$(uname) == "Darwin" ] ; then cargo install cargo-lipo ; fi
# cargo install cbindgen

## android-init: Install missing dependencies.
.PHONY: android-init
android-init:
	rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

## :

# ##############################################################################
# # RECIPES
# ##############################################################################

## all: Compile Android and Linux
all: android linux

# ios: Compile the iOS universal library
# ios: target/universal/release/libexample.a

# target/universal/release/libexample.a: $(SOURCES) ndk-home
# 	@if [ $$(uname) == "Darwin" ] ; then \
# 		cargo lipo --release ; \
# 		else echo "Skipping iOS compilation on $$(uname)" ; \
# 	fi
# 	@echo "[DONE] $@"

# macos: Compile the macOS libraries
# macos: target/x86_64-apple-darwin/release/libexample.dylib target/aarch64-apple-darwin/release/libexample.dylib

# target/x86_64-apple-darwin/release/libexample.dylib: $(SOURCES)
# 	@if [ $$(uname) == "Darwin" ] ; then \
# 		cargo lipo --release --targets x86_64-apple-darwin ; \
# 		else echo "Skipping macOS compilation on $$(uname)" ; \
# 	fi
# 	@echo "[DONE] $@"

# target/aarch64-apple-darwin/release/libexample.dylib: $(SOURCES)
# 	@if [ $$(uname) == "Darwin" ] ; then \
# 		cargo lipo --release --targets aarch64-apple-darwin ; \
# 		else echo "Skipping macOS compilation on $$(uname)" ; \
# 	fi
# 	@echo "[DONE] $@"

## android: Compile the android targets (arm64, armv7 and i686)
android: android-init target/aarch64-linux-android/release target/armv7-linux-androideabi/release target/x86_64-linux-android/release
 #target/i686-linux-android/release target/x86_64-linux-android/release

## linux: Compile Linux
linux: target/x86_64-unknown-linux-gnu/release

target/aarch64-linux-android/release: $(SOURCES) ndk-home
	CC_aarch64_linux_android=$(ANDROID_AARCH64_LINKER) \
	CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$(ANDROID_AARCH64_LINKER) \
		cargo build --target aarch64-linux-android --release
	@echo "[DONE] $@"

target/armv7-linux-androideabi/release: $(SOURCES) ndk-home
	CC_armv7_linux_androideabi=$(ANDROID_ARMV7_LINKER) \
	CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER=$(ANDROID_ARMV7_LINKER) \
		cargo build --target armv7-linux-androideabi --release
	@echo "[DONE] $@"

# target/i686-linux-android/release: $(SOURCES) ndk-home
# 	CC_i686_linux_android=$(ANDROID_I686_LINKER) \
# 	CARGO_TARGET_I686_LINUX_ANDROID_LINKER=$(ANDROID_I686_LINKER) \
# 		cargo  build --target i686-linux-android --release
# 	@echo "[DONE] $@"

target/x86_64-linux-android/release: $(SOURCES) ndk-home
	CC_x86_64_linux_android=$(ANDROID_X86_64_LINKER) \
	CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=$(ANDROID_X86_64_LINKER) \
		cargo build --target x86_64-linux-android --release
	@echo "[DONE] $@"

target/x86_64-unknown-linux-gnu/release: 
	cargo  build --target x86_64-unknown-linux-gnu --release
	@echo "[DONE] $@"
		
.PHONY: ndk-home
ndk-home:
	@if [ ! -d "${ANDROID_NDK_HOME}" ] ; then \
		echo "Error: Please, set the ANDROID_NDK_HOME env variable to point to your NDK folder" ; \
		exit 1 ; \
	fi

# bindings: Generate the .h file for iOS
# bindings: target/bindings.h

# target/bindings.h: $(SOURCES)
# 	cbindgen $^ -c cbindgen.toml | grep -v \#include | uniq > $@
# 	@echo "[DONE] $@"

## :

# ##############################################################################
# # OTHER
# ##############################################################################

## clean:
.PHONY: clean
clean:
	cargo clean
	rm -f target/bindings.h target/bindings.src.h

## test:
.PHONY: test
test:
	cargo test
