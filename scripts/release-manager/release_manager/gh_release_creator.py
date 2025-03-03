"""
Creates a GitHub release.
"""

# pylint: disable=import-error

import json
import requests
import semver

from github import GithubException
from release_manager import gh_wrapper
from release_manager.errors import CommandError

from release_manager.constants import NILLION_REPO

# pylint: disable=too-many-locals
def create_github_release(tag_name: str, release_name: str):
    """
    Creates a GitHub release.
    """

    # Start using GitHub API.
    gh_client, token = gh_wrapper.get_client()

    try:
        nillion_repo = gh_client.get_repo(NILLION_REPO)
    except GithubException as gh_ex:
        raise CommandError(
            f"An error occurred getting nillion repo from GitHub API: {gh_ex}"
        ) from gh_ex

    try:
        releases = nillion_repo.get_releases()
    except GithubException as gh_ex:
        raise CommandError(
            f"An error occurred getting releases from GitHub API: {gh_ex}"
        ) from gh_ex

    releases = sorted(releases, key=lambda r: r.created_at, reverse=True)

    latest_release = releases[0] if len(releases) > 0 else None

    # Determine if it's a pre-release
    semver_p = semver.parse(release_name.lstrip(f"v"))
    is_prerelease = bool(semver_p['prerelease'] or semver_p['build'])

    print(f"Creating GitHub {'pre-' if is_prerelease else ''}release {release_name} from tag {tag_name}")

    # Generate release notes.
    #
    # PyGithub doesn't support GitHub API's generate-notes endpoint so we have
    # to use requests and call it the old fashioned way.
    release_notes = ""

    if latest_release is not None:
        try:
            resp = requests.post(
                f"https://api.github.com/repos/{NILLION_REPO}/releases/generate-notes",
                data=json.dumps({
                    "previous_tag_name": latest_release.tag_name,
                    "tag_name": release_name,
                }),
                headers={
                    "Accept": "application/vnd.github+json",
                    "Authorization": f"Bearer {token}",
                    "X-GitHub-Api-Version": "2022-11-28",
                },
                timeout=10, # Seconds
            )
            resp.raise_for_status()

            generate_notes_resp = resp.json()
            release_notes = generate_notes_resp["body"]
        except requests.HTTPError as http_ex:
            raise CommandError((
                f"An error occurred generating release notes with the GitHub API: {http_ex}"
            )) from http_ex
        except requests.exceptions.JSONDecodeError as json_ex:
            raise CommandError((
                f"An error occurred decoding release notes response from GitHub API: {json_ex}"
            )) from json_ex

    # Create the release.
    try:
        nillion_repo.create_git_release(
            generate_release_notes=False,
            message=release_notes,
            name=release_name,
            prerelease=is_prerelease,
            tag=tag_name,
        )
    except GithubException as gh_ex:
        raise CommandError(
            f"An error occurred creating a Git tag and release with the GitHub API: {gh_ex}"
        ) from gh_ex
