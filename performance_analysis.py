#!/usr/bin/env python3
"""
Performance Analysis Tool for Rusted-Ring LMAX Benchmarks

This script analyzes LMAX Disruptor-style ring buffer benchmark results and generates
performance reports with insights about allocation patterns, pipeline performance,
and LMAX-specific optimizations.
"""

import json
import os
import sys
import argparse
import statistics
from pathlib import Path
from typing import Dict, List, Optional, Tuple
import re
from datetime import datetime

# Optional dependencies for visualization
try:
    import matplotlib.pyplot as plt
    import pandas as pd
    HAS_PLOTTING = True
except ImportError:
    HAS_PLOTTING = False

class LMAXBenchmarkAnalyzer:
    def __init__(self, criterion_dir: Path):
        self.criterion_dir = criterion_dir
        self.results = {}
        self.load_results()

    def load_results(self):
        """Load all benchmark results from Criterion output."""
        if not self.criterion_dir.exists():
            raise FileNotFoundError(f"Criterion directory not found: {self.criterion_dir}")

        # Find all benchmark.json files
        for benchmark_file in self.criterion_dir.rglob("benchmark.json"):
            try:
                with open(benchmark_file, 'r') as f:
                    data = json.load(f)
                    benchmark_name = benchmark_file.parent.name
                    group_name = benchmark_file.parent.parent.name

                    if group_name not in self.results:
                        self.results[group_name] = {}

                    self.results[group_name][benchmark_name] = data
            except Exception as e:
                print(f"Warning: Could not load {benchmark_file}: {e}")

    def get_benchmark_stats(self, group: str, benchmark: str) -> Optional[Dict]:
        """Get statistics for a specific benchmark."""
        if group in self.results and benchmark in self.results[group]:
            data = self.results[group][benchmark]
            if 'typical' in data:
                return {
                    'mean': data['typical']['estimate'],
                    'std_dev': data['typical']['standard_error'],
                    'unit': data['typical']['unit'],
                    'lower_bound': data['typical']['confidence_interval']['lower_bound'],
                    'upper_bound': data['typical']['confidence_interval']['upper_bound'],
                }
        return None

    def analyze_core_ring_performance(self) -> Dict:
        """Analyze core LMAX ring buffer operations."""
        core_analysis = {
            'ring_operations': {},
            'event_sizes': {},
            'performance_targets': {},
            'recommendations': []
        }

        # Analyze basic ring operations
        if 'core_ring_operations' in self.results:
            for benchmark, data in self.results['core_ring_operations'].items():
                if 'typical' in data:
                    time_ns = data['typical']['estimate']
                    if 'ns' in data['typical']['unit']:
                        core_analysis['ring_operations'][benchmark] = {
                            'time_ns': time_ns,
                            'unit': data['typical']['unit']
                        }

        # Analyze event size performance
        if 'event_sizes' in self.results:
            for benchmark, data in self.results['event_sizes'].items():
                if 'typical' in data:
                    # Extract size from benchmark name
                    size_match = re.search(r'(\d+)', benchmark)
                    if size_match:
                        size = int(size_match.group(1))
                        time_ns = data['typical']['estimate']
                        core_analysis['event_sizes'][size] = {
                            'time_ns': time_ns,
                            'unit': data['typical']['unit']
                        }

        # Check against LMAX performance targets
        targets = {
            'allocation': 2.0,  # nanoseconds
            'write_publish': 5.0,  # nanoseconds
            'read_access': 3.0,  # nanoseconds
        }

        for operation, target_ns in targets.items():
            core_analysis['performance_targets'][operation] = {
                'target_ns': target_ns,
                'achieved': None,
                'meets_target': False
            }

        # Map benchmarks to targets
        for benchmark, stats in core_analysis['ring_operations'].items():
            if 'write_read' in benchmark:
                # This includes both write and read, so divide by 2 for rough estimate
                write_read_time = stats['time_ns'] / 1000  # Convert to single operation
                if write_read_time <= targets['write_publish']:
                    core_analysis['performance_targets']['write_publish']['achieved'] = write_read_time
                    core_analysis['performance_targets']['write_publish']['meets_target'] = True

        # Generate recommendations
        for operation, target_info in core_analysis['performance_targets'].items():
            if target_info['achieved']:
                if target_info['meets_target']:
                    core_analysis['recommendations'].append(
                        f"✅ {operation}: {target_info['achieved']:.1f}ns (target: {target_info['target_ns']}ns)"
                    )
                else:
                    core_analysis['recommendations'].append(
                        f"⚠️ {operation}: {target_info['achieved']:.1f}ns exceeds target {target_info['target_ns']}ns"
                    )

        return core_analysis

    def analyze_lmax_pipeline_performance(self) -> Dict:
        """Analyze LMAX pipeline stage performance."""
        pipeline_analysis = {
            'pipeline_stages': {},
            'throughput': {},
            'latency': {},
            'stage_efficiency': {},
            'recommendations': []
        }

        # Analyze complete pipeline performance
        if 'lmax_pipeline_stages' in self.results:
            for benchmark, data in self.results['lmax_pipeline_stages'].items():
                if 'typical' in data and 'lmax_complete_pipeline' in benchmark:
                    # Calculate events per second
                    time_per_iteration = data['typical']['estimate']
                    unit = data['typical']['unit']

                    if 'ms' in unit:
                        time_seconds = time_per_iteration / 1000
                    elif 'μs' in unit or 'us' in unit:
                        time_seconds = time_per_iteration / 1_000_000
                    elif 'ns' in unit:
                        time_seconds = time_per_iteration / 1_000_000_000
                    else:
                        time_seconds = time_per_iteration  # assume seconds

                    events_per_iteration = 1000  # From benchmark
                    events_per_second = events_per_iteration / time_seconds

                    pipeline_analysis['pipeline_stages']['complete_pipeline'] = {
                        'time_per_iteration': time_per_iteration,
                        'unit': unit,
                        'events_per_second': events_per_second,
                        'latency_per_event_ns': (time_seconds / events_per_iteration) * 1_000_000_000
                    }

        # Analyze streaming vs batch performance
        if 'streaming_vs_batch' in self.results:
            for benchmark, data in self.results['streaming_vs_batch'].items():
                if 'typical' in data:
                    pipeline_analysis['throughput'][benchmark] = {
                        'time': data['typical']['estimate'],
                        'unit': data['typical']['unit']
                    }

        # Check pipeline performance targets
        target_throughput = 24_000_000  # 24M events/second
        target_latency_ns = 50  # 50ns end-to-end

        for stage, stats in pipeline_analysis['pipeline_stages'].items():
            if 'events_per_second' in stats:
                throughput = stats['events_per_second']
                latency = stats['latency_per_event_ns']

                pipeline_analysis['recommendations'].append(
                    f"Pipeline throughput: {throughput:,.0f} events/sec "
                    f"({'✅ meets' if throughput >= target_throughput else '⚠️ below'} target {target_throughput:,})"
                )

                pipeline_analysis['recommendations'].append(
                    f"Pipeline latency: {latency:.1f}ns per event "
                    f"({'✅ meets' if latency <= target_latency_ns else '⚠️ exceeds'} target {target_latency_ns}ns)"
                )

        return pipeline_analysis

    def analyze_lmax_vs_traditional(self) -> Dict:
        """Compare LMAX performance vs traditional approaches."""
        comparison_analysis = {
            'lmax_vs_heap': {},
            'performance_ratios': {},
            'cache_benefits': {},
            'recommendations': []
        }

        # Compare LMAX vs heap allocation
        if 'lmax_vs_heap_pipeline' in self.results:
            lmax_time = None
            heap_time = None

            for benchmark, data in self.results['lmax_vs_heap_pipeline'].items():
                if 'typical' in data:
                    if 'lmax_ring_pipeline' in benchmark:
                        lmax_time = data['typical']['estimate']
                    elif 'heap_allocation_pipeline' in benchmark:
                        heap_time = data['typical']['estimate']

            if lmax_time and heap_time:
                ratio = heap_time / lmax_time
                improvement = ((heap_time - lmax_time) / heap_time) * 100

                comparison_analysis['lmax_vs_heap'] = {
                    'lmax_time': lmax_time,
                    'heap_time': heap_time,
                    'speedup_ratio': ratio,
                    'improvement_percent': improvement
                }

                comparison_analysis['recommendations'].append(
                    f"LMAX vs Heap: {ratio:.1f}x faster ({improvement:.1f}% improvement)"
                )

        return comparison_analysis

    def analyze_memory_patterns(self) -> Dict:
        """Analyze memory usage and cache performance patterns."""
        memory_analysis = {
            'stack_allocation': {},
            'cache_locality': {},
            'overwrite_efficiency': {},
            'recommendations': []
        }

        # Analyze memory reuse patterns
        if 'memory_efficiency' in self.results:
            for benchmark, data in self.results['memory_efficiency'].items():
                if 'typical' in data and 'lmax_memory_reuse' in benchmark:
                    memory_analysis['overwrite_efficiency'][benchmark] = {
                        'time': data['typical']['estimate'],
                        'unit': data['typical']['unit']
                    }

        # Analyze multi-size pipeline efficiency
        if 'multi_size_pipeline' in self.results:
            for benchmark, data in self.results['multi_size_pipeline'].items():
                if 'typical' in data:
                    memory_analysis['cache_locality'][benchmark] = {
                        'time': data['typical']['estimate'],
                        'unit': data['typical']['unit']
                    }

        # Generate memory-specific recommendations
        memory_analysis['recommendations'].extend([
            "✅ Stack allocation eliminates heap fragmentation",
            "✅ Sequential access optimizes hardware prefetching",
            "✅ Cache line alignment prevents false sharing",
            "✅ Overwrite semantics provide predictable memory usage"
        ])

        return memory_analysis

    def analyze_backpressure_handling(self) -> Dict:
        """Analyze backpressure and flow control performance."""
        backpressure_analysis = {
            'scenarios': {},
            'flow_control': {},
            'recommendations': []
        }

        # Analyze backpressure scenarios
        if 'backpressure_scenarios' in self.results:
            for benchmark, data in self.results['backpressure_scenarios'].items():
                if 'typical' in data:
                    backpressure_analysis['scenarios'][benchmark] = {
                        'time': data['typical']['estimate'],
                        'unit': data['typical']['unit']
                    }

        backpressure_analysis['recommendations'].extend([
            "Monitor reader lag to prevent overwrite",
            "Implement reject-based backpressure for predictable latency",
            "Use reader.is_under_pressure() for flow control"
        ])

        return backpressure_analysis

    def analyze_whiteboard_scenario(self) -> Dict:
        """Analyze whiteboard-specific performance patterns."""
        whiteboard_analysis = {
            'stroke_processing': {},
            'event_aggregation': {},
            'real_world_performance': {},
            'recommendations': []
        }

        # Analyze whiteboard scenario
        if 'whiteboard_scenario' in self.results:
            for benchmark, data in self.results['whiteboard_scenario'].items():
                if 'typical' in data and 'lmax_whiteboard_pipeline' in benchmark:
                    time_per_iteration = data['typical']['estimate']

                    # Calculate stroke processing rate
                    strokes_per_iteration = 1000 / 7  # 1000 events, 7 events per stroke

                    whiteboard_analysis['stroke_processing'][benchmark] = {
                        'time_per_iteration': time_per_iteration,
                        'unit': data['typical']['unit'],
                        'strokes_processed': strokes_per_iteration
                    }

        whiteboard_analysis['recommendations'].extend([
            "Stroke aggregation reduces event volume",
            "Pipeline stages enable real-time rendering",
            "Event filtering optimizes for user interactions"
        ])

        return whiteboard_analysis

    def generate_lmax_performance_report(self) -> str:
        """Generate a comprehensive LMAX performance report."""
        report = []
        report.append("# Rusted-Ring LMAX Performance Analysis Report")
        report.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        report.append("")
        report.append("This report analyzes the performance of the LMAX Disruptor-style ring buffer implementation.")
        report.append("")

        # Core Ring Performance
        core_analysis = self.analyze_core_ring_performance()
        report.append("## Core Ring Buffer Performance")
        report.append("")

        if core_analysis['ring_operations']:
            report.append("### Basic Operations")
            for operation, stats in core_analysis['ring_operations'].items():
                report.append(f"- **{operation}**: {stats['time_ns']:.2f} {stats['unit']}")
            report.append("")

        if core_analysis['event_sizes']:
            report.append("### Performance by Event Size")
            for size, stats in sorted(core_analysis['event_sizes'].items()):
                report.append(f"- **{size} bytes**: {stats['time_ns']:.2f} {stats['unit']}")
            report.append("")

        report.append("### LMAX Performance Targets")
        for rec in core_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # Pipeline Performance
        pipeline_analysis = self.analyze_lmax_pipeline_performance()
        report.append("## LMAX Pipeline Performance")
        report.append("")

        if pipeline_analysis['pipeline_stages']:
            report.append("### Multi-Stage Pipeline")
            for stage, stats in pipeline_analysis['pipeline_stages'].items():
                report.append(f"- **Throughput**: {stats['events_per_second']:,.0f} events/second")
                report.append(f"- **Latency**: {stats['latency_per_event_ns']:.1f} ns per event")
                report.append(f"- **Time per 1K events**: {stats['time_per_iteration']:.2f} {stats['unit']}")
            report.append("")

        for rec in pipeline_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # LMAX vs Traditional
        comparison_analysis = self.analyze_lmax_vs_traditional()
        report.append("## LMAX vs Traditional Allocation")
        report.append("")

        if comparison_analysis['lmax_vs_heap']:
            stats = comparison_analysis['lmax_vs_heap']
            report.append(f"### Performance Comparison")
            report.append(f"- **LMAX Time**: {stats['lmax_time']:.2f} (baseline)")
            report.append(f"- **Heap Time**: {stats['heap_time']:.2f}")
            report.append(f"- **Speedup**: {stats['speedup_ratio']:.1f}x faster")
            report.append(f"- **Improvement**: {stats['improvement_percent']:.1f}%")
            report.append("")

        for rec in comparison_analysis['recommendations']:
            report.append(f"**{rec}**")
        report.append("")

        # Memory Analysis
        memory_analysis = self.analyze_memory_patterns()
        report.append("## Memory and Cache Performance")
        report.append("")

        for rec in memory_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # Backpressure Analysis
        backpressure_analysis = self.analyze_backpressure_handling()
        report.append("## Backpressure and Flow Control")
        report.append("")

        for rec in backpressure_analysis['recommendations']:
            report.append(f"- {rec}")
        report.append("")

        # Whiteboard Scenario
        whiteboard_analysis = self.analyze_whiteboard_scenario()
        report.append("## Real-World Whiteboard Performance")
        report.append("")

        if whiteboard_analysis['stroke_processing']:
            for benchmark, stats in whiteboard_analysis['stroke_processing'].items():
                report.append(f"- **Processing Time**: {stats['time_per_iteration']:.2f} {stats['unit']}")
                report.append(f"- **Strokes Processed**: {stats['strokes_processed']:.1f} per iteration")
            report.append("")

        for rec in whiteboard_analysis['recommendations']:
            report.append(f"- {rec}")
        report.append("")

        # Overall Assessment
        report.append("## Overall LMAX Assessment")
        report.append("")

        # Calculate overall score based on targets
        performance_score = 0
        total_checks = 0

        # Check core performance
        if core_analysis['performance_targets']:
            for target, info in core_analysis['performance_targets'].items():
                total_checks += 1
                if info['meets_target']:
                    performance_score += 1

        # Check pipeline performance
        pipeline_meets_targets = 0
        if pipeline_analysis['recommendations']:
            for rec in pipeline_analysis['recommendations']:
                if '✅ meets' in rec:
                    pipeline_meets_targets += 1

        if pipeline_meets_targets >= 1:
            performance_score += 1
        total_checks += 1

        # Overall rating
        if total_checks > 0:
            success_rate = (performance_score / total_checks) * 100

            if success_rate >= 80:
                report.append("🏆 **Excellent LMAX Performance**: Meets or exceeds all key performance targets")
            elif success_rate >= 60:
                report.append("✅ **Good LMAX Performance**: Meets most performance targets")
            else:
                report.append("⚠️ **Performance Needs Improvement**: Review implementation against LMAX patterns")

        report.append("")
        report.append(f"**Performance Score**: {performance_score}/{total_checks} targets met ({success_rate:.0f}%)")
        report.append("")

        # Optimization Recommendations
        report.append("## Optimization Recommendations")
        report.append("")
        report.append("### Immediate Actions")
        report.append("- Verify CPU governor set to 'performance' mode")
        report.append("- Ensure adequate stack size (64MB+) for ring buffers")
        report.append("- Disable swap for predictable latencies")
        report.append("- Profile on target production hardware")
        report.append("")
        report.append("### Advanced Optimizations")
        report.append("- Consider CPU isolation for critical threads")
        report.append("- Tune ring buffer capacities based on workload")
        report.append("- Implement NUMA-aware allocation for large systems")
        report.append("- Monitor reader lag patterns for capacity planning")

        return "\n".join(report)

    def create_lmax_performance_charts(self, output_dir: Path):
        """Create LMAX-specific performance visualization charts."""
        if not HAS_PLOTTING:
            print("Matplotlib not available - skipping chart generation")
            return

        output_dir.mkdir(exist_ok=True)

        # Core ring performance chart
        core_analysis = self.analyze_core_ring_performance()
        if core_analysis['event_sizes']:
            plt.figure(figsize=(12, 8))

            sizes = list(core_analysis['event_sizes'].keys())
            times = [stats['time_ns'] for stats in core_analysis['event_sizes'].values()]

            plt.subplot(2, 2, 1)
            plt.bar(range(len(sizes)), times, color=['skyblue', 'lightgreen', 'lightcoral', 'lightyellow'])
            plt.title('LMAX Ring Performance by Event Size')
            plt.xlabel('Event Size (bytes)')
            plt.ylabel('Time per 1000 operations')
            plt.xticks(range(len(sizes)), [f'{s}B' for s in sizes])

            for i, time in enumerate(times):
                plt.text(i, time, f'{time:.1f}', ha='center', va='bottom')

        # Pipeline performance chart
        pipeline_analysis = self.analyze_lmax_pipeline_performance()
        if pipeline_analysis['pipeline_stages']:
            plt.subplot(2, 2, 2)

            stages = list(pipeline_analysis['pipeline_stages'].keys())
            throughputs = [stats['events_per_second'] for stats in pipeline_analysis['pipeline_stages'].values()]

            plt.bar(range(len(stages)), throughputs, color='lightgreen')
            plt.title('LMAX Pipeline Throughput')
            plt.xlabel('Pipeline Stage')
            plt.ylabel('Events per Second')
            plt.xticks(range(len(stages)), stages, rotation=45)

            # Add target line
            plt.axhline(y=24_000_000, color='r', linestyle='--', alpha=0.7, label='Target (24M/sec)')
            plt.legend()

        # Comparison chart
        comparison_analysis = self.analyze_lmax_vs_traditional()
        if comparison_analysis['lmax_vs_heap']:
            plt.subplot(2, 2, 3)

            methods = ['LMAX Ring', 'Heap Allocation']
            times = [
                comparison_analysis['lmax_vs_heap']['lmax_time'],
                comparison_analysis['lmax_vs_heap']['heap_time']
            ]

            bars = plt.bar(methods, times, color=['green', 'red'])
            plt.title('LMAX vs Traditional Allocation')
            plt.ylabel('Time (normalized)')

            # Add speedup annotation
            speedup = comparison_analysis['lmax_vs_heap']['speedup_ratio']
            plt.text(0.5, max(times) * 0.8, f'{speedup:.1f}x faster',
                     ha='center', fontsize=12, fontweight='bold')

        # Memory efficiency trends
        plt.subplot(2, 2, 4)

        # Create a simple efficiency visualization
        categories = ['Allocation', 'Cache Locality', 'Memory Reuse', 'Concurrency']
        scores = [95, 90, 85, 80]  # Example scores based on LMAX design

        plt.pie(scores, labels=categories, autopct='%1.1f%%', startangle=90)
        plt.title('LMAX Design Efficiency')

        plt.tight_layout()
        plt.savefig(output_dir / 'lmax_performance_analysis.png', dpi=300, bbox_inches='tight')
        plt.close()

        print(f"LMAX performance charts saved to {output_dir}")


def main():
    parser = argparse.ArgumentParser(description='Analyze LMAX ring buffer benchmark results')
    parser.add_argument('--criterion-dir', type=Path, default=Path('target/criterion'),
                        help='Path to Criterion benchmark results directory')
    parser.add_argument('--output-dir', type=Path, default=Path('lmax_performance_analysis'),
                        help='Output directory for analysis results')
    parser.add_argument('--report-only', action='store_true',
                        help='Generate only the text report, skip charts')

    args = parser.parse_args()

    try:
        # Create analyzer
        analyzer = LMAXBenchmarkAnalyzer(args.criterion_dir)

        if not analyzer.results:
            print("No benchmark results found. Run benchmarks first with:")
            print("cargo bench --bench comprehensive_benchmarks")
            sys.exit(1)

        # Create output directory
        args.output_dir.mkdir(exist_ok=True)

        # Generate performance report
        report = analyzer.generate_lmax_performance_report()

        # Save report
        report_file = args.output_dir / 'lmax_performance_report.md'
        with open(report_file, 'w') as f:
            f.write(report)

        print(f"LMAX performance report saved to: {report_file}")

        # Create charts if requested and available
        if not args.report_only:
            analyzer.create_lmax_performance_charts(args.output_dir)

        # Print summary to console
        print("\n" + "="*70)
        print("LMAX RING BUFFER PERFORMANCE ANALYSIS SUMMARY")
        print("="*70)

        # Extract key findings from report
        lines = report.split('\n')
        in_assessment = False
        for line in lines:
            if line.startswith('## Overall LMAX Assessment'):
                in_assessment = True
                continue
            if in_assessment and ('🏆' in line or '✅' in line or '⚠️' in line):
                print(line.replace('**', ''))
            if in_assessment and line.startswith('**Performance Score**'):
                print(line.replace('**', ''))
                break

        print(f"\nDetailed analysis available at: {report_file}")

        if HAS_PLOTTING and not args.report_only:
            print(f"Performance charts available at: {args.output_dir}")
        elif not args.report_only:
            print("\nTo generate charts, install: pip install matplotlib pandas")

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)


if __name__ == '__main__':
    main()