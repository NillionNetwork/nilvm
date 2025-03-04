"""
Checks GitHub for tags.
"""

# pylint: disable=import-error

from github import GithubException
from release_manager import gh_wrapper
from release_manager.errors import CommandError, NotFoundError

def check_github_tags(repo_name: str, release_version_prefix: str=""):
    """
    A function that conforms to existing methods of checking for releases.
    """
    def _check_github_tags(release_version: str):
        """
        Checks GitHub for existing tags with the release version.
        """
        gh_client, _ = gh_wrapper.get_client()

        try:
            repo = gh_client.get_repo(repo_name)
        except GithubException as ex:
            raise CommandError(
                f"An error occurred getting nillion repo from GitHub API: {ex}"
            ) from ex

        if not release_version.startswith(release_version_prefix):
            release_version=f"{release_version_prefix}{release_version}"

        try:
            ref = repo.get_git_ref(f"tags/{release_version}")
        except GithubException as ex:
            if ex.status == 404:
                raise NotFoundError(
                    f"Ref for tag '{release_version}' returned from GitHub API not found"
                ) from ex

            raise CommandError((
                f"An error occurred getting ref for tag '{release_version}' "
                "from GitHub API"
            )) from ex

        if ref is None or ref.ref is None:
            raise NotFoundError(
                f"Ref for tag '{release_version}' returned from GitHub API not found"
            )

    return _check_github_tags
