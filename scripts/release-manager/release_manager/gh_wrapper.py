"""
Wrapper for GitHub client.
"""

# pylint: disable=import-error

import os

from github import Auth, Github
from release_manager.errors import CommandError

def get_client() -> (Github, str):
    """
    Returns an authenticated GitHub client.
    """
    gh_token = os.getenv("GITHUB_TOKEN")

    if gh_token is None:
        raise CommandError(
            "GITHUB_TOKEN must be set in environment to access GitHub API"
        )

    return Github(auth=Auth.Token(gh_token)), gh_token
