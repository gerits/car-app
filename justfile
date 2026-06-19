default:
    @just --list

# Build the workspace in debug mode
build:
    cargo build

# Build the workspace in release mode
build-release:
    cargo build --release

# Run the car-app. Specify a map file path with MAP (e.g., just run target/data/map.pmtiles)
run MAP="":
    #!/usr/bin/env bash
    if [ -n "{{MAP}}" ]; then
        abs_path=$(realpath "{{MAP}}")
        echo "Running car-app with map: $abs_path"
        CAR_APP_MAP_PATH="$abs_path" cargo run --bin car-app
    else
        echo "Running car-app with default/baked-in map..."
        cargo run --bin car-app
    fi

# Run the car-app with interactive map selection
run-interactive:
    #!/usr/bin/env bash
    echo "=== Car App Local Map Launcher ==="
    echo "Searching for available map files (*.mbtiles, *.pmtiles)..."
    map_files=()
    while IFS= read -r file; do
        if [ -f "$file" ]; then
            map_files+=("$file")
        fi
    done < <( { find . \( -name "*.mbtiles" -o -name "*.pmtiles" \) -not -path "*/target/*" -not -path "*/.*" ; \
               if [ -d "target/assets" ]; then find target/assets \( -name "*.mbtiles" -o -name "*.pmtiles" \) -not -path "*/.*"; fi; \
               if [ -d "target/data" ]; then find target/data \( -name "*.mbtiles" -o -name "*.pmtiles" \) -not -path "*/.*"; fi; } | sort -u )
    if [ ${#map_files[@]} -eq 0 ]; then
        echo "No map files found in the repository."
        echo "Using default/baked-in test map (car-app/assets/map.mbtiles)."
        cargo run --bin car-app
        exit 0
    fi
    echo ""
    echo "Select which map to run the car-app with:"
    echo "1) Default/baked-in test map (car-app/assets/map.mbtiles)"
    index=2
    for file in "${map_files[@]}"; do
        if [ "$file" = "./car-app/assets/map.mbtiles" ] || [ "$file" = "car-app/assets/map.mbtiles" ]; then
            continue
        fi
        echo "$index) $file"
        index=$((index+1))
    done
    echo ""
    read -p "Enter choice [1-$((index-1))]: " choice
    if [ -z "$choice" ]; then
        choice=1
    fi
    if [ "$choice" -eq 1 ]; then
        echo "Running with default/baked-in test map..."
        cargo run --bin car-app
    else
        current_choice=2
        selected_file=""
        for file in "${map_files[@]}"; do
            if [ "$file" = "./car-app/assets/map.mbtiles" ] || [ "$file" = "car-app/assets/map.mbtiles" ]; then
                continue
            fi
            if [ "$current_choice" -eq "$choice" ]; then
                selected_file="$file"
                break
            fi
            current_choice=$((current_choice+1))
        done
        if [ -n "$selected_file" ]; then
            abs_path=$(pwd)/${selected_file#./}
            echo "Running with selected map: $selected_file"
            CAR_APP_MAP_PATH="$abs_path" cargo run --bin car-app
        else
            echo "Invalid choice. Exiting."
            exit 1
        fi
    fi

# Run the map-tool member
run-map-tool:
    cargo run --bin map-tool

# Run unit tests across the workspace
test:
    cargo test

# Format codebase
fmt:
    cargo fmt --all

# Check formatting without applying changes
fmt-check:
    cargo fmt --all -- --check

# Run clippy linter on the workspace
lint:
    cargo clippy --all-targets --all-features

# Clean cargo target directory
clean:
    cargo clean
