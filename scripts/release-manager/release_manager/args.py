"""
Arg parser for release manager.
"""

# pylint: disable=import-error, invalid-name

import argparse

def setup_arg_parser() -> (argparse.ArgumentParser, argparse.ArgumentParser):
    """
    Sets up arg parsers.
    """
    arg_parser = argparse.ArgumentParser(description="A tool for release management")
    sub_parser = arg_parser.add_subparsers(title="commands", dest="command")

    setup_create_github_release(sub_parser)
    setup_delete_release(sub_parser)
    setup_get_release_next_version(sub_parser)
    setup_get_releases(sub_parser)
    setup_promote_release(sub_parser)

    return arg_parser, sub_parser

def setup_create_github_release(arg_parser: argparse.ArgumentParser):
    """
    Sets up parser for create-github-release.
    """
    # Create parser.
    child_parser = arg_parser.add_parser("create-github-release",
            description="Creates a GitHub release")

    # Add arguments
    child_parser.add_argument("tag_name", type=str, help="Name of existing tag")
    child_parser.add_argument("release_name", type=str, help="Name to give release")

def setup_delete_release(parser: argparse.ArgumentParser):
    """
    Sets up arg parser for delete-release.
    """
    child_parser = parser.add_parser("delete-release", description="Delete a release")
    child_parser.add_argument("release_version", type=str, help="Release version to delete")
    child_parser.add_argument("--force", action=argparse.BooleanOptionalAction,
            help="Ignore errors from intermediate deletion steps")
    child_parser.set_defaults(force=False)

def setup_get_release_next_version(parser: argparse.ArgumentParser):
    """
    Sets up arg parser for get-release-next-version.
    """
    child_parser = parser.add_parser("get-release-next-version",
            description="Gets next version for release")

    # Optional flags.
    child_parser.add_argument(
        "--release-candidate-base-version",
        default=None,
        help="Base version from which next version should be derived",
        type=str,
    )

    # Required arguments.
    child_parser.add_argument("bump_type",
            choices=["patch", "minor", "major", "prerelease", "promote"],
            help="type of version bump", type=str)
    child_parser.add_argument("latest_version", help="Version to bump", type=str)

def setup_get_releases(parser: argparse.ArgumentParser):
    """
    Sets up arg parser for get-releases.
    """
    child_parser = parser.add_parser("get-releases", description="Get releases")
    child_parser.add_argument("--filter",
        choices=["incremental", "nightly", "testnet", "all"],
        help="Filter releases by the release type",
        type=str,
    )
    child_parser.set_defaults(filter="all")

def setup_promote_release(parser: argparse.ArgumentParser):
    """
    Sets up arg parser for promote-release.
    """
    child_parser = parser.add_parser("promote-release", description="promote a release")
    child_parser.add_argument("from_version", type=str, help="Release version to promote")
    child_parser.add_argument("to_version", type=str, help="Release version to promote to")
