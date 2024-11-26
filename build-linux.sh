#!/bin/bash

# Exit on error
set -e

# Configuration
PACKAGE_NAME=$(grep "^name" Cargo.toml | cut -d '"' -f 2)
VERSION=$(grep "^version" Cargo.toml | cut -d '"' -f 2)
OUTPUT_DIR="target/gh-release"
BUILD_TYPE="release"  # or "debug"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Target platforms
declare -a targets=(
    # Linux x86
    "x86_64-unknown-linux-gnu"     # 64-bit Linux (kernel 2.6.32+, glibc 2.11+)
    "x86_64-unknown-linux-musl"    # 64-bit Linux with MUSL
    "i686-unknown-linux-gnu"       # 32-bit Linux (kernel 2.6.32+, glibc 2.11+)
    "i686-unknown-linux-musl"      # 32-bit Linux with MUSL
    
    # Linux ARM
    "aarch64-unknown-linux-gnu"    # ARM64 Linux (kernel 4.1, glibc 2.17+)
    "aarch64-unknown-linux-musl"   # ARM64 Linux with MUSL
    "armv7-unknown-linux-gnueabihf" # ARMv7 Linux, hardfloat
    "armv7-unknown-linux-musleabihf" # ARMv7 Linux with MUSL, hardfloat
    
    # MIPS
    "mips64-unknown-linux-gnuabi64" # MIPS64 Linux, n64 ABI
    "mips64el-unknown-linux-gnuabi64" # MIPS64 Linux, n64 ABI, little-endian
    
    # PowerPC
    "powerpc64-unknown-linux-gnu"  # PowerPC64 Linux
    "powerpc64le-unknown-linux-gnu" # PowerPC64 Linux, little-endian
)

# Function to check if required tools are installed
check_requirements() {
    echo -e "${BLUE}Checking requirements...${NC}"
    
    # Check for Rust and Cargo
    if ! command -v cargo &> /dev/null; then
        echo "cargo not found. Please install Rust and Cargo first."
        exit 1
    fi
    
    # Check for cross-compilation tool
    if ! command -v cross &> /dev/null; then
        echo "Installing cross-compilation tool..."
        cargo install cross
    fi
    
    # Check for compression tools
    if ! command -v tar &> /dev/null; then
        echo "tar not found. Please install tar."
        exit 1
    fi
}

# Function to format binary name for GitHub release
format_binary_name() {
    local target=$1
    local binary_name="${PACKAGE_NAME}-${VERSION}-${target}"
    
    # Add .exe extension for Windows targets (if we add them later)
    if [[ $target == *"windows"* ]]; then
        binary_name="${binary_name}.exe"
    fi
    
    echo "$binary_name"
}

# Function to format archive name for GitHub release
format_archive_name() {
    local target=$1
    local archive_name="${PACKAGE_NAME}-${VERSION}-${target}.tar.gz"
    echo "$archive_name"
}

# Function to prepare build environment
prepare_build() {
    echo -e "${BLUE}Preparing build environment...${NC}"
    
    # Create output directory
    mkdir -p "$OUTPUT_DIR"
    
    # Clean previous builds
    if [ "${BUILD_TYPE}" = "release" ]; then
        cargo clean --release
    else
        cargo clean
    fi
    
    # Create checksums file
    echo "# SHA256 Checksums" > "${OUTPUT_DIR}/SHA256SUMS"
}

# Function to build for a specific target
build_target() {
    local target=$1
    echo -e "${BLUE}Building for ${target}...${NC}"
    
    # Build the project
    if [ "${BUILD_TYPE}" = "release" ]; then
        cross build --target "${target}" --release
    else
        cross build --target "${target}"
    fi
    
    # Determine binary paths
    if [ "${BUILD_TYPE}" = "release" ]; then
        local binary_path="target/${target}/release/${PACKAGE_NAME}"
    else
        local binary_path="target/${target}/debug/${PACKAGE_NAME}"
    fi
    
    if [ -f "${binary_path}" ]; then
        # Format names for GitHub release
        local release_binary_name=$(format_binary_name "${target}")
        local release_archive_name=$(format_archive_name "${target}")
        
        # Create a temporary directory for the archive
        local temp_dir=$(mktemp -d)
        cp "${binary_path}" "${temp_dir}/${release_binary_name}"
        
        # Copy license and readme if they exist
        [ -f "LICENSE" ] && cp "LICENSE" "${temp_dir}/"
        [ -f "README.md" ] && cp "README.md" "${temp_dir}/"
        
        # Create archive
        tar -czf "${OUTPUT_DIR}/${release_archive_name}" -C "${temp_dir}" .
        
        # Copy binary
        cp "${binary_path}" "${OUTPUT_DIR}/${release_binary_name}"
        
        # Generate checksums
        (cd "${OUTPUT_DIR}" && sha256sum "${release_archive_name}" "${release_binary_name}" >> "SHA256SUMS")
        
        # Cleanup
        rm -rf "${temp_dir}"
        
        echo -e "${GREEN}Successfully built for ${target}${NC}"
    else
        echo "Failed to build for ${target}"
        return 1
    fi
}

# Function to display build summary
show_summary() {
    echo -e "${BLUE}Build Summary:${NC}"
    echo "Package: ${PACKAGE_NAME}"
    echo "Version: ${VERSION}"
    echo "Build type: ${BUILD_TYPE}"
    echo "Output directory: ${OUTPUT_DIR}"
    echo -e "\nBuilt artifacts:"
    
    # List all files in the release directory with their sizes
    if [ -d "${OUTPUT_DIR}" ]; then
        find "${OUTPUT_DIR}" -type f -exec ls -lh {} \; | \
        while read -r line; do
            local file_name=$(echo "$line" | awk '{print $NF}')
            local file_size=$(echo "$line" | awk '{print $5}')
            echo -e "${GREEN}✓${NC} $(basename "$file_name") ($file_size)"
        done
    fi
}

# Function to generate release notes template
generate_release_notes() {
    local release_notes="${OUTPUT_DIR}/RELEASE_NOTES.md"
    
    cat > "${release_notes}" << EOF
# ${PACKAGE_NAME} v${VERSION}

## Release Assets

### Binary Downloads
$(for target in "${targets[@]}"; do
    echo "* \`${PACKAGE_NAME}-${VERSION}-${target}\`"
    echo "* \`${PACKAGE_NAME}-${VERSION}-${target}.tar.gz\`"
done)

### Checksums
SHA256 checksums for all release artifacts are available in the \`SHA256SUMS\` file.

## Installation

### Pre-built Binaries
1. Download the appropriate binary for your system
2. Extract it if you downloaded the \`.tar.gz\` archive
3. Copy the binary to a directory in your \$PATH

### Verify Checksums
\`\`\`bash
# Download the SHA256SUMS file
# Then verify the checksums
sha256sum -c SHA256SUMS
\`\`\`

## Changes
[Add release changes here]
EOF
    
    echo -e "${BLUE}Generated release notes template at: ${release_notes}${NC}"
}

# Main execution
main() {
    echo "Starting cross-platform build for ${PACKAGE_NAME} v${VERSION}"
    
    # Check requirements
    check_requirements
    
    # Prepare build environment
    prepare_build
    
    # Build for each target
    for target in "${targets[@]}"; do
        # Add target if not already added
        rustup target add "${target}" || true
        
        # Build for target
        build_target "${target}" || true
    done
    
    # Generate release notes template
    generate_release_notes
    
    # Show build summary
    show_summary
    
    echo -e "\n${GREEN}GitHub release artifacts are ready in: ${OUTPUT_DIR}${NC}"
}

# Run main function
main "$@"