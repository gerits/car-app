# Makefile for Car App Workspace

# Use bash for recipes to support array operations and read prompts
SHELL := /bin/bash

.PHONY: help
help: ## Display this help message
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

.PHONY: build
build: ## Build the workspace in debug mode
	cargo build

.PHONY: build-release
build-release: ## Build the workspace in release mode
	cargo build --release

.PHONY: run
run: ## Run the car-app. Use MAP=<path> to specify a map file (e.g., make run MAP=target/data/map.pmtiles)
ifdef MAP
	@abs_path=$$(realpath "$(MAP)"); \
	echo "Running car-app with map: $$abs_path"; \
	CAR_APP_MAP_PATH="$$abs_path" cargo run --bin car-app
else
	@echo "Running car-app with default/baked-in map..."; \
	cargo run --bin car-app
endif

.PHONY: run-interactive
run-interactive: ## Run the car-app with interactive map selection (replaces run_local.sh)
	@echo "=== Car App Local Map Launcher ==="
	@echo "Searching for available map files (*.mbtiles, *.pmtiles)..."
	@map_files=(); \
	while IFS= read -r file; do \
		if [ -f "$$file" ]; then \
			map_files+=("$$file"); \
		fi; \
	done < <( { find . \( -name "*.mbtiles" -o -name "*.pmtiles" \) -not -path "*/target/*" -not -path "*/.*" ; \
	           if [ -d "target/assets" ]; then find target/assets \( -name "*.mbtiles" -o -name "*.pmtiles" \) -not -path "*/.*"; fi; \
	           if [ -d "target/data" ]; then find target/data \( -name "*.mbtiles" -o -name "*.pmtiles" \) -not -path "*/.*"; fi; } | sort -u ); \
	if [ $${#map_files[@]} -eq 0 ]; then \
		echo "No map files found in the repository."; \
		echo "Using default/baked-in test map (car-app/assets/map.mbtiles)."; \
		cargo run --bin car-app; \
		exit 0; \
	fi; \
	echo ""; \
	echo "Select which map to run the car-app with:"; \
	echo "1) Default/baked-in test map (car-app/assets/map.mbtiles)"; \
	index=2; \
	for file in "$${map_files[@]}"; do \
		if [ "$$file" = "./car-app/assets/map.mbtiles" ] || [ "$$file" = "car-app/assets/map.mbtiles" ]; then \
			continue; \
		fi; \
		echo "$$index) $$file"; \
		index=$$((index+1)); \
	done; \
	echo ""; \
	read -p "Enter choice [1-$$((index-1))]: " choice; \
	if [ -z "$$choice" ]; then \
		choice=1; \
	fi; \
	if [ "$$choice" -eq 1 ]; then \
		echo "Running with default/baked-in test map..."; \
		cargo run --bin car-app; \
	else \
		current_choice=2; \
		selected_file=""; \
		for file in "$${map_files[@]}"; do \
			if [ "$$file" = "./car-app/assets/map.mbtiles" ] || [ "$$file" = "car-app/assets/map.mbtiles" ]; then \
				continue; \
			fi; \
			if [ "$$current_choice" -eq "$$choice" ]; then \
				selected_file="$$file"; \
				break; \
			fi; \
			current_choice=$$((current_choice+1)); \
		done; \
		if [ -n "$$selected_file" ]; then \
			abs_path=$$(pwd)/$${selected_file#./}; \
			echo "Running with selected map: $$selected_file"; \
			CAR_APP_MAP_PATH="$$abs_path" cargo run --bin car-app; \
		else \
			echo "Invalid choice. Exiting."; \
			exit 1; \
		fi; \
	fi

.PHONY: run-map-tool
run-map-tool: ## Run the map-tool member
	cargo run --bin map-tool

.PHONY: test
test: ## Run unit tests across the workspace
	cargo test

.PHONY: fmt
fmt: ## Format codebase
	cargo fmt --all

.PHONY: fmt-check
fmt-check: ## Check formatting without applying changes
	cargo fmt --all -- --check

.PHONY: lint
lint: ## Run clippy linter on the workspace
	cargo clippy --all-targets --all-features

.PHONY: clean
clean: ## Clean cargo target directory
	cargo clean
