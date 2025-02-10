"""
Checks S3 for SDK artifacts.
"""

# pylint: disable=import-error

import boto3

from release_manager.errors import CommandError, NotFoundError

from release_manager.constants import NILLION_RELEASES_BUCKET

def check_s3_artifacts(release_version_prefix: str=""):
    """
    A function that conforms to existing methods of checking for releases.
    """
    def _check_s3_artifacts(release_version: str):
        """
        Checks S3 for release with matching version number.
        """
        s3_client = boto3.client("s3")


        if release_version_prefix != "v" and release_version.startswith("v"):
            release_version=f"{release_version.lstrip('v')}/"
        elif release_version_prefix == "v" and release_version.startswith("v"):
            release_version=f"{release_version}/"
        else:
            release_version=f"{release_version_prefix}{release_version}/"

        try:
            objects = s3_client.list_objects(
                Bucket=NILLION_RELEASES_BUCKET,
                Prefix=release_version,
		Delimiter='/',
            )
        except s3_client.exceptions.NoSuchBucket as ex:
            raise CommandError(
                f"An error occurred listing objects in the {NILLION_RELEASES_BUCKET} bucket"
            ) from ex

        if "Contents" not in objects:
            raise NotFoundError(
                f"No such release '{release_version}' in S3"
            )

        contents = objects["Contents"]

        # TODO: Replace basic check with, e.g. checksum file and checksum
        # comparisons.
        if len(contents) == 0:
            raise CommandError(
                f"Release '{release_version}' has no files in S3"
            )

    return _check_s3_artifacts
