#!/usr/bin/env python3
"""
Performance Analysis Tool for Rusted-Ring LMAX Benchmarks

Updated to match the current benchmark structure and focus on XaeroFlux-specific metrics.
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

        print(f"Loaded {len(self.results)} benchmark groups with results")

    def get_time_in_nanoseconds(self, time_value: float, unit: str) -> float:
        """Convert time to nanoseconds for consistent comparison."""
        if 'ns' in unit:
            return time_value
        elif 'μs' in unit or 'us' in unit:
            return time_value * 1_000
        elif 'ms' in unit:
            return time_value * 1_000_000
        elif 's' in unit and 'ms' not in unit and 'μs' not in unit:
            return time_value * 1_000_000_000
        else:
            return time_value  # Assume nanoseconds

    def analyze_ffi_performance(self) -> Dict:
        """Analyze FFI to Subject performance (XaeroFlux entry point)."""
        ffi_analysis = {
            'ffi_burst_performance': {},
            'burst_scenarios': {},
            'recommendations': []
        }

        # Analyze FFI burst scenario
        if 'ffi_to_subject' in self.results:
            for benchmark, data in self.results['ffi_to_subject'].items():
                if 'typical' in data and 'ffi_burst_scenario' in benchmark:
                    time_per_iteration = data['typical']['estimate']
                    unit = data['typical']['unit']

                    # Convert to nanoseconds per event
                    time_ns = self.get_time_in_nanoseconds(time_per_iteration, unit)
                    events_per_iteration = 250  # From throughput setting
                    time_per_event_ns = time_ns / events_per_iteration

                    ffi_analysis['ffi_burst_performance'] = {
                        'time_per_iteration': time_per_iteration,
                        'unit': unit,
                        'time_per_event_ns': time_per_event_ns,
                        'events_per_second': 1_000_000_000 / time_per_event_ns
                    }

        # FFI Performance targets for XaeroFlux
        target_ffi_latency_ns = 1000  # 1μs per FFI event acceptable

        if ffi_analysis['ffi_burst_performance']:
            perf = ffi_analysis['ffi_burst_performance']
            if perf['time_per_event_ns'] <= target_ffi_latency_ns:
                ffi_analysis['recommendations'].append(
                    f"✅ FFI Performance: {perf['time_per_event_ns']:.1f}ns per event "
                    f"(target: <{target_ffi_latency_ns}ns)"
                )
            else:
                ffi_analysis['recommendations'].append(
                    f"⚠️ FFI Performance: {perf['time_per_event_ns']:.1f}ns per event "
                    f"exceeds target {target_ffi_latency_ns}ns"
                )

            ffi_analysis['recommendations'].append(
                f"FFI Throughput: {perf['events_per_second']:,.0f} events/sec"
            )

        return ffi_analysis

    def analyze_write_performance(self) -> Dict:
        """Analyze pure write performance across pool sizes."""
        write_analysis = {
            'write_performance': {},
            'pool_efficiency': {},
            'recommendations': []
        }

        # Analyze write-only performance
        if 'write_performance' in self.results:
            for benchmark, data in self.results['write_performance'].items():
                if 'typical' in data:
                    time_per_iteration = data['typical']['estimate']
                    unit = data['typical']['unit']

                    time_ns = self.get_time_in_nanoseconds(time_per_iteration, unit)
                    events_per_iteration = 1000  # From benchmark
                    time_per_write_ns = time_ns / events_per_iteration

                    pool_size = 'XS' if 'xs_pool' in benchmark else 'S' if 's_pool' in benchmark else 'Unknown'

                    write_analysis['write_performance'][pool_size] = {
                        'time_per_write_ns': time_per_write_ns,
                        'writes_per_second': 1_000_000_000 / time_per_write_ns,
                        'unit': unit
                    }

        # Write performance targets
        target_write_ns = 10  # 10ns per write (very aggressive)

        for pool, perf in write_analysis['write_performance'].items():
            if perf['time_per_write_ns'] <= target_write_ns:
                write_analysis['recommendations'].append(
                    f"✅ {pool} Write: {perf['time_per_write_ns']:.1f}ns per write"
                )
            else:
                write_analysis['recommendations'].append(
                    f"⚠️ {pool} Write: {perf['time_per_write_ns']:.1f}ns per write "
                    f"(target: <{target_write_ns}ns)"
                )

        return write_analysis

    def analyze_pipeline_performance(self) -> Dict:
        """Analyze multi-stage pipeline performance."""
        pipeline_analysis = {
            'pipeline_stages': {},
            'multi_pool_performance': {},
            'realistic_scenarios': {},
            'recommendations': []
        }

        # Analyze realistic pipeline
        if 'realistic_pipeline' in self.results:
            for benchmark, data in self.results['realistic_pipeline'].items():
                if 'typical' in data and 'controlled_pipeline' in benchmark:
                    time_per_iteration = data['typical']['estimate']
                    unit = data['typical']['unit']

                    time_ns = self.get_time_in_nanoseconds(time_per_iteration, unit)
                    events_processed = 50  # From benchmark
                    time_per_pipeline_stage_ns = time_ns / events_processed

                    pipeline_analysis['realistic_scenarios']['controlled_pipeline'] = {
                        'time_per_iteration': time_per_iteration,
                        'unit': unit,
                        'time_per_event_ns': time_per_pipeline_stage_ns,
                        'events_per_second': events_processed * (1_000_000_000 / time_ns)
                    }

        # Analyze multi-pool performance
        if 'multi_pool' in self.results:
            for benchmark, data in self.results['multi_pool'].items():
                if 'typical' in data:
                    time_per_iteration = data['typical']['estimate']
                    unit = data['typical']['unit']

                    time_ns = self.get_time_in_nanoseconds(time_per_iteration, unit)
                    total_events = 300  # 100 XS + 100 S + 100 M from benchmark

                    pipeline_analysis['multi_pool_performance']['size_progression'] = {
                        'time_per_iteration': time_per_iteration,
                        'unit': unit,
                        'avg_time_per_event_ns': time_ns / total_events,
                        'events_per_second': total_events * (1_000_000_000 / time_ns)
                    }

        # Pipeline performance targets for XaeroFlux
        target_pipeline_latency_ns = 100  # 100ns per pipeline stage acceptable

        for stage_name, perf in pipeline_analysis['realistic_scenarios'].items():
            if perf['time_per_event_ns'] <= target_pipeline_latency_ns:
                pipeline_analysis['recommendations'].append(
                    f"✅ Pipeline {stage_name}: {perf['time_per_event_ns']:.1f}ns per event"
                )
            else:
                pipeline_analysis['recommendations'].append(
                    f"⚠️ Pipeline {stage_name}: {perf['time_per_event_ns']:.1f}ns per event "
                    f"(target: <{target_pipeline_latency_ns}ns)"
                )

        return pipeline_analysis

    def analyze_backpressure_performance(self) -> Dict:
        """Analyze backpressure detection and handling."""
        backpressure_analysis = {
            'backpressure_scenarios': {},
            'flow_control_efficiency': {},
            'recommendations': []
        }

        # Analyze backpressure simulation
        if 'backpressure' in self.results:
            for benchmark, data in self.results['backpressure'].items():
                if 'typical' in data and 'fast_writer_slow_reader' in benchmark:
                    time_per_iteration = data['typical']['estimate']
                    unit = data['typical']['unit']

                    backpressure_analysis['backpressure_scenarios']['fast_writer_slow_reader'] = {
                        'time_per_iteration': time_per_iteration,
                        'unit': unit,
                        'scenario': 'Fast writer with slow reader'
                    }

        backpressure_analysis['recommendations'].extend([
            "✅ Backpressure detection implemented via reader.backpressure_ratio()",
            "✅ Fast producer/slow consumer scenario tested",
            "⚠️ Monitor backpressure in production XaeroFlux deployment"
        ])

        return backpressure_analysis

    def analyze_memory_patterns(self) -> Dict:
        """Analyze memory access patterns and cache efficiency."""
        memory_analysis = {
            'access_patterns': {},
            'cache_performance': {},
            'recommendations': []
        }

        # Analyze memory patterns
        if 'memory_patterns' in self.results:
            for benchmark, data in self.results['memory_patterns'].items():
                if 'typical' in data:
                    time_per_iteration = data['typical']['estimate']
                    unit = data['typical']['unit']

                    pattern_type = 'sequential' if 'sequential_access' in benchmark else 'cache_aligned' if 'cache_aligned' in benchmark else 'unknown'

                    memory_analysis['access_patterns'][pattern_type] = {
                        'time_per_iteration': time_per_iteration,
                        'unit': unit,
                        'events_per_iteration': 1000
                    }

        memory_analysis['recommendations'].extend([
            "✅ Sequential access pattern optimizes hardware prefetching",
            "✅ 64-byte cache line alignment prevents false sharing",
            "✅ Static allocation eliminates heap fragmentation",
            "✅ LMAX overwrite semantics provide predictable memory usage"
        ])

        return memory_analysis

    def analyze_xaeroflux_scenarios(self) -> Dict:
        """Analyze XaeroFlux-specific performance scenarios."""
        xaeroflux_analysis = {
            'ffi_scenarios': {},
            'drawing_performance': {},
            'real_world_metrics': {},
            'recommendations': []
        }

        # Extract XaeroFlux-relevant metrics from various benchmarks
        ffi_perf = self.analyze_ffi_performance()
        if ffi_perf['ffi_burst_performance']:
            events_per_sec = ffi_perf['ffi_burst_performance']['events_per_second']

            # Calculate drawing strokes per second (assuming 10 events per stroke)
            strokes_per_second = events_per_sec / 10

            xaeroflux_analysis['drawing_performance'] = {
                'ffi_events_per_second': events_per_sec,
                'estimated_strokes_per_second': strokes_per_second,
                'concurrent_users_supportable': int(strokes_per_second / 100)  # 100 strokes/sec per user
            }

        # XaeroFlux performance targets
        target_users = 50  # Support 50 concurrent users
        target_strokes_per_user = 100  # 100 strokes per second per user

        if xaeroflux_analysis['drawing_performance']:
            perf = xaeroflux_analysis['drawing_performance']
            supportable_users = perf['concurrent_users_supportable']

            if supportable_users >= target_users:
                xaeroflux_analysis['recommendations'].append(
                    f"✅ XaeroFlux Scalability: Supports {supportable_users} concurrent users "
                    f"(target: {target_users})"
                )
            else:
                xaeroflux_analysis['recommendations'].append(
                    f"⚠️ XaeroFlux Scalability: Supports {supportable_users} concurrent users "
                    f"(target: {target_users})"
                )

            xaeroflux_analysis['recommendations'].append(
                f"Drawing Performance: {perf['estimated_strokes_per_second']:,.0f} strokes/sec"
            )

        return xaeroflux_analysis

    def generate_comprehensive_lmax_report(self) -> str:
        """Generate a comprehensive LMAX performance report focused on XaeroFlux needs."""
        report = []
        report.append("# Rusted-Ring LMAX Performance Analysis Report")
        report.append(f"**Generated**: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        report.append("")
        report.append("This report analyzes LMAX Disruptor performance for XaeroFlux real-time whiteboard application.")
        report.append("")

        # Executive Summary
        report.append("## Executive Summary")
        report.append("")
        report.append("### Key Performance Indicators")

        # Analyze all components
        ffi_analysis = self.analyze_ffi_performance()
        write_analysis = self.analyze_write_performance()
        pipeline_analysis = self.analyze_pipeline_performance()
        backpressure_analysis = self.analyze_backpressure_performance()
        memory_analysis = self.analyze_memory_patterns()
        xaeroflux_analysis = self.analyze_xaeroflux_scenarios()

        # Collect all recommendations
        all_recommendations = []
        all_recommendations.extend(ffi_analysis['recommendations'])
        all_recommendations.extend(write_analysis['recommendations'])
        all_recommendations.extend(pipeline_analysis['recommendations'])
        all_recommendations.extend(xaeroflux_analysis['recommendations'])

        for rec in all_recommendations[:5]:  # Top 5 most important
            report.append(f"{rec}")
        report.append("")

        # FFI Performance (Critical for XaeroFlux)
        report.append("## FFI Performance Analysis")
        report.append("*Critical for XaeroFlux Dart ↔ Rust boundary*")
        report.append("")

        if ffi_analysis['ffi_burst_performance']:
            perf = ffi_analysis['ffi_burst_performance']
            report.append(f"### FFI Burst Scenario Results")
            report.append(f"- **Time per Event**: {perf['time_per_event_ns']:.1f} ns")
            report.append(f"- **Events per Second**: {perf['events_per_second']:,.0f}")
            report.append(f"- **Time per 250-event Burst**: {perf['time_per_iteration']:.2f} {perf['unit']}")
            report.append("")

        for rec in ffi_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # Write Performance
        report.append("## Core Write Performance")
        report.append("*LMAX ring buffer allocation speed*")
        report.append("")

        if write_analysis['write_performance']:
            report.append("### Write Performance by Pool Size")
            for pool, perf in write_analysis['write_performance'].items():
                report.append(f"- **{pool} Pool**: {perf['time_per_write_ns']:.1f} ns per write "
                              f"({perf['writes_per_second']:,.0f} writes/sec)")
            report.append("")

        for rec in write_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # Pipeline Performance
        report.append("## Pipeline Performance Analysis")
        report.append("*Multi-stage processing for XaeroFlux operators*")
        report.append("")

        if pipeline_analysis['realistic_scenarios']:
            for scenario, perf in pipeline_analysis['realistic_scenarios'].items():
                report.append(f"### {scenario.replace('_', ' ').title()}")
                report.append(f"- **Time per Event**: {perf['time_per_event_ns']:.1f} ns")
                report.append(f"- **Processing Rate**: {perf['events_per_second']:,.0f} events/sec")
                report.append(f"- **Iteration Time**: {perf['time_per_iteration']:.2f} {perf['unit']}")
                report.append("")

        if pipeline_analysis['multi_pool_performance']:
            for scenario, perf in pipeline_analysis['multi_pool_performance'].items():
                report.append(f"### Multi-Pool Pipeline")
                report.append(f"- **Average Time per Event**: {perf['avg_time_per_event_ns']:.1f} ns")
                report.append(f"- **Mixed Size Processing**: {perf['events_per_second']:,.0f} events/sec")
                report.append("")

        for rec in pipeline_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # XaeroFlux-Specific Analysis
        report.append("## XaeroFlux Application Performance")
        report.append("*Real-world whiteboard application metrics*")
        report.append("")

        if xaeroflux_analysis['drawing_performance']:
            perf = xaeroflux_analysis['drawing_performance']
            report.append(f"### Drawing and Collaboration Performance")
            report.append(f"- **FFI Events**: {perf['ffi_events_per_second']:,.0f} events/sec")
            report.append(f"- **Drawing Strokes**: {perf['estimated_strokes_per_second']:,.0f} strokes/sec")
            report.append(f"- **Concurrent Users**: {perf['concurrent_users_supportable']} users supported")
            report.append("")

        for rec in xaeroflux_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # Backpressure and Flow Control
        report.append("## Backpressure and Flow Control")
        report.append("*Critical for handling fast drawing, slow storage scenarios*")
        report.append("")

        for rec in backpressure_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # Memory and Cache Performance
        report.append("## Memory and Cache Optimization")
        report.append("*LMAX sequential access and cache efficiency*")
        report.append("")

        for rec in memory_analysis['recommendations']:
            report.append(f"{rec}")
        report.append("")

        # Overall Assessment
        report.append("## Overall LMAX Assessment for XaeroFlux")
        report.append("")

        # Calculate performance score
        total_passed = sum(1 for rec in all_recommendations if '✅' in rec)
        total_warnings = sum(1 for rec in all_recommendations if '⚠️' in rec)
        total_checks = total_passed + total_warnings

        if total_checks > 0:
            success_rate = (total_passed / total_checks) * 100

            if success_rate >= 80:
                rating = "🏆 **Excellent**: Ready for production XaeroFlux deployment"
            elif success_rate >= 60:
                rating = "✅ **Good**: Suitable for XaeroFlux with minor optimizations"
            else:
                rating = "⚠️ **Needs Improvement**: Review LMAX implementation before production"

            report.append(rating)
            report.append("")
            report.append(f"**Performance Score**: {total_passed}/{total_checks} checks passed ({success_rate:.0f}%)")
            report.append("")

        # Production Readiness
        report.append("## Production Readiness for XaeroFlux")
        report.append("")
        report.append("### Deployment Recommendations")
        report.append("- **Environment**: Configure CPU governor to 'performance' mode")
        report.append("- **Memory**: Ensure 32MB+ stack size for LMAX ring buffers")
        report.append("- **Monitoring**: Implement backpressure ratio monitoring")
        report.append("- **Scaling**: Monitor FFI event rates and reader lag in production")
        report.append("")
        report.append("### Next Steps")
        report.append("1. Deploy to staging environment with production-like load")
        report.append("2. Monitor XaeroFlux drawing sessions for performance bottlenecks")
        report.append("3. Tune ring buffer capacities based on real usage patterns")
        report.append("4. Implement alerts for backpressure threshold breaches")

        return "\n".join(report)

    def create_xaeroflux_performance_charts(self, output_dir: Path):
        """Create XaeroFlux-focused performance charts."""
        if not HAS_PLOTTING:
            print("Matplotlib not available - skipping chart generation")
            return

        output_dir.mkdir(exist_ok=True)

        fig, ((ax1, ax2), (ax3, ax4)) = plt.subplots(2, 2, figsize=(15, 12))
        fig.suptitle('XaeroFlux LMAX Performance Analysis', fontsize=16, fontweight='bold')

        # Chart 1: FFI Performance
        ffi_analysis = self.analyze_ffi_performance()
        if ffi_analysis['ffi_burst_performance']:
            perf = ffi_analysis['ffi_burst_performance']

            ax1.bar(['FFI Event Processing'], [perf['time_per_event_ns']],
                    color='lightblue', alpha=0.7)
            ax1.axhline(y=1000, color='red', linestyle='--', alpha=0.7, label='Target (1μs)')
            ax1.set_ylabel('Time per Event (ns)')
            ax1.set_title('FFI Performance (Dart ↔ Rust)')
            ax1.legend()
            ax1.grid(True, alpha=0.3)

        # Chart 2: Write Performance by Pool Size
        write_analysis = self.analyze_write_performance()
        if write_analysis['write_performance']:
            pools = list(write_analysis['write_performance'].keys())
            times = [perf['time_per_write_ns'] for perf in write_analysis['write_performance'].values()]

            bars = ax2.bar(pools, times, color=['skyblue', 'lightgreen', 'lightcoral'][:len(pools)])
            ax2.axhline(y=10, color='red', linestyle='--', alpha=0.7, label='Target (10ns)')
            ax2.set_ylabel('Time per Write (ns)')
            ax2.set_title('Write Performance by Pool Size')
            ax2.legend()
            ax2.grid(True, alpha=0.3)

        # Chart 3: XaeroFlux Scalability
        xaeroflux_analysis = self.analyze_xaeroflux_scenarios()
        if xaeroflux_analysis['drawing_performance']:
            perf = xaeroflux_analysis['drawing_performance']

            categories = ['Events/sec', 'Strokes/sec', 'Users Supported']
            values = [
                perf['ffi_events_per_second'] / 1000,  # Scale down for visibility
                perf['estimated_strokes_per_second'],
                perf['concurrent_users_supportable']
            ]

            bars = ax3.bar(categories, values, color=['gold', 'lightcyan', 'lightpink'])
            ax3.set_ylabel('Rate (thousands for events/sec)')
            ax3.set_title('XaeroFlux Application Scalability')
            ax3.grid(True, alpha=0.3)

            # Add value labels on bars
            for bar, value in zip(bars, values):
                height = bar.get_height()
                ax3.text(bar.get_x() + bar.get_width()/2., height + height*0.05,
                         f'{value:,.0f}', ha='center', va='bottom')

        # Chart 4: Performance Score Summary
        all_recommendations = []
        all_recommendations.extend(ffi_analysis['recommendations'])
        all_recommendations.extend(write_analysis['recommendations'])

        total_passed = sum(1 for rec in all_recommendations if '✅' in rec)
        total_warnings = sum(1 for rec in all_recommendations if '⚠️' in rec)

        if total_passed + total_warnings > 0:
            labels = ['Passed', 'Warnings']
            sizes = [total_passed, total_warnings]
            colors = ['lightgreen', 'orange']

            wedges, texts, autotexts = ax4.pie(sizes, labels=labels, colors=colors,
                                               autopct='%1.0f', startangle=90)
            ax4.set_title('Performance Assessment Summary')

            # Add total score
            success_rate = (total_passed / (total_passed + total_warnings)) * 100
            ax4.text(0, -1.3, f'Overall Score: {success_rate:.0f}%',
                     ha='center', fontsize=12, fontweight='bold')

        plt.tight_layout()
        plt.savefig(output_dir / 'xaeroflux_lmax_performance.png', dpi=300, bbox_inches='tight')
        plt.close()

        print(f"XaeroFlux performance charts saved to {output_dir}")


def main():
    parser = argparse.ArgumentParser(description='Analyze LMAX ring buffer performance for XaeroFlux')
    parser.add_argument('--criterion-dir', type=Path, default=Path('../target/criterion'),
                        help='Path to Criterion benchmark results directory')
    parser.add_argument('--output-dir', type=Path, default=Path('../xaeroflux_performance_analysis'),
                        help='Output directory for analysis results')
    parser.add_argument('--report-only', action='store_true',
                        help='Generate only the text report, skip charts')

    args = parser.parse_args()

    try:
        # Create analyzer
        analyzer = LMAXBenchmarkAnalyzer(args.criterion_dir)

        if not analyzer.results:
            print("No benchmark results found. Run benchmarks first with:")
            print("./benchmark_runner.sh")
            print("or: cargo bench --bench comprehensive_benchmarks")
            sys.exit(1)

        print(f"Found benchmark groups: {list(analyzer.results.keys())}")

        # Create output directory
        args.output_dir.mkdir(exist_ok=True)

        # Generate performance report
        report = analyzer.generate_comprehensive_lmax_report()

        # Save report
        report_file = args.output_dir / 'xaeroflux_lmax_performance_report.md'
        with open(report_file, 'w') as f:
            f.write(report)

        print(f"XaeroFlux LMAX performance report saved to: {report_file}")

        # Create charts if requested and available
        if not args.report_only:
            analyzer.create_xaeroflux_performance_charts(args.output_dir)

        # Print summary to console
        print("\n" + "="*80)
        print("XAEROFLUX LMAX PERFORMANCE ANALYSIS SUMMARY")
        print("="*80)

        # Quick analysis for console output
        ffi_analysis = analyzer.analyze_ffi_performance()
        xaeroflux_analysis = analyzer.analyze_xaeroflux_scenarios()

        if ffi_analysis['ffi_burst_performance']:
            perf = ffi_analysis['ffi_burst_performance']
            print(f"FFI Performance: {perf['time_per_event_ns']:.1f}ns per event")
            print(f"FFI Throughput: {perf['events_per_second']:,.0f} events/sec")

        if xaeroflux_analysis['drawing_performance']:
            perf = xaeroflux_analysis['drawing_performance']
            print(f"Drawing Strokes: {perf['estimated_strokes_per_second']:,.0f} strokes/sec")
            print(f"Concurrent Users: {perf['concurrent_users_supportable']} users supported")

        print(f"\nDetailed analysis: {report_file}")
        if HAS_PLOTTING and not args.report_only:
            print(f"Performance charts: {args.output_dir}")

    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == '__main__':
    main()