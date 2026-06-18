#!/bin/bash

# Exit on error
set -e

echo "=== Car App Local Map Launcher ==="
echo "Searching for available map files (*.mbtiles, *.pmtiles)..."

# Find all map files in the workspace, excluding target and hidden directories
MAP_FILES=()
while IFS= read -r file; do
    if [ -f "$file" ]; then
        MAP_FILES+=("$file")
    fi
done < <(find . \( -name "*.mbtiles" -o -name "*.pmtiles" \) -not -path "*/target/*" -not -path "*/.*" | sort)

# Check if we found any maps
if [ ${#MAP_FILES[@]} -eq 0 ]; then
    echo "No map files found in the repository."
    echo "Using default/baked-in test map (assets/map.mbtiles)."
    echo "Running: cargo run --bin car-app"
    cargo run --bin car-app
    exit 0
fi

echo ""
echo "Select which map to run the car-app with:"
echo "1) Default/baked-in test map (assets/map.mbtiles)"

# Display other choices
index=2
for file in "${MAP_FILES[@]}"; do
    # Skip assets/map.mbtiles from the search list since it's choice 1
    if [ "$file" = "./assets/map.mbtiles" ] || [ "$file" = "assets/map.mbtiles" ]; then
        continue
    fi
    echo "$index) $file"
    index=$((index+1))
done

echo ""
read -p "Enter choice [1-$((index-1))]: " choice

# Default to 1 if no choice was entered
if [ -z "$choice" ]; then
    choice=1
fi

if [ "$choice" -eq 1 ]; then
    echo "Running with default/baked-in test map..."
    cargo run --bin car-app
else
    # Map selection choice index back to the file
    current_choice=2
    selected_file=""
    for file in "${MAP_FILES[@]}"; do
        if [ "$file" = "./assets/map.mbtiles" ] || [ "$file" = "assets/map.mbtiles" ]; then
            continue
        fi
        if [ "$current_choice" -eq "$choice" ]; then
            selected_file="$file"
            break
        fi
        current_choice=$((current_choice+1))
    done

    if [ -n "$selected_file" ]; then
        # Get absolute path of the selected map file
        ABS_PATH=$(pwd)/${selected_file#./}
        echo "Running with selected map: $selected_file"
        echo "Command: CAR_APP_MAP_PATH=\"$ABS_PATH\" cargo run --bin car-app"
        CAR_APP_MAP_PATH="$ABS_PATH" cargo run --bin car-app
    else
        echo "Invalid choice. Exiting."
        exit 1
    fi
fi
