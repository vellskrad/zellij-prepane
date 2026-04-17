.PHONY: build install

build:
	cargo build --release --target wasm32-wasip1

install: build
	mkdir -p "$(HOME)/.config/zellij/plugins"
	cp "target/wasm32-wasip1/release/prepane.wasm" "$(HOME)/.config/zellij/plugins/prepane.wasm"
