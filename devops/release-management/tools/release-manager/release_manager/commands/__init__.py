"""
File containing general classes related to command handling.
"""

from dataclasses import dataclass
from enum import auto, Enum

import boto3
import semver

from botocore.exceptions import ClientError
from github import GithubException
# pylint: disable=import-error
from release_manager import gh_wrapper
from release_manager.errors import CommandError, NotFoundError
from release_manager.gh_release_checker import check_github_tags
from release_manager.s3_artifact import S3Artifact
from release_manager.s3_artifact_checker import check_s3_artifacts
from release_manager.constants import (
    DEVOPS_REPO,
    FUNCTIONAL_TEST_ECR_REPO,
    NILLION_PYTHON_PACKAGES,
    NILLION_NPM_PACKAGES,
    NILLION_NPM_REGISTRY_URL,
    NILLION_REPO,
    NODE_ECR_REPO,
    NILLION_RELEASES_BUCKET,
    NILLION_PRIVATE_RELEASES_BUCKET
)
from tabulate import tabulate
from termcolor import colored


class CommandResultCode(Enum):
    """
    Enum which encapsulates types of results return from commands.
    """
    SUCCESS = auto()
    NO_METADATA_FOUND = auto()
    NO_RELEASE_CANDIDATES_MATCHING_TAG_CONVENTION = auto()

    def __str__(self):
        return self.name

@dataclass
# pylint: disable=too-few-public-methods
class CommandResult:
    """
    Class which encapsulates possible result of running command.
    """
    code: CommandResultCode
    non_success: bool = False

# pylint: disable=too-many-branches
def delete_release(release_version: str, force: bool=False):
    """
    Deletes a release from S3, GitHub and ECR.
    """
    for artifact in [
        S3Artifact(NILLION_RELEASES_BUCKET, release_version),
        S3Artifact(NILLION_PRIVATE_RELEASES_BUCKET, release_version),
    ]:
        try:
            artifact.delete()

            print(f"✓ Release has been deleted from S3 bucket '{artifact.bucket_name}'")
        except (CommandError, NotFoundError) as ex:
            if not force:
                raise ex

            print(f"❌Error deleting release from S3 bucket '{artifact.bucket_name}': {ex}")

    try:
        _delete_github_nillion_repo_tag(release_version)
    except (CommandError, NotFoundError) as ex:
        if force:
            print(f"❌Error deleting tag from nillion GitHub repo: {ex}")
        else:
            raise ex
    else:
        print("✓ Release tag has been deleted from nillion GitHub repo")

    try:
        _delete_github_devops_repo_tag(release_version)
    except (CommandError, NotFoundError) as ex:
        if force:
            print(f"❌Error deleting tag from devops GitHub repo: {ex}")
        else:
            raise ex
    else:
        print("✓ Release tag has been deleted from devops GitHub repo")

    try:
        _delete_ecr_docker_images(release_version, NODE_ECR_REPO,
                f"{release_version}-amd64", f"{release_version}-arm64")
    except (CommandError, NotFoundError) as ex:
        if force:
            print(f"❌Error deleting Docker image from ECR repo '{NODE_ECR_REPO}': {ex}")
        else:
            raise ex
    else:
        print(f"✓ Release node Docker image has been deleted from ECR repo '{NODE_ECR_REPO}'")

    try:
        _delete_ecr_docker_images(release_version, FUNCTIONAL_TEST_ECR_REPO,
                f"{release_version}-amd64")
    except (CommandError, NotFoundError) as ex:
        if force:
            print(f"❌Error deleting Docker image from ECR repo '{FUNCTIONAL_TEST_ECR_REPO}': {ex}")
        else:
            raise ex
    else:
        print(f"✓ Release Docker image has been deleted from ECR repo '{FUNCTIONAL_TEST_ECR_REPO}'")

    print(f"Release '{release_version}' has been deleted.")

def get_release_next_version(
    bump_type: str,
    latest_version: str,
    release_candidate_base_version: str
) -> str:
    """
    Prints next version according to bump type.
    """
    next_version = _get_release_next_version(
        bump_type,
        latest_version,
        release_candidate_base_version,
    )

    print(next_version)

def get_releases():
    """
    Prints releases.
    """
    tags = _get_tags()

    rows = []

    for tag in sorted(tags):
        statuses = []

        for check in [
            check_s3_artifacts(),
            check_github_tags(NILLION_REPO),
            check_github_tags(DEVOPS_REPO),
            _check_ecr_docker_image,
        ]:
            try:
                check(tag)
                statuses.append(colored("✓", "green"))
            except NotFoundError:
                statuses.append(colored("x", "red"))
            except CommandError as ex:
                statuses.append(colored(f"? (Error: {ex})", "yellow"))

        rows.append([tag, *statuses])

    headers = ["RELEASE", "S3", "GITHUB (nillion)", "GITHUB (devops)", "ECR"]
    print(tabulate(rows, headers, tablefmt="plain"))

# pylint: disable=too-many-branches
def promote_release(from_version: str, to_version: str=""):
    """
    Promotes a release in S3, and ECR.
    """

    if to_version == "":
        to_version=_get_release_next_version("promote", from_version)

    for artifact in [
        S3Artifact(NILLION_PRIVATE_RELEASES_BUCKET, from_version),
    ]:
        try:
            artifact.copy(to_version)

            print(f"✓ Release {from_version} has been promoted to {to_version} in S3 bucket '{artifact.bucket_name}'")
        except (CommandError, NotFoundError) as ex:
            raise ex

    for artifact in [
        S3Artifact(NILLION_RELEASES_BUCKET, from_version),
    ]:
        try:
            artifact.copy(to_version)
            print(f"✓ Release {from_version} has been promoted to {to_version} in S3 bucket '{artifact.bucket_name}'")

            artifact.copy(f"public/sdk/{to_version}")
            print(f"✓ Release {to_version} has been published to 'public/sdk/{to_version}' in S3 bucket '{artifact.bucket_name}'")

            artifact.sync("public/sdk/latest")
            print(f"✓ Release {to_version} has been published to 'public/sdk/latest' in S3 bucket '{artifact.bucket_name}'")

        except (CommandError, NotFoundError) as ex:
            raise ex

    try:
        _promote_ecr_docker_image(from_version, to_version, NODE_ECR_REPO)
    except (CommandError, NotFoundError) as ex:
        raise ex
    else:
        print(f"✓ Release node Docker image {from_version} has been promoted to {to_version}")

    try:
        _promote_ecr_docker_image(from_version, to_version, FUNCTIONAL_TEST_ECR_REPO)
    except (CommandError, NotFoundError) as ex:
        raise ex
    else:
        print(f"✓ Release functional-test Docker image {from_version} has been promoted to {to_version}")

    print(f"Release {from_version} has been promoted to {to_version}.")

def _check_ecr_docker_image(release_version: str):
    """
    Checks ECR nillion-node repo for Docker image.
    """
    ecr_client = boto3.client("ecr")

    # pylint: disable=broad-exception-caught
    image_tag = f"{release_version}-amd64"
    if not image_tag.startswith("v"):
        image_tag=f"v{image_tag}"

    try:
        images = ecr_client.describe_images(
            repositoryName=NODE_ECR_REPO,
            imageIds=[{"imageTag": image_tag}]
        )
    except ecr_client.exceptions.ImageNotFoundException as ex:
        raise NotFoundError(
            f"ECR image with tag '{release_version}' not found"
        ) from ex
    except ClientError as ex:
        raise CommandError(
            "An error occurred describing images from the ECR API in repo"
            f" '{NODE_ECR_REPO}' with image tag '{release_version}'"
        ) from ex

    if images is None:
        raise CommandError(
            "Empty images returned from ECR API for repo"
            f" '{NODE_ECR_REPO}' and image tag '{release_version}'"
        )

    if "imageDetails" not in images:
        raise CommandError(
            "Images with no image details returned from ECR API for repo"
            f" '{NODE_ECR_REPO}' and image tag '{release_version}'"
        )

def _delete_ecr_docker_images(release_version: str, ecr_repo: str, *image_ids: list[str]):
    """
    Deletes a Docker image from ECR.
    """
    ecr_client = boto3.client("ecr")

    try:
        response = ecr_client.batch_delete_image(
            repositoryName=ecr_repo,
            # ECR API call expects a list of image ID objects.
            imageIds=list(map(lambda x: {"imageTag": x}, image_ids))
        )
    except (ecr_client.exceptions.InvalidParameterException,
            ecr_client.exceptions.RepositoryNotFoundException,
            ecr_client.exceptions.ServerException) as ex:
        raise CommandError(
            "An error occurred batch deleting images from the ECR API in repo"
            f" '{ecr_repo}' with image tag '{release_version}'"
        ) from ex

    if "failures" in response and len(response["failures"]) > 0:
        image_not_found = []

        for failure in response["failures"]:
            if failure["failureCode"] == "ImageNotFound":
                image_not_found.append(failure["imageId"]["imageTag"])

        if len(image_not_found) > 0:
            raise NotFoundError(
                f"Image not found for tags: {', '.join(image_not_found)}"
            )

        raise CommandError(
            "An unexpected error was present in the response from the ECR API in repo"
            f" '{ecr_repo}' with image tag '{release_version}': {response['failures']}"
        )

def _delete_github_devops_repo_tag(release_version: str):
    """
    Deletes a tag from the devops GitHub repo.
    """
    _delete_github_repo_tag(DEVOPS_REPO, release_version)

def _delete_github_nillion_repo_tag(release_version: str):
    """
    Deletes a tag from the nillion GitHub repo.
    """
    _delete_github_repo_tag(NILLION_REPO, release_version)

def _delete_github_repo_tag(repo: str, release_version: str):
    """
    Deletes a tag from a GitHub repo.
    """
    gh_client, _ = gh_wrapper.get_client()

    try:
        repo = gh_client.get_repo(repo)
    except GithubException as ex:
        raise CommandError(
            f"An error occurred getting repo '{repo}' from GitHub API: {ex}"
        ) from ex

    try:
        ref = repo.get_git_ref(f"tags/{release_version}")
    except GithubException as ex:
        if ex.status == 404:
            raise NotFoundError(
                f"Ref for tag '{release_version}' returned from GitHub API not found"
            ) from ex

        raise CommandError(
            f"An error occurred getting ref for tag '{release_version}' from GitHub API"
        ) from ex

    if ref is None or ref.ref is None:
        raise NotFoundError(
            f"Ref for tag '{release_version}' returned from GitHub API not found"
        )

    try:
        ref.delete()
    except GithubException as ex:
        raise CommandError(
            f"An error occurred deleting ref for tag '{release_version}' from GitHub API"
        ) from ex

def _get_release_next_version(
    bump_type: str,
    latest_version: str,
    release_candidate_base_version: str=None
) -> str:
    """
    Gets next version.

    If a non-None release candidate base version is provided, then the next version will be the
    first version bump based on the base version and not the latest version. For example:

    Given:

    * Bump type: prerelease
    * Latest version: v0.8.0-rc.39
    * Base version: v0.9.0-rc.0

    Then next version: v0.9.0-rc.1
    """
    # Make latest_version semver compatible: v1.0.0 -> 1.0.0.
    semver_latest_version = latest_version.lstrip(f"v")
    semver_version = semver.Version.parse(semver_latest_version)

    if release_candidate_base_version is not None:
        semver_base_version = semver.Version.parse(release_candidate_base_version.lstrip("v"))

        if semver_version.finalize_version() != semver_base_version.finalize_version():
            semver_version = semver_base_version

    # Determine if it's a pre-release
    is_prerelease = bool(semver_version.prerelease or semver_version.build)

    if is_prerelease and bump_type == "promote":
        next_version = semver_version.finalize_version()
    else:
        if bump_type == "promote":
            raise CommandError(
                "Bump type 'promote' cannot be used with non-release candidate "
                f"latest versions '{latest_version}'"
            )
        else:
            next_version = semver_version.next_version(bump_type)

    return f"v{next_version}"

def _get_tags() -> list[str]:
    """
    Gets tags from nillion repo using GitHub API.
    """
    gh_client, _ = gh_wrapper.get_client()

    try:
        repo = gh_client.get_repo(NILLION_REPO)
    except GithubException as ex:
        raise CommandError(
            f"An error occurred getting nillion repo from GitHub API: {ex}"
        ) from ex

    try:
        tags = repo.get_tags()
    except GithubException as ex:
        raise CommandError(
            f"An error occurred getting tags from nillion repo from GitHub API: {ex}"
        ) from ex

    tag_names = []
    for tag in tags:
        tag_names.append(tag.name)

    return tag_names

def _promote_ecr_docker_image(from_version: str, to_version: str, ecr_repo: str):
    """
    Promotes a Docker image in the ECR nillion-node repo.
    """
    ecr_client = boto3.client("ecr")

    # pylint: disable=broad-exception-caught
    from_tag = f"{from_version}-amd64"
    to_tag = f"{to_version}-amd64"

    if not from_tag.startswith("v"):
        from_tag=f"v{from_tag}"

    if not to_tag.startswith("v"):
        to_tag=f"v{to_tag}"

    try:
        response = ecr_client.batch_get_image(
            repositoryName=ecr_repo,
            imageIds=[{"imageTag": from_tag}]
        )

    except (ecr_client.exceptions.LimitExceededException,
            ecr_client.exceptions.InvalidParameterException,
            ecr_client.exceptions.RepositoryNotFoundException,
            ecr_client.exceptions.UnableToGetUpstreamImageException,
            ecr_client.exceptions.ServerException) as ex:
        raise CommandError(
            "An error occurred batch getting images from the ECR API in repo"
            f" '{ecr_repo}' with image tag '{from_tag}'"
        ) from ex

    if "failures" in response and len(response['failures']) > 0:
        image_not_found = []

        for failure in response["failures"]:
            if failure["failureCode"] == "ImageNotFound":
                image_not_found.append(failure["imageId"]["imageTag"])

        if len(image_not_found) > 0:
            raise NotFoundError(
                f"Image not found in {ecr_repo} for tags: {', '.join(image_not_found)}"
            )

        raise CommandError(
            "An unexpected error was present in the response from the ECR API in repo"
            f" '{ecr_repo}' with image tag '{from_tag}': {response['failures']}"
        )

    if response is None:
        raise CommandError(
            "Empty images returned from ECR API for repo"
            f" '{ecr_repo}' and image tag '{from_tag}'"
        )

    images = response['images']

    if len(images) > 1:
        raise CommandError(
            "Multiple images returned from batch get for ECR repo"
            f" '{ecr_repo}' and image tag '{from_tag}'"
        )

    for image in images:
        if "imageManifest" not in image:
            raise CommandError(
                "Image with no manifest returned from ECR API for repo"
                f" '{ecr_repo}' and image tag '{from_tag}'"
            )

        try:
            promoted_image = ecr_client.put_image(
                repositoryName=ecr_repo,
                imageManifest=image['imageManifest'],
                imageDigest=image['imageId']['imageDigest'],
                imageTag=to_tag,
            )
        except (ecr_client.exceptions.ServerException,
                ecr_client.exceptions.InvalidParameterException,
                ecr_client.exceptions.RepositoryNotFoundException,
                ecr_client.exceptions.ImageAlreadyExistsException,
                ecr_client.exceptions.LayersNotFoundException,
                ecr_client.exceptions.ReferencedImagesNotFoundException,
                ecr_client.exceptions.LimitExceededException,
                ecr_client.exceptions.ImageTagAlreadyExistsException,
                ecr_client.exceptions.ImageDigestDoesNotMatchException,
                ecr_client.exceptions.KmsException) as ex:
            raise CommandError(
                "An error occurred putting an image via the ECR API in repo"
                f" '{ecr_repo}' with image tag '{to_tag}': {ex}"
            ) from ex
        except ClientError as ex:
            raise CommandError(
                "An unexpected error occurred putting an image via the ECR API in repo"
                f" '{ecr_repo}' with image tag '{to_tag}': {ex}"
            ) from ex
