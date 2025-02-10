"""
Sets up arg parser for load-test-manager.
"""

import argparse


def parse():
    """
    Sets up arg parser for commands.
    """
    arg_parser = argparse.ArgumentParser(description="A load test manager tool")
    sub_parser = arg_parser.add_subparsers(title="commands", dest="command")

    setup_grafana_snapshot_parser(sub_parser)
    setup_render_spec_parser(sub_parser)

    return arg_parser.parse_args()


def setup_grafana_snapshot_parser(sub_parser):
    """
    Sets up arg parser for the grafana-snapshot command.
    """
    child_parser = sub_parser.add_parser(
        "grafana-snapshot", description="Take a Grafana snapshot"
    )

    child_parser.add_argument("uuid", type=str, help="UUID of dashboard")
    child_parser.add_argument(
        "start_time_secs",
        help="Start time of load test in seconds",
        type=int,
    )
    child_parser.add_argument(
        "end_time_secs", type=int, help="End time of load test in seconds"
    )
    child_parser.add_argument(
        "env_name", type=str, help="Name of nillion-network environment"
    )


def setup_render_spec_parser(sub_parser):
    """
    Sets up arg parser for the render-spec command.
    """
    child_parser = sub_parser.add_parser(
        "render-spec", description="Render a spec with dynamic parameters"
    )

    child_parser.add_argument("spec_path", type=str, help="Path to load test spec")
    child_parser.add_argument(
        "--max-flow-duration",
        default=None,
        help="Max flow duration of load test",
        type=str,
    )
    child_parser.add_argument(
        "--max-test-duration",
        default=None,
        help="Max test duration of load test",
        type=str,
    )
    child_parser.add_argument(
        "--operation-input-size", default=None, help="Size of input", type=str
    )
    child_parser.add_argument("--workers", default=0, help="Size of input", type=int)
    child_parser.add_argument(
        "--required-starting-balance", default=0, help="Size of input", type=int
    )
