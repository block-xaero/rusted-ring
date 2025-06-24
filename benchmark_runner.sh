#!/bin/bash

# Comprehensive Benchmark Runner for Rusted-Ring
# This script runs all benchmarks with proper system configuration

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're on a supported platform
check_platform() {
    print_status "Checking platform compatibility..."

    case "$(uname -s)" in
        Linux*)
            PLATFORM=Linux
            STACK_CMD="ulimit -s"
            ;;
        Darwin*)
            PLATFORM=Mac
            STACK_CMD="ulimit -s"
            ;;
        CYGWIN*|MINGW32*|MSYS*|MINGW*)
            PLATFORM=Windows
            print_warning "Windows detected. Stack size configuration may differ."
            STACK_CMD="echo 'Windows: Stack size set via environment or linker'"
            ;;
        *)
            print_error "Unsupported platform: $(uname -s)"
            exit 1
            ;;
    esac

    print_success "Platform: $PLATFORM"
}

# Configure system for optimal benchmarking
configure_system() {
    print_status "Configuring system for optimal benchmarking..."

    # Set stack size to 32MB (32768 KB)
    if [[ "$PLATFORM" != "Windows" ]]; then
        current_stack=$(ulimit -s)
        print_status "Current stack size: ${current_stack} KB"

        if [[ $current_stack -lt 32768 ]]; then
            print_status "Setting stack size to 32MB..."
            ulimit -s 32768
            new_stack=$(ulimit -s)
            if [[ $new_stack -ge 32768 ]]; then
                print_success "Stack size set to ${new_stack} KB"
            else
                print_warning "Failed to set stack size. Current: ${new_stack} KB"
                print_warning "You may need to run: sudo ulimit -s 32768"
            fi
        else
            print_success "Stack size is already sufficient: ${current_stack} KB"
        fi
    fi

    # Check for performance governor on Linux
    if [[ "$PLATFORM" == "Linux" ]]; then
        if [[ -f /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor ]]; then
            governor=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)
            print_status "CPU scaling governor: $governor"

            if [[ "$governor" != "performance" ]]; then
                print_warning "Consider setting CPU governor to 'performance' for consistent results:"
                print_warning "sudo cpupower frequency-set -g performance"
            else
                print_success "CPU governor is set to performance"
            fi
        fi
    fi

    # Check available memory
    if command -v free &> /dev/null; then
        print_status "Memory status:"
        free -h | head -2
    elif command -v vm_stat &> /dev/null; then
        print_status "Memory status (macOS):"
        vm_stat | head -5
    fi

    # Disable swap if possible (optional, requires sudo)
    if command -v swapon &> /dev/null && swapon --summary 2>/dev/null | grep -q "/"; then
        print_warning "Swap is enabled. For best results, consider: sudo swapoff -a"
    fi
}

# Build the project
build_project() {
    print_status "Building project in release mode..."

    if cargo build --release; then
        print_success "Build completed successfully"
    else
        print_error "Build failed"
        exit 1
    fi

    # Run a quick test to ensure everything works
    print_status "Running quick test..."
    if cargo test --release --quiet; then
        print_success "Tests passed"
    else
        print_error "Tests failed"
        exit 1
    fi
}

# Run specific benchmark suite
run_benchmark_suite() {
    local suite_name=$1
    local benchmark_file=$2
    local description=$3

    print_status "Running $suite_name benchmark suite..."
    print_status "Description: $description"

    local timestamp=$(date +"%Y%m%d_%H%M%S")
    local output_dir="benchmark_results/${suite_name}_${timestamp}"

    mkdir -p "$output_dir"

    # Run the benchmark and capture output
    if cargo bench --bench "$benchmark_file" 2>&1 | tee "$output_dir/output.log"; then
        print_success "$suite_name benchmarks completed"
        print_status "Results saved to: $output_dir"

        # Copy HTML reports if they exist
        if [[ -d "target/criterion" ]]; then
            cp -r target/criterion "$output_dir/" 2>/dev/null || true
            print_status "HTML reports copied to: $output_dir/criterion/"
        fi
    else
        print_error "$suite_name benchmarks failed"
        return 1
    fi
}

# Generate summary report
generate_summary() {
    print_status "Generating benchmark summary..."

    local summary_file="benchmark_summary_$(date +"%Y%m%d_%H%M%S").md"

    cat > "$summary_file" << EOF
# Rusted-Ring Benchmark Results Summary

Generated on: $(date)
Platform: $PLATFORM $(uname -r)
Rust Version: $(rustc --version)
Stack Size: $(ulimit -s 2>/dev/null || echo "Unknown") KB

## System Configuration

EOF

    # Add system info
    if command -v lscpu &> /dev/null; then
        echo "### CPU Information" >> "$summary_file"
        echo '```' >> "$summary_file"
        lscpu | head -10 >> "$summary_file"
        echo '```' >> "$summary_file"
        echo "" >> "$summary_file"
    fi

    if command -v free &> /dev/null; then
        echo "### Memory Information" >> "$summary_file"
        echo '```' >> "$summary_file"
        free -h >> "$summary_file"
        echo '```' >> "$summary_file"
        echo "" >> "$summary_file"
    fi

    echo "## Benchmark Results" >> "$summary_file"
    echo "" >> "$summary_file"
    echo "Detailed results are available in the individual benchmark report directories." >> "$summary_file"
    echo "" >> "$summary_file"
    echo "### Quick Access Links" >> "$summary_file"
    echo "" >> "$summary_file"

    # Find all benchmark result directories
    if [[ -d "benchmark_results" ]]; then
        for result_dir in benchmark_results/*/; do
            if [[ -d "$result_dir" ]]; then
                dir_name=$(basename "$result_dir")
                echo "- [$dir_name]($result_dir)" >> "$summary_file"
            fi
        done
    fi

    print_success "Summary generated: $summary_file"
}

# Main execution function
main() {
    echo "============================================================"
    echo "          Rusted-Ring Comprehensive Benchmark Suite        "
    echo "============================================================"
    echo ""

    # Parse command line arguments
    QUICK_MODE=false
    COMPARISON_ONLY=false
    MEMORY_ONLY=false

    while [[ $# -gt 0 ]]; do
        case $1 in
            --quick)
                QUICK_MODE=true
                shift
                ;;
            --comparison-only)
                COMPARISON_ONLY=true
                shift
                ;;
            --memory-only)
                MEMORY_ONLY=true
                shift
                ;;
            --help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --quick           Run only essential benchmarks"
                echo "  --comparison-only Run only comparison benchmarks"
                echo "  --memory-only     Run only memory-related benchmarks"
                echo "  --help           Show this help message"
                echo ""
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done

    # System checks and configuration
    check_platform
    configure_system
    build_project

    # Create results directory
    mkdir -p benchmark_results

    echo ""
    print_status "Starting benchmark execution..."
    echo ""

    # Run benchmark suites based on flags
    if [[ "$COMPARISON_ONLY" == "true" ]]; then
        run_benchmark_suite "comparison" "comparison_benchmarks" \
            "Performance comparison against standard allocation methods"
    elif [[ "$MEMORY_ONLY" == "true" ]]; then
        run_benchmark_suite "memory_profiling" "memory_profiling" \
            "Memory usage, cache performance, and stack allocation analysis"
    elif [[ "$QUICK_MODE" == "true" ]]; then
        print_status "Running in quick mode (essential benchmarks only)..."

        # Run only core allocation benchmarks
        cargo bench --bench comprehensive_benchmarks allocation_benches
        print_success "Quick benchmark run completed"
    else
        # Run all benchmark suites
        run_benchmark_suite "comprehensive" "comprehensive_benchmarks" \
            "Core allocation, concurrency, and stress tests"

        run_benchmark_suite "memory_profiling" "memory_profiling" \
            "Memory usage, cache performance, and stack allocation analysis"

        run_benchmark_suite "comparison" "comparison_benchmarks" \
            "Performance comparison against standard allocation methods"
    fi

    # Generate summary report
    generate_summary

    echo ""
    echo "============================================================"
    print_success "All benchmarks completed successfully!"
    echo "============================================================"
    echo ""
    print_status "Next steps:"
    echo "1. Review the generated HTML reports in target/criterion/"
    echo "2. Check benchmark_results/ for detailed logs"
    echo "3. Open the summary markdown file for an overview"
    echo ""
    print_status "To compare against a baseline:"
    echo "cargo bench -- --save-baseline my_baseline"
    echo "cargo bench -- --baseline my_baseline"
    echo ""
}

# Trap to ensure cleanup on exit
cleanup() {
    print_status "Cleaning up..."
    # Add any cleanup tasks here
}

trap cleanup EXIT

# Run main function
main "$@"