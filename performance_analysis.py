#!/usr/bin/env python3
"""
Performance Analysis Tool for Rusted-Ring Benchmarks

This script analyzes benchmark results and generates performance reports
with insights about allocation patterns, memory usage, and optimization opportunities.
"""

import json
import os
import sys
import argparse
import statistics
from pathlib import Path
from typing import Dict, List, Optional, Tuple
import matplotlib.pyplot as plt
import pandas as pd
from datetime import datetime

class BenchmarkAnalyzer:
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

    def compare_benchmarks(self, comparisons: List[Tuple[str, str, str]]) -> Dict:
        """Compare multiple benchmarks and return performance ratios."""
        results = {}

        for group, bench1, bench2 in comparisons:
            stats1 = self.get_benchmark_stats(group, bench1)
            stats2 = self.get_benchmark_stats(group, bench2)

            if stats1 and stats2:
                ratio = stats2['mean'] / stats1['mean']
                improvement = ((stats1['mean'] - stats2['mean']) / stats1['mean']) * 100

                results[f"{bench1}_vs_{bench2}"] = {
                    'ratio': ratio,
                    'improvement_percent': improvement,
                    'faster': bench1 if stats1['mean'] < stats2['mean'] else bench2,
                    'bench1_time': stats1['mean'],
                    'bench2_time': stats2['mean'],
                    'unit': stats1['unit']
                }

        return results

    def analyze_allocation_performance(self) -> Dict:
        """Analyze allocation performance across different pool sizes."""
        allocation_analysis = {
            'pool_performance': {},
            'size_efficiency': {},
            'recommendations': []
        }

        # Analyze single allocation performance
        if 'single_allocation' in self.results:
            pool_times = {}
            for benchmark, data in self.results['single_allocation'].items():
                if 'pool' in benchmark and 'typical' in data:
                    pool_size = benchmark.replace('_pool', '').upper()
                    pool_times[pool_size] = data['typical']['estimate']

            allocation_analysis['pool_performance'] = pool_times

            # Calculate efficiency ratios
            if 'XS' in pool_times:
                base_time = pool_times['XS']
                for pool, time in pool_times.items():
                    if pool != 'XS':
                        overhead = ((time - base_time) / base_time) * 100
                        allocation_analysis['size_efficiency'][pool] = {
                            'overhead_percent': overhead,
                            'absolute_time': time
                        }

        # Generate recommendations
        if allocation_analysis['pool_performance']:
            fastest_pool = min(allocation_analysis['pool_performance'].items(), key=lambda x: x[1])
            slowest_pool = max(allocation_analysis['pool_performance'].items(), key=lambda x: x[1])

            allocation_analysis['recommendations'].extend([
                f"Fastest allocation: {fastest_pool[0]} pool ({fastest_pool[1]:.2f} ns)",
                f"Slowest allocation: {slowest_pool[0]} pool ({slowest_pool[1]:.2f} ns)",
            ])

            # Check if there's significant overhead
            if slowest_pool[1] / fastest_pool[1] > 2.0:
                allocation_analysis['recommendations'].append(
                    f"WARNING: {slowest_pool[0]} pool is {slowest_pool[1]/fastest_pool[1]:.1f}x slower than {fastest_pool[0]}"
                )

        return allocation_analysis

    def analyze_memory_efficiency(self) -> Dict:
        """Analyze memory usage patterns and efficiency."""
        memory_analysis = {
            'stack_usage': {},
            'heap_usage': {},
            'cache_performance': {},
            'recommendations': []
        }

        # Analyze cache performance if available
        if 'cache_performance' in self.results:
            cache_benchmarks = ['sequential_allocation', 'sequential_data_access']
            for benchmark in cache_benchmarks:
                if benchmark in self.results['cache_performance']:
                    data = self.results['cache_performance'][benchmark]
                    if 'typical' in data:
                        memory_analysis['cache_performance'][benchmark] = {
                            'time': data['typical']['estimate'],
                            'unit': data['typical']['unit']
                        }

        # Analyze alignment performance
        if 'alignment_performance' in self.results:
            alignment_data = self.results['alignment_performance']
            if 'cache_line_alignment_check' in alignment_data:
                data = alignment_data['cache_line_alignment_check']
                if 'typical' in data:
                    memory_analysis['cache_performance']['alignment_check'] = {
                        'time': data['typical']['estimate'],
                        'unit': data['typical']['unit']
                    }

        # Generate memory recommendations
        memory_analysis['recommendations'].extend([
            "Stack allocation should show near-zero heap usage",
            "Cache-aligned access should be faster than unaligned",
            "Pool allocation should show better locality than heap allocation"
        ])

        return memory_analysis

    def analyze_concurrent_performance(self) -> Dict:
        """Analyze concurrent allocation and reference counting performance."""
        concurrent_analysis = {
            'scaling': {},
            'contention': {},
            'recommendations': []
        }

        # Analyze concurrent allocation scaling
        if 'concurrent_allocation' in self.results:
            thread_counts = []
            times = []

            for benchmark, data in self.results['concurrent_allocation'].items():
                if 'concurrent_xs' in benchmark and 'typical' in data:
                    # Extract thread count from benchmark name
                    parts = benchmark.split('/')
                    if len(parts) > 1:
                        try:
                            thread_count = int(parts[1])
                            thread_counts.append(thread_count)
                            times.append(data['typical']['estimate'])
                        except ValueError:
                            pass

            if thread_counts and times:
                # Calculate scaling efficiency
                baseline_time = times[0] if times else 0
                for i, (threads, time) in enumerate(zip(thread_counts, times)):
                    if baseline_time > 0:
                        ideal_time = baseline_time / threads
                        efficiency = (ideal_time / time) * 100
                        concurrent_analysis['scaling'][threads] = {
                            'actual_time': time,
                            'ideal_time': ideal_time,
                            'efficiency_percent': efficiency
                        }

        # Generate concurrent recommendations
        if concurrent_analysis['scaling']:
            avg_efficiency = statistics.mean([s['efficiency_percent'] for s in concurrent_analysis['scaling'].values()])
            concurrent_analysis['recommendations'].append(f"Average scaling efficiency: {avg_efficiency:.1f}%")

            if avg_efficiency > 80:
                concurrent_analysis['recommendations'].append("Excellent concurrent scaling")
            elif avg_efficiency > 60:
                concurrent_analysis['recommendations'].append("Good concurrent scaling")
            else:
                concurrent_analysis['recommendations'].append("Poor concurrent scaling - check for contention")

        return concurrent_analysis

    def compare_with_alternatives(self) -> Dict:
        """Compare rusted-ring performance with alternative approaches."""
        comparison_analysis = {
            'allocation_comparison': {},
            'clone_comparison': {},
            'access_comparison': {},
            'recommendations': []
        }

        # Define comparison pairs
        comparisons = [
            ('allocation_comparison', 'rusted_ring_allocation', 'vec_allocation'),
            ('allocation_comparison', 'rusted_ring_allocation', 'box_allocation'),
            ('allocation_comparison', 'rusted_ring_allocation', 'arc_allocation'),
            ('clone_comparison', 'rusted_ring_clone', 'vec_clone'),
            ('clone_comparison', 'rusted_ring_clone', 'arc_clone'),
            ('data_access_comparison', 'rusted_ring_access', 'vec_access'),
        ]

        comparison_results = self.compare_benchmarks(comparisons)

        # Categorize results
        for comp_name, result in comparison_results.items():
            if 'allocation' in comp_name:
                comparison_analysis['allocation_comparison'][comp_name] = result
            elif 'clone' in comp_name:
                comparison_analysis['clone_comparison'][comp_name] = result
            elif 'access' in comp_name:
                comparison_analysis['access_comparison'][comp_name] = result

        # Generate recommendations based on comparisons
        for category, comparisons in comparison_analysis.items():
            if category == 'recommendations':
                continue

            rusted_ring_wins = 0
            total_comparisons = len(comparisons)

            for comp_name, result in comparisons.items():
                if 'rusted_ring' in result['faster']:
                    rusted_ring_wins += 1

            if total_comparisons > 0:
                win_rate = (rusted_ring_wins / total_comparisons) * 100
                comparison_analysis['recommendations'].append(
                    f"{category}: rusted-ring wins {rusted_ring_wins}/{total_comparisons} ({win_rate:.0f}%)"
                )

        return comparison_analysis

    def generate_performance_report(self) -> str:
        """Generate a comprehensive performance report."""
        report = []
        report.append("# Rusted-Ring Performance Analysis Report")
        report.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        report.append("")

        # Allocation Performance Analysis
        allocation_analysis = self.analyze_allocation_performance()
        report.append("## Allocation Performance")
        report.append("")

        if allocation_analysis['pool_performance']:
            report.append("### Pool Performance (nanoseconds per allocation)")
            for pool, time in sorted(allocation_analysis['pool_performance'].items()):
                report.append(f"- {pool}: {time:.2f} ns")
            report.append("")

        if allocation_analysis['size_efficiency']:
            report.append("### Size Efficiency (overhead vs XS pool)")
            for pool, stats in allocation_analysis['size_efficiency'].items():
                report.append(f"- {pool}: {stats['overhead_percent']:+.1f}% overhead")
            report.append("")

        for rec in allocation_analysis['recommendations']:
            report.append(f"**{rec}**")
        report.append("")

        # Memory Efficiency Analysis
        memory_analysis = self.analyze_memory_efficiency()
        report.append("## Memory Efficiency")
        report.append("")

        if memory_analysis['cache_performance']:
            report.append("### Cache Performance")
            for test, stats in memory_analysis['cache_performance'].items():
                report.append(f"- {test}: {stats['time']:.2f} {stats['unit']}")
            report.append("")

        for rec in memory_analysis['recommendations']:
            report.append(f"- {rec}")
        report.append("")

        # Concurrent Performance Analysis
        concurrent_analysis = self.analyze_concurrent_performance()
        report.append("## Concurrent Performance")
        report.append("")

        if concurrent_analysis['scaling']:
            report.append("### Scaling Efficiency")
            for threads, stats in sorted(concurrent_analysis['scaling'].items()):
                report.append(f"- {threads} threads: {stats['efficiency_percent']:.1f}% efficiency")
            report.append("")

        for rec in concurrent_analysis['recommendations']:
            report.append(f"**{rec}**")
        report.append("")

        # Comparison Analysis
        comparison_analysis = self.compare_with_alternatives()
        report.append("## Comparison with Alternatives")
        report.append("")

        for category, comparisons in comparison_analysis.items():
            if category == 'recommendations':
                continue

            report.append(f"### {category.replace('_', ' ').title()}")
            for comp_name, result in comparisons.items():
                improvement = result['improvement_percent']
                faster = result['faster']
                report.append(f"- {comp_name}: {improvement:+.1f}% ({'rusted-ring wins' if 'rusted_ring' in faster else 'alternative wins'})")
            report.append("")

        for rec in comparison_analysis['recommendations']:
            report.append(f"**{rec}**")
        report.append("")

        # Overall Recommendations
        report.append("## Overall Recommendations")
        report.append("")

        # Calculate overall performance score
        total_wins = sum(1 for analysis in [allocation_analysis, memory_analysis, concurrent_analysis, comparison_analysis]
                         for rec in analysis['recommendations'] if 'wins' in rec or 'Excellent' in rec or 'Good' in rec)

        if total_wins >= 3:
            report.append("✅ **Excellent Performance**: Rusted-ring shows strong performance across all categories")
        elif total_wins >= 2:
            report.append("✅ **Good Performance**: Rusted-ring performs well in most categories")
        else:
            report.append("⚠️ **Needs Optimization**: Consider reviewing implementation for performance improvements")

        report.append("")
        report.append("### Optimization Opportunities")
        report.append("- Monitor pool utilization to optimize capacity settings")
        report.append("- Profile memory access patterns for cache optimization")
        report.append("- Consider NUMA-aware allocation for large-scale deployment")
        report.append("- Benchmark on target production hardware")

        return "\n".join(report)

    def create_performance_charts(self, output_dir: Path):
        """Create performance visualization charts."""
        output_dir.mkdir(exist_ok=True)

        # Allocation performance chart
        allocation_analysis = self.analyze_allocation_performance()
        if allocation_analysis['pool_performance']:
            plt.figure(figsize=(10, 6))
            pools = list(allocation_analysis['pool_performance'].keys())
            times = list(allocation_analysis['pool_performance'].values())

            plt.bar(pools, times, color=['skyblue', 'lightgreen', 'lightcoral', 'lightyellow', 'lightpink'])
            plt.title('Allocation Performance by Pool Size')
            plt.xlabel('Pool Size')
            plt.ylabel('Time (nanoseconds)')
            plt.yscale('log')

            for i, time in enumerate(times):
                plt.text(i, time, f'{time:.1f}', ha='center', va='bottom')

            plt.tight_layout()
            plt.savefig(output_dir / 'allocation_performance.png', dpi=300, bbox_inches='tight')
            plt.close()

        # Concurrent scaling chart
        concurrent_analysis = self.analyze_concurrent_performance()
        if concurrent_analysis['scaling']:
            plt.figure(figsize=(10, 6))
            threads = list(concurrent_analysis['scaling'].keys())
            efficiencies = [s['efficiency_percent'] for s in concurrent_analysis['scaling'].values()]

            plt.plot(threads, efficiencies, 'o-', linewidth=2, markersize=8)
            plt.axhline(y=100, color='r', linestyle='--', alpha=0.7, label='Ideal (100%)')
            plt.title('Concurrent Scaling Efficiency')
            plt.xlabel('Number of Threads')
            plt.ylabel('Scaling Efficiency (%)')
            plt.grid(True, alpha=0.3)
            plt.legend()

            plt.tight_layout()
            plt.savefig(output_dir / 'concurrent_scaling.png', dpi=300, bbox_inches='tight')
            plt.close()

        print(f"Performance charts saved to {output_dir}")


def main():
    parser = argparse.ArgumentParser(description='Analyze rusted-ring benchmark results')
    parser.add_argument('--criterion-dir', type=Path, default=Path('target/criterion'),
                        help='Path to Criterion benchmark results directory')
    parser.add_argument('--output-dir', type=Path, default=Path('performance_analysis'),
                        help='Output directory for analysis results')
    parser.add_argument('--report-only', action='store_true',
                        help='Generate only the text report, skip charts')

    args = parser.parse_args()

    try:
        # Create analyzer
        analyzer = BenchmarkAnalyzer(args.criterion_dir)

        # Create output directory
        args.output_dir.mkdir(exist_ok=True)

        # Generate performance report
        report = analyzer.generate_performance_report()

        # Save report
        report_file = args.output_dir / 'performance_report.md'
        with open(report_file, 'w') as f:
            f.write(report)

        print(f"Performance report saved to: {report_file}")

        # Create charts if requested
        if not args.report_only:
            try:
                analyzer.create_performance_charts(args.output_dir)
            except ImportError:
                print("Warning: matplotlib not available, skipping charts")
                print("Install with: pip install matplotlib pandas")

        # Print summary to console
        print("\n" + "="*60)
        print("PERFORMANCE ANALYSIS SUMMARY")
        print("="*60)

        # Extract key findings from report
        lines = report.split('\n')
        in_recommendations = False
        for line in lines:
            if line.startswith('## Overall Recommendations'):
                in_recommendations = True
                continue
            if in_recommendations and line.startswith('✅'):
                print(line)
            if in_recommendations and line.startswith('⚠️'):
                print(line)

        print(f"\nFull report available at: {report_file}")

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)


if __name__ == '__main__':
    main()